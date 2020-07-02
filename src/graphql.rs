use crate::{auth, environment::Environment, model, session::Session};
use futures::channel::mpsc::channel;
use juniper::FieldResult;
use shrinkwraprs::Shrinkwrap;
use sqlx::{query_as_unchecked, query_unchecked};
use uuid::Uuid;

pub type Schema = juniper::RootNode<'static, Query, Mutation, Subscription>;
pub fn schema() -> Schema {
    Schema::new(Query, Mutation, Subscription)
}

#[derive(Shrinkwrap, Clone)]
pub struct Context {
    session: Option<Session>,
    #[shrinkwrap(main_field)]
    env: Environment,
}

impl Context {
    pub async fn new(env: Environment, auth: Option<(String, String)>) -> anyhow::Result<Self> {
        if let Some((jwt, csrf)) = auth {
            let session = Some(Session::new(env.clone(), &jwt, &csrf).await?);
            Ok(Self { env, session })
        } else {
            Ok(Self { env, session: None })
        }
    }

    fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    fn is_authenticated(&self) -> bool {
        self.session.is_some()
    }
}

impl juniper::Context for Context {}

pub struct Query;

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

pub struct Mutation;

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

pub struct Subscription;

type CallsStream = std::pin::Pin<Box<dyn futures::Stream<Item = FieldResult<i32>> + Send>>;

#[juniper::graphql_subscription(Context = Context)]
impl Subscription {
    pub async fn calls(ctx: &Context) -> CallsStream {
        let (tx, rx) = channel(16);
        Box::pin(rx)
    }
}
