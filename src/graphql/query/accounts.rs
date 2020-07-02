use crate::{
    auth,
    graphql::{context::Context, query::Query},
    model,
};
use juniper::FieldResult;

#[juniper::graphql_object(Context = Context)]
impl Query {
    pub async fn accounts(ctx: &Context) -> FieldResult<Vec<model::Account>> {
        if ctx.is_authenticated() {
            Ok(crate::sql::account::get_all_accounts(ctx.database()).await?)
        } else {
            Err(auth::AuthError::InvalidCredentials.into())
        }
    }
}
