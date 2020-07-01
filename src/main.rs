mod argon;
mod auth;
mod cache;
mod environment;
mod graphql;
mod jwt;
mod model;
mod problem;
mod session;

use clap::Clap;
use environment::Environment;
use std::net::SocketAddr;
use warp::Filter;

#[derive(Clap, Debug)]
#[clap(
    name = "warp-api-app",
    rename_all = "kebab-case",
    rename_all_env = "screaming-snake"
)]
pub struct Args {
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
            .and(context)
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
