use crate::graphql::{mutation::Mutation, Context};
use crate::{auth, model};
use juniper::FieldResult;
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
    email: String,
}

#[juniper::graphql_object(Context = Context)]
impl AccountMutation {
    async fn create(ctx: &Context, input: CreateAccountInput) -> FieldResult<model::Account> {
        let argon = ctx.argon();

        let CreateAccountInput { email, password } = input;
        let password = argon.hasher().with_password(password).hash()?;
        let id = Uuid::new_v4();

        crate::sql::account::create_account(ctx.database(), id, &email, &password).await?;

        Ok(crate::sql::account::get_account(ctx.database(), &email).await?)
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

        Ok(crate::sql::account::update_email(ctx.database(), id, &input.email).await?)
    }
}
