use crate::{
    environment::Environment,
    model::{self, session::Identity},
};
use chrono::{Duration, Utc};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shrinkwraprs::Shrinkwrap;
use std::net::SocketAddr;
use thiserror::Error;
use warp::{self, http, Reply};

#[derive(Shrinkwrap, Clone, Serialize, Deserialize, Debug)]
pub struct Session(model::Session);

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    email: String,
    password: String,
    lifetime: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Claims {
    session: String,
    csrf: String,
}

impl Claims {
    pub fn session(&self) -> String {
        self.session.to_owned()
    }
}

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("could not hash password")]
    ArgonError,
}

pub async fn filter(
    env: Environment,
    req: Request,
    address: Option<SocketAddr>,
) -> anyhow::Result<impl Reply> {
    let (jwt, csrf) = request(env, req, address).await?;

    let reply = warp::reply::json(&json!({ "jwt": jwt, "csrf": csrf }));
    let reply = warp::reply::with_status(reply, http::StatusCode::OK);

    let reply = warp::reply::with_header(
        reply,
        http::header::CONTENT_TYPE,
        http_api_problem::PROBLEM_JSON_MEDIA_TYPE,
    );

    let reply = warp::reply::with_header(reply, http::header::SET_COOKIE, format!("jwt={}", jwt));

    Ok(reply)
}

async fn request(
    env: Environment,
    req: Request,
    address: Option<SocketAddr>,
) -> anyhow::Result<(String, String)> {
    let account = crate::sql::account::get_account_id_password_by_email(env.database(), &req.email)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

    let is_valid = env
        .argon()
        .verifier()
        .with_hash(&account.password)
        .with_password(&req.password)
        .verify()
        .or(Err(AuthError::ArgonError))?;

    if !is_valid {
        return Err(AuthError::InvalidCredentials.into());
    }

    let identity = Identity {
        fingerprint: None,
        ip: address.map(|addr| addr.ip()),
    };

    let claims = Claims {
        session: rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .collect(),
        csrf: rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .collect(),
    };

    let csrf = claims.csrf.clone();
    let expiry = Utc::now() + Duration::seconds(env.session_lifetime(req.lifetime));

    crate::sql::account::create_session(
        env.database(),
        &claims.session,
        &claims.csrf,
        account.id,
        identity,
        expiry,
    )
    .await?;

    Ok((env.jwt().encode(claims, expiry)?, csrf))
}

pub fn claims(env: &Environment, jwt: &str, csrf: &str) -> anyhow::Result<Claims> {
    let claims: Claims = env.jwt().decode(jwt)?;

    if claims.csrf != csrf {
        return Err(AuthError::InvalidCredentials.into());
    }

    Ok(claims)
}

pub async fn session(env: Environment, jwt: &str, csrf: &str) -> anyhow::Result<Session> {
    let claims = claims(&env, &jwt, &csrf)?;

    let session =
        crate::sql::account::get_csrf_validated_session(env.database(), &claims.session, &csrf)
            .await?;

    Ok(Session(session.ok_or(AuthError::InvalidCredentials)?))
}
