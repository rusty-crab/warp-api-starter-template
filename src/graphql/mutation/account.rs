use crate::graphql::{mutation::Mutation, Context};
use crate::{auth, model};
use juniper::FieldResult;
use sqlx::{query_as_unchecked, query_unchecked};
use uuid::Uuid;

#[juniper::graphql_object(Context = Context)]
impl Mutation {
    fn account() -> AccountMutation {
        AccountMutation
    }
}

#[derive(juniper::GraphQLInputObject, Debug)]
pub struct CreateAccountInput {
    email: String,
    password: String,
}

pub struct AccountMutation;

#[derive(juniper::GraphQLInputObject, Debug)]
pub struct AccountInput {
    email: Option<String>,
}

#[juniper::graphql_object(Context = Context)]
impl AccountMutation {
    async fn create(ctx: &Context, input: CreateAccountInput) -> FieldResult<model::Account> {
        let argon = ctx.argon();

        let CreateAccountInput { email, password } = input;
        let password = argon.hasher().with_password(password).hash()?;
        let id = Uuid::new_v4();

        query_unchecked!(
            r#"
            INSERT INTO accounts (id, email, password) 
                VALUES ($1, $2, $3)
            "#,
            id,
            email,
            password
        )
        .execute(ctx.database())
        .await?;

        Ok(query_as_unchecked!(
            model::Account,
            r#"
            SELECT * FROM accounts 
            WHERE email = $1
        "#,
            email
        )
        .fetch_one(ctx.database())
        .await?)
    }

    async fn update(ctx: &Context, id: Uuid, input: AccountInput) -> FieldResult<model::Account> {
        let acc = ctx
            .session()
            .ok_or(auth::AuthError::InvalidCredentials)?
            .account()
            .await?;
        if acc.id != id {
            return Err(auth::AuthError::InvalidCredentials.into());
        }

        Ok(query_as_unchecked!(
            model::Account,
            r#"
    UPDATE accounts
      SET email = COALESCE($2, email)
      WHERE id = $1
      RETURNING *
  "#,
            id,
            input.email
        )
        .fetch_one(ctx.database())
        .await?)
    }
}
