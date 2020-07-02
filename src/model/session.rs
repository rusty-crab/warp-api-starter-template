use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{types::json::Json, FromRow};
use std::net::IpAddr;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, FromRow, Debug)]
pub struct Session {
    pub key: String,
    pub csrf: String,
    pub account: Uuid,
    pub identity: Json<Identity>,
    pub expiry: DateTime<Utc>,
    pub invalidated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub struct Identity {
    pub fingerprint: Option<String>,
    pub ip: Option<IpAddr>,
}
