use crate::{
    environment::Environment,
    model::{self, session::Identity},
};
use chrono::{Duration, Utc};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shrinkwraprs::Shrinkwrap;
use sqlx::{query, query_as_unchecked, query_unchecked, types::json::Json};
use std::net::SocketAddr;
use thiserror::Error;
use warp::{self, http, Reply};

#[derive(Shrinkwrap, Clone, Debug)]
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
    let account = query!(
        r#"
    SELECT id, password
      FROM accounts
      WHERE email = $1
    "#,
        &req.email
    )
    .fetch_optional(env.database())
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

    query_unchecked!(
        r#"
    INSERT INTO sessions (key, csrf, account, identity, expiry)
      VALUES ($1, $2, $3, $4, $5)
  "#,
        &claims.session,
        &claims.csrf,
        account.id,
        Json(identity),
        expiry
    )
    .execute(env.database())
    .await?;

    Ok((env.jwt().encode(claims, expiry)?, csrf))
}

pub async fn session(env: Environment, jwt: &str, csrf: &str) -> anyhow::Result<Session> {
    let claims: Claims = env.jwt().decode(jwt)?;

    if claims.csrf != csrf {
        return Err(AuthError::InvalidCredentials.into());
    }

    let session = query_as_unchecked!(
        model::Session,
        r#"
      SELECT *
        FROM sessions
        WHERE key = $1 AND csrf = $2 AND expiry > NOW() AND NOT invalidated
    "#,
        claims.session,
        &csrf
    )
    .fetch_optional(env.database())
    .await?;

    Ok(Session(session.ok_or(AuthError::InvalidCredentials)?))
}
