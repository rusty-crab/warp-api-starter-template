use chrono::{DateTime, Utc};
use juniper::GraphQLObject;
use serde::{Deserialize, Serialize, Serializer};
use shrinkwraprs::Shrinkwrap;
use sqlx::{types::json::Json, FromRow};
use std::fmt;
use std::iter;
use unicode_width::UnicodeWidthStr;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, GraphQLObject, Debug)]
pub struct Account {
    pub id: Uuid,
    pub email: String,

    #[graphql(skip)]
    #[serde(skip_serializing)]
    pub password: Redacted<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize, FromRow, Debug)]
pub struct Session {
    pub key: String,
    pub csrf: String,
    pub account: Uuid,
    pub identity: Json<session::Identity>,
    pub expiry: DateTime<Utc>,
    pub invalidated: bool,

    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}
pub mod session {
    use serde::{Deserialize, Serialize};
    use std::net::IpAddr;

    #[derive(Clone, Serialize, Deserialize, Default, Debug)]
    pub struct Identity {
        pub fingerprint: Option<String>,
        pub ip: Option<IpAddr>,
    }
}
#[derive(
    Shrinkwrap, Deserialize, sqlx::Type, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default,
)]
#[sqlx(transparent)]
pub struct Redacted<T>(T);

impl<T> Redacted<T> {
    pub fn _new(value: T) -> Self {
        Self(value)
    }
}

impl<T> Serialize for Redacted<T> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_none()
    }
}

impl<T: fmt::Debug> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            iter::repeat("█")
                .take(UnicodeWidthStr::width(format!("{:?}", self.0).as_str()))
                .collect::<String>()
        )
    }
}

impl<T: fmt::Display> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            iter::repeat("█")
                .take(UnicodeWidthStr::width(format!("{}", self.0).as_str()))
                .collect::<String>()
        )
    }
}
