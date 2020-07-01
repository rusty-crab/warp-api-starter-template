use crate::auth;
use crate::model;
use crate::Environment;
use chrono::Utc;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::query_as_unchecked;
use std::any::type_name;
use std::convert::TryInto;

#[derive(Clone)]
pub struct Session {
    auth: auth::Session,
    env: Environment,
    redis: MultiplexedConnection,
}

impl Session {
    pub async fn new(env: Environment, auth: auth::Session) -> anyhow::Result<Self> {
        let redis = env.redis().await?;
        Ok(Self { env, auth, redis })
    }

    pub async fn account(&self) -> anyhow::Result<model::Account> {
        Ok(query_as_unchecked!(
            model::Account,
            r#"
        SELECT accounts.*
          FROM sessions
          INNER JOIN accounts
            ON sessions.account = accounts.id
          WHERE
            sessions.key = $1
      "#,
            self.auth.key
        )
        .fetch_one(self.env.database())
        .await?)
    }

    pub async fn _set<T: Serialize>(&mut self, value: &T) -> anyhow::Result<()> {
        let expiry = self.auth.expiry.signed_duration_since(Utc::now());

        self.redis
            .set_ex(
                format!("session:{}:{}", self.auth.key, type_name::<T>()),
                bincode::serialize(value)?,
                expiry.num_seconds().try_into()?,
            )
            .await?;

        Ok(())
    }

    pub async fn _get<T: DeserializeOwned>(&mut self) -> anyhow::Result<T> {
        let bytes: Vec<u8> = self
            .redis
            .get(format!("session:{}:{}", self.auth.key, type_name::<T>()))
            .await?;

        Ok(bincode::deserialize(&bytes)?)
    }
}
