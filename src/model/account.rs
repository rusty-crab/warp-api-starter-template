use super::redacted::Redacted;
use chrono::{DateTime, Utc};
use juniper::GraphQLObject;
use serde::{Deserialize, Serialize};
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
