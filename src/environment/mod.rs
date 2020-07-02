mod argon;
mod jwt;

use crate::Args;
use argon::Argon;
use jwt::Jwt;
use sqlx::postgres::PgPool;

#[derive(Clone, Debug)]
pub struct Environment {
    db_pool: PgPool,
    redis: redis::Client,
    argon: Argon,
    jwt: Jwt,
    session_lifetime: Option<i64>,
}

impl Environment {
    pub async fn new(args: &Args) -> anyhow::Result<Self> {
        let Args {
            database_url,
            redis_url,
            session_lifetime,
            jwt_secret,
            ..
        } = &args;
        let db_pool = PgPool::builder().max_size(5).build(database_url).await?;
        let redis = redis::Client::open(redis_url.as_str())?;
        let argon = Argon::new(&args);
        let jwt = Jwt::new(&jwt_secret);
        Ok(Self {
            db_pool,
            redis,
            argon,
            jwt,
            session_lifetime: session_lifetime.to_owned(),
        })
    }

    pub fn database(&self) -> &PgPool {
        &self.db_pool
    }

    pub fn argon(&self) -> &Argon {
        &self.argon
    }

    pub async fn redis(&self) -> anyhow::Result<redis::aio::MultiplexedConnection> {
        self.redis
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|e| e.into())
    }

    pub fn jwt(&self) -> &Jwt {
        &self.jwt
    }

    pub fn session_lifetime(&self, req_lifetime: Option<i64>) -> i64 {
        req_lifetime.or(self.session_lifetime).unwrap_or(86400i64)
    }
}
