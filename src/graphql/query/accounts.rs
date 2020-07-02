use crate::{
    auth,
    graphql::{context::Context, query::Query},
    model,
};
use juniper::FieldResult;
use sqlx::query_as_unchecked;

#[juniper::graphql_object(Context = Context)]
impl Query {
    pub async fn accounts(ctx: &Context) -> FieldResult<Vec<model::Account>> {
        if ctx.is_authenticated() {
            Ok(
                query_as_unchecked!(model::Account, "SELECT * FROM accounts")
                    .fetch_all(ctx.database())
                    .await?,
            )
        } else {
            Err(auth::AuthError::InvalidCredentials.into())
        }
    }
}
