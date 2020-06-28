use clap::Clap;
use sqlx::postgres::PgPool;
use std::net::SocketAddr;
use warp::Filter;

#[derive(Clap, Debug)]
#[clap(
    name = "warp-api-app",
    rename_all = "kebab-case",
    rename_all_env = "screaming-snake"
)]
struct Args {
    #[clap(short, long)]
    debug: bool,

    #[clap(required = true, short = "D", long, env)]
    database_url: String,
    #[clap(required = true, short = "R", long, env)]
    redis_url: String,

    #[clap(required = true, long, env)]
    jwt_secret: String,
    #[clap(required = true, long, env)]
    argon_secret: String,
    #[clap(long, env)]
    argon_iterations: Option<u32>,
    #[clap(long, env)]
    argon_memory_size: Option<u32>,
    #[clap(short, long, env)]
    session_lifetime: Option<i64>,

    #[clap(default_value = "127.0.0.1:3535")]
    host: SocketAddr,
}

#[derive(Clone, Debug)]
pub struct Argon {
    secret: String,
    memory_size: Option<u32>,
    iterations: Option<u32>,
}

impl Argon {
    fn new(args: &Args) -> Self {
        let Args {
            argon_secret,
            argon_memory_size,
            argon_iterations,
            ..
        } = args;
        Self {
            secret: argon_secret.to_owned(),
            memory_size: argon_memory_size.to_owned(),
            iterations: argon_iterations.to_owned(),
        }
    }

    fn hasher(&self) -> argonautica::Hasher<'static> {
        let mut hasher = argonautica::Hasher::default();
        let mut hasher = hasher.with_secret_key(&self.secret);
        if let Some(memory_size) = self.memory_size {
            hasher = hasher.configure_memory_size(memory_size);
        }
        if let Some(iterations) = self.iterations {
            hasher = hasher.configure_iterations(iterations);
        }
        hasher.to_owned()
    }

    fn verifier(&self) -> argonautica::Verifier<'static> {
        let mut verifier = argonautica::Verifier::default();
        let verifier = verifier.with_secret_key(&self.secret);
        verifier.to_owned()
    }
}

type DateTimeUtc = chrono::DateTime<chrono::Utc>;
#[derive(Clone, Debug)]
pub struct Jwt {
    secret: String,
}

impl Jwt {
    fn new(secret: &str) -> Self {
        Self {
            secret: secret.to_owned(),
        }
    }

    pub fn encode(&self, claims: auth::Claims, _expiry: DateTimeUtc) -> anyhow::Result<String> {
        let registered = biscuit::RegisteredClaims {
            issuer: Some(std::str::FromStr::from_str(
                "https://www.example.com/change/me",
            )?),
            subject: Some(std::str::FromStr::from_str(
                "JSON Web Token Issued for Warp API Starter Template",
            )?),
            ..Default::default()
        };
        let private = claims;
        let claims = biscuit::ClaimsSet::<auth::Claims> {
            registered,
            private,
        };

        let jwt = biscuit::JWT::new_decoded(
            From::from(biscuit::jws::RegisteredHeader {
                algorithm: biscuit::jwa::SignatureAlgorithm::HS256,
                ..Default::default()
            }),
            claims.clone(),
        );

        let secret = biscuit::jws::Secret::bytes_from_str(&self.secret);

        jwt.into_encoded(&secret)
            .map(|t| t.unwrap_encoded().to_string())
            .map_err(|e| anyhow::anyhow!("Unable to encode jwt {:?}", e))
    }

    pub fn decode(&self, token: &str) -> anyhow::Result<auth::Claims> {
        let token = biscuit::JWT::<auth::Claims, biscuit::Empty>::new_encoded(&token);
        let secret = biscuit::jws::Secret::bytes_from_str(&self.secret);
        let token = token.into_decoded(&secret, biscuit::jwa::SignatureAlgorithm::HS256)?;
        let payload = token.payload()?.private.to_owned();
        Ok(payload)
    }
}

#[derive(Clone, Debug)]
pub struct Environment {
    db_pool: PgPool,
    redis: redis::Client,
    argon: Argon,
    jwt: Jwt,
    session_lifetime: Option<i64>,
}

impl Environment {
    async fn new(args: &Args) -> anyhow::Result<Self> {
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

    fn database(&self) -> &PgPool {
        &self.db_pool
    }

    fn argon(&self) -> &Argon {
        &self.argon
    }

    async fn redis(&self) -> anyhow::Result<redis::aio::MultiplexedConnection> {
        self.redis
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|e| anyhow::anyhow!("error occured on redis connection, {:?}", e))
    }

    fn jwt(&self) -> &Jwt {
        &self.jwt
    }

    fn session_lifetime(&self, req_lifetime: Option<i64>) -> i64 {
        req_lifetime.or(self.session_lifetime).unwrap_or(86400i64)
    }
}

mod session {
    use super::auth;
    use super::model;
    use super::Environment;
    use chrono::Utc;
    use redis::aio::MultiplexedConnection;
    use redis::AsyncCommands;
    use serde::{de::DeserializeOwned, Serialize};
    use sqlx::query_as_unchecked;
    use std::any::type_name;
    use std::convert::TryInto;

    #[derive(Clone)]
    pub struct Session {
        auth: auth::Session,
        env: Environment,
        redis: MultiplexedConnection,
    }

    impl Session {
        pub async fn new(env: Environment, auth: auth::Session) -> anyhow::Result<Self> {
            let redis = env.redis().await?;
            Ok(Self { env, auth, redis })
        }

        pub async fn account(&self) -> anyhow::Result<model::Account> {
            Ok(query_as_unchecked!(
                model::Account,
                r#"
        SELECT accounts.*
          FROM sessions
          INNER JOIN accounts
            ON sessions.account = accounts.id
          WHERE
            sessions.key = $1
      "#,
                self.auth.key
            )
            .fetch_one(self.env.database())
            .await?)
        }

        pub async fn _set<T: Serialize>(&mut self, value: &T) -> anyhow::Result<()> {
            let expiry = self.auth.expiry.signed_duration_since(Utc::now());

            self.redis
                .set_ex(
                    format!("session:{}:{}", self.auth.key, type_name::<T>()),
                    bincode::serialize(value)?,
                    expiry.num_seconds().try_into()?,
                )
                .await?;

            Ok(())
        }

        pub async fn _get<T: DeserializeOwned>(&mut self) -> anyhow::Result<T> {
            let bytes: Vec<u8> = self
                .redis
                .get(format!("session:{}:{}", self.auth.key, type_name::<T>()))
                .await?;

            Ok(bincode::deserialize(&bytes)?)
        }
    }
}

mod graphql {
    use super::auth;
    use super::model;
    use super::session::Session;
    use super::Environment;
    use futures::channel::mpsc::channel;
    use juniper::FieldResult;
    use sqlx::{query_as_unchecked, query_unchecked};
    use uuid::Uuid;

    pub type Schema = juniper::RootNode<'static, Query, Mutation, Subscription>;
    pub fn schema() -> Schema {
        Schema::new(Query, Mutation, Subscription)
    }

    #[derive(Clone)]
    pub struct Context {
        session: Option<Session>,
        env: Environment,
    }

    impl Context {
        pub async fn new(env: Environment, auth: Option<(String, String)>) -> anyhow::Result<Self> {
            if let Some((jwt, csrf)) = auth {
                let session = auth::session(env.clone(), &jwt, &csrf).await?;
                let session = Session::new(env.clone(), session).await.ok();
                Ok(Self { env, session })
            } else {
                Ok(Self { env, session: None })
            }
        }

        fn database(&self) -> &sqlx::postgres::PgPool {
            self.env.database()
        }

        fn session(&self) -> Option<&Session> {
            self.session.as_ref()
        }
    }

    impl juniper::Context for Context {}

    pub struct Query;

    #[juniper::graphql_object(Context = Context)]
    impl Query {
        pub async fn accounts(ctx: &Context) -> FieldResult<Vec<model::Account>> {
            Ok(
                query_as_unchecked!(model::Account, "SELECT * FROM accounts")
                    .fetch_all(ctx.database())
                    .await?,
            )
        }
    }

    pub struct Mutation;

    #[juniper::graphql_object(Context = Context)]
    impl Mutation {
        fn create_account() -> CreateAccountMutation {
            CreateAccountMutation
        }

        fn account() -> AccountMutation {
            AccountMutation
        }
    }

    pub struct CreateAccountMutation;

    #[derive(juniper::GraphQLInputObject, Debug)]
    pub struct CreateAccountInput {
        email: String,
        password: String,
    }

    #[juniper::graphql_object(Context = Context)]
    impl CreateAccountMutation {
        async fn create(ctx: &Context, input: CreateAccountInput) -> FieldResult<model::Account> {
            let argon = ctx.env.argon();

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
    }

    pub struct AccountMutation;

    #[derive(juniper::GraphQLInputObject, Debug)]
    pub struct AccountInput {
        email: Option<String>,
    }

    #[juniper::graphql_object(Context = Context)]
    impl AccountMutation {
        async fn update(
            ctx: &Context,
            id: Uuid,
            input: AccountInput,
        ) -> FieldResult<model::Account> {
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
}

mod model {
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

    #[derive(Clone, FromRow, Debug)]
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
}

mod auth {
    use super::model::{self, session::Identity};
    use super::Environment;
    use chrono::{Duration, Utc};
    use rand::{distributions::Alphanumeric, Rng};
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use shrinkwraprs::Shrinkwrap;
    use sqlx::{query, query_as_unchecked, query_unchecked, types::json::Json};
    use std::net::SocketAddr;
    use thiserror::Error;
    use warp::{self, http, Reply};

    #[derive(Shrinkwrap, Clone, Debug)]
    pub struct Session(model::Session);

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Request {
        email: String,
        password: String,
        lifetime: Option<i64>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Claims {
        session: String,
        csrf: String,
    }

    #[allow(dead_code)]
    #[derive(Error, Debug)]
    pub enum AuthError {
        #[error("invalid credentials")]
        InvalidCredentials,
        #[error("could not hash password")]
        ArgonError,
    }

    pub async fn filter(
        env: Environment,
        req: Request,
        address: Option<SocketAddr>,
    ) -> anyhow::Result<impl Reply> {
        let (jwt, csrf) = request(env, req, address).await?;

        let reply = warp::reply::json(&json!({ "jwt": jwt, "csrf": csrf }));
        let reply = warp::reply::with_status(reply, http::StatusCode::OK);

        let reply = warp::reply::with_header(
            reply,
            http::header::CONTENT_TYPE,
            http_api_problem::PROBLEM_JSON_MEDIA_TYPE,
        );

        let reply =
            warp::reply::with_header(reply, http::header::SET_COOKIE, format!("jwt={}", jwt));

        Ok(reply)
    }

    async fn request(
        env: Environment,
        req: Request,
        address: Option<SocketAddr>,
    ) -> anyhow::Result<(String, String)> {
        let account = query!(
            r#"
        SELECT id, password
          FROM accounts
          WHERE email = $1
        "#,
            &req.email
        )
        .fetch_optional(env.database())
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

        let is_valid = env
            .argon()
            .verifier()
            .with_hash(&account.password)
            .with_password(&req.password)
            .verify()
            .or(Err(AuthError::ArgonError))?;

        if !is_valid {
            return Err(AuthError::InvalidCredentials.into());
        }

        let identity = Identity {
            fingerprint: None,
            ip: address.map(|addr| addr.ip()),
        };

        let claims = Claims {
            session: rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(64)
                .collect(),
            csrf: rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(64)
                .collect(),
        };

        let csrf = claims.csrf.clone();
        let expiry = Utc::now() + Duration::seconds(env.session_lifetime(req.lifetime));

        query_unchecked!(
            r#"
        INSERT INTO sessions (key, csrf, account, identity, expiry)
          VALUES ($1, $2, $3, $4, $5)
      "#,
            &claims.session,
            &claims.csrf,
            account.id,
            Json(identity),
            expiry
        )
        .execute(env.database())
        .await?;

        Ok((env.jwt().encode(claims, expiry)?, csrf))
    }

    pub async fn session(env: Environment, jwt: &str, csrf: &str) -> anyhow::Result<Session> {
        let claims: Claims = env.jwt().decode(jwt)?;

        if claims.csrf != csrf {
            return Err(AuthError::InvalidCredentials.into());
        }

        let session = query_as_unchecked!(
            model::Session,
            r#"
          SELECT *
            FROM sessions
            WHERE key = $1 AND csrf = $2 AND expiry > NOW() AND NOT invalidated
        "#,
            claims.session,
            &csrf
        )
        .fetch_optional(env.database())
        .await?;

        Ok(Session(session.ok_or(AuthError::InvalidCredentials)?))
    }
}

mod problem {
    use super::auth;
    use http_api_problem::HttpApiProblem as Problem;
    use warp::http;
    use warp::{Rejection, Reply};

    pub fn build<E: Into<anyhow::Error>>(err: E) -> Rejection {
        warp::reject::custom(pack(err.into()))
    }

    pub fn pack(err: anyhow::Error) -> Problem {
        let err = match err.downcast::<Problem>() {
            Ok(problem) => return problem,

            Err(err) => err,
        };

        if let Some(err) = err.downcast_ref::<auth::AuthError>() {
            match err {
                auth::AuthError::InvalidCredentials => {
                    return Problem::new("Invalid credentials.")
                        .set_status(http::StatusCode::BAD_REQUEST)
                        .set_detail("The passed credentials were invalid.")
                }

                auth::AuthError::ArgonError => (),
            }
        }

        tracing::error!("internal error occurred: {:#}", err);
        Problem::with_title_and_type_from_status(http::StatusCode::INTERNAL_SERVER_ERROR)
    }

    pub async fn unpack(rejection: Rejection) -> Result<impl Reply, Rejection> {
        if let Some(problem) = rejection.find::<Problem>() {
            let code = problem
                .status
                .unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);

            let reply = warp::reply::json(problem);
            let reply = warp::reply::with_status(reply, code);
            let reply = warp::reply::with_header(
                reply,
                http::header::CONTENT_TYPE,
                http_api_problem::PROBLEM_JSON_MEDIA_TYPE,
            );

            Ok(reply)
        } else {
            Err(rejection)
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    if dotenv::dotenv().is_err() {
        eprintln!("Warning: Did not find .env file in current working directory!");
    }
    let args = Args::parse();
    let env = Environment::new(&args).await?;
    let env = warp::any().map(move || env.clone());
    let cors = warp::cors()
        .allow_methods(vec!["GET", "POST"])
        .allow_header("content-type")
        .allow_header("authorization")
        .allow_any_origin()
        .build();
    let log = warp::log("brrrrr::request");
    let auth = warp::post()
        .and(warp::path("auth"))
        .and(env.clone())
        .and(warp::body::json())
        .and(warp::addr::remote())
        .and_then(|env, req, addr| async move {
            auth::filter(env, req, addr).await.map_err(problem::build)
        });
    let graphql = {
        use futures::{future, FutureExt as _};
        use juniper_subscriptions::*;
        use juniper_warp::{subscriptions::*, *};
        use serde::Deserialize;
        use std::sync::Arc;
        use warp::Filter;

        #[derive(Deserialize, Debug)]
        struct Query {
            csrf: Option<String>,
        }

        let auth = warp::header::optional("authorization")
            .or(warp::cookie::optional("jwt"))
            .unify()
            .and(warp::query())
            .and_then(|jwt: Option<String>, query: Query| {
                if jwt.is_none() && query.csrf.is_none() {
                    return future::ok(None);
                }

                if jwt.is_none() || query.csrf.is_none() {
                    return future::err(problem::build(auth::AuthError::InvalidCredentials));
                }

                future::ok(Some((jwt.unwrap(), query.csrf.unwrap())))
            });

        let context = warp::any()
            .and(env.clone())
            .and(auth)
            .and_then(|env, auth| async {
                graphql::Context::new(env, auth)
                    .await
                    .map_err(problem::build)
            })
            .boxed();

        let coordinator = Arc::new(Coordinator::new(graphql::schema()));

        let query = warp::post()
            .and(warp::path("query"))
            .and(make_graphql_filter(graphql::schema(), context.clone()));

        let subscriptions = warp::path("subscriptions")
            .and(warp::ws())
            .and(context.clone())
            .and(warp::any().map(move || Arc::clone(&coordinator)))
            .map(|socket: warp::ws::Ws, context, coordinator| {
                socket.on_upgrade(|socket| {
                    graphql_subscriptions(socket, coordinator, context)
                        .map(|res| {
                            if let Err(err) = res {
                                tracing::error!("websocket error: {:?}", err);
                            }
                        })
                        .boxed()
                })
            })
            .map(|reply| warp::reply::with_header(reply, "Sec-WebSocket-Protocol", "graphql-ws"));

        let playground = warp::path("playground").and(playground_filter(
            "/graphql/query",
            Some("/graphql/subscriptions"),
        ));

        warp::path("graphql").and(query.or(subscriptions).or(playground))
    };

    warp::serve(
        auth.or(graphql)
            .recover(problem::unpack)
            .with(cors)
            .with(log),
    )
    .run(args.host)
    .await;

    Ok(())
}
