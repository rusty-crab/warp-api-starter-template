use crate::model;
use sqlx::{postgres::PgPool, query_as, query_as_unchecked, query_unchecked};

pub async fn get_all_accounts(connection: &PgPool) -> anyhow::Result<Vec<model::Account>> {
    query_as_unchecked!(
        model::Account,
        "SELECT id, email, password, created_at, updated_at FROM accounts"
    )
    .fetch_all(connection)
    .await
    .map_err(|e| e.into())
}

pub async fn get_account(connection: &PgPool, email: &str) -> anyhow::Result<model::Account> {
    query_as_unchecked!(
        model::Account,
        r#"
SELECT id, email, password, created_at, updated_at
FROM accounts 
WHERE email = $1
"#,
        email
    )
    .fetch_one(connection)
    .await
    .map_err(|e| e.into())
}

pub async fn create_account(
    connection: &PgPool,
    id: uuid::Uuid,
    email: &str,
    password: &str,
) -> anyhow::Result<u64> {
    query_unchecked!(
        r#"
INSERT INTO accounts (id, email, password) 
    VALUES ($1, $2, $3)
"#,
        id,
        email,
        password
    )
    .execute(connection)
    .await
    .map_err(|e| e.into())
}

pub async fn update_email(
    connection: &PgPool,
    id: uuid::Uuid,
    email: &str,
) -> anyhow::Result<model::Account> {
    query_as_unchecked!(
        model::Account,
        r#"
UPDATE accounts
  SET email = COALESCE($2, email)
  WHERE id = $1
  RETURNING *
"#,
        id,
        email
    )
    .fetch_one(connection)
    .await
    .map_err(|e| e.into())
}

pub async fn get_account_by_session_key(
    connection: &PgPool,
    session_key: &str,
) -> anyhow::Result<model::Account> {
    Ok(query_as_unchecked!(
        model::Account,
        r#"
SELECT accounts.id, accounts.email, accounts.password, accounts.created_at, accounts.updated_at
  FROM sessions
  INNER JOIN accounts
    ON sessions.account = accounts.id
  WHERE
    sessions.key = $1
"#,
        session_key
    )
    .fetch_one(connection)
    .await?)
}

pub struct AccountByEmail {
    pub id: uuid::Uuid,
    pub password: String,
}

pub async fn get_account_id_password_by_email(
    connection: &PgPool,
    email: &str,
) -> anyhow::Result<Option<AccountByEmail>> {
    query_as!(
        AccountByEmail,
        r#"
SELECT id, password
  FROM accounts
  WHERE email = $1
"#,
        email
    )
    .fetch_optional(connection)
    .await
    .map_err(|e| e.into())
}

pub async fn create_session(
    connection: &PgPool,
    session_key: &str,
    csrf: &str,
    id: uuid::Uuid,
    identity: crate::model::session::Identity,
    expiry: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<u64> {
    query_unchecked!(
        r#"
INSERT INTO sessions (key, csrf, account, identity, expiry)
  VALUES ($1, $2, $3, $4, $5)
"#,
        session_key,
        csrf,
        id,
        sqlx::types::Json(identity),
        expiry
    )
    .execute(connection)
    .await
    .map_err(|e| e.into())
}

pub async fn get_csrf_validated_session(
    connection: &PgPool,
    session_key: &str,
    csrf: &str,
) -> anyhow::Result<Option<model::Session>> {
    query_as_unchecked!(
        model::Session,
        r#"
SELECT *
  FROM sessions
  WHERE key = $1 AND csrf = $2 AND expiry > NOW() AND NOT invalidated
"#,
        session_key,
        csrf
    )
    .fetch_optional(connection)
    .await
    .map_err(|e| e.into())
}
