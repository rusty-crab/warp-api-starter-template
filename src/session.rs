use crate::{auth, helpers::cache, model, Environment};
use chrono::Utc;
use redis::aio::MultiplexedConnection;
use serde::{de::DeserializeOwned, Serialize};
use std::any::type_name;
use std::convert::TryInto;

#[derive(Clone)]
pub struct Session {
    auth: auth::Session,
    env: Environment,
    redis: MultiplexedConnection,
}

impl Session {
    pub async fn new(env: Environment, jwt: &str, csrf: &str) -> anyhow::Result<Self> {
        let session_key = auth::claims(&env, &jwt, &csrf)?.session();
        let mut redis = env.redis().await?;
        // Fetch session from cache if exists otherwise create
        let auth = cache::get_or_create(&mut redis, session_key, || async {
            let auth = auth::session(env.clone(), &jwt, &csrf).await?;
            let expiry = auth.expiry.signed_duration_since(Utc::now());
            let expiry: usize = expiry.num_seconds().try_into()?;
            Ok((auth, expiry))
        })
        .await?;
        Ok(Self { env, auth, redis })
    }

    pub async fn account(&self) -> anyhow::Result<model::Account> {
        crate::sql::account::get_account_by_session_key(self.env.database(), &self.auth.key).await
    }

    pub async fn _set<T: Serialize>(&mut self, value: &T) -> anyhow::Result<()> {
        let expiry = self.auth.expiry.signed_duration_since(Utc::now());

        cache::set_ex(
            &mut self.redis,
            format!("session:{}:{}", self.auth.key, type_name::<T>()),
            value,
            expiry.num_seconds().try_into()?,
        )
        .await?;

        Ok(())
    }

    pub async fn _get<T: DeserializeOwned>(&mut self) -> anyhow::Result<T> {
        cache::get(
            &mut self.redis,
            format!("session:{}:{}", self.auth.key, type_name::<T>()),
        )
        .await
    }
}
