mod auth;
mod environment;
mod graphql;
mod helpers;
mod model;
mod session;
mod sql;

use clap::Clap;
use environment::Environment;
use helpers::problem;
use hyper::server::Server;
use listenfd::ListenFd;
use std::convert::Infallible;
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

    #[clap(required = true, short = 'D', long, env)]
    database_url: String,
    #[clap(required = true, short = 'R', long, env)]
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

    #[clap(default_value = "127.0.0.1:3535", env)]
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
    let log = warp::log("api::request");
    let status = warp::path("status")
        .and(warp::get())
        .and(warp::path::end())
        .map(|| format!("OK"));
    let auth = warp::path("auth")
        .and(warp::post())
        .and(warp::path::end())
        .and(env.clone())
        .and(warp::body::json())
        .and(warp::addr::remote())
        .and_then(|env, req, addr| async move {
            auth::filter(env, req, addr).await.map_err(problem::build)
        });
    let graphql = {
        use futures::FutureExt as _;
        use juniper_subscriptions::Coordinator;
        use juniper_warp::{
            make_graphql_filter, playground_filter, subscriptions::graphql_subscriptions,
        };
        use serde::Deserialize;
        use std::sync::Arc;
        use warp::Filter;

        #[derive(Deserialize, Debug)]
        struct Query {
            csrf: Option<String>,
        }

        let auth = warp::header("authorization")
            .or(warp::cookie("jwt"))
            .unify()
            .map(Some)
            .or(warp::any().map(|| None))
            .unify()
            .and(warp::query())
            .and_then(|jwt: Option<String>, query: Query| async {
                if jwt.is_none() && query.csrf.is_none() {
                    return Ok(None);
                }

                if jwt.is_none() || query.csrf.is_none() {
                    return Err(problem::build(auth::AuthError::InvalidCredentials));
                }

                Ok(Some((jwt.unwrap(), query.csrf.unwrap())))
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

        let query = warp::path("query")
            .and(warp::post())
            .and(warp::path::end())
            .and(make_graphql_filter(graphql::schema(), context.clone()));

        let subscriptions = warp::path("subscriptions")
            .and(warp::path::end())
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

        let playground = warp::path("playground")
            .and(warp::path::end())
            .and(playground_filter(
                "/graphql/query",
                Some("/graphql/subscriptions"),
            ));

        warp::path("graphql").and(query.or(subscriptions).or(playground))
    };

    let svc = warp::service(
        auth.or(status)
            .or(graphql)
            .recover(problem::unpack)
            .with(cors)
            .with(log),
    );

    let make_svc = hyper::service::make_service_fn(|_: _| {
        let svc = svc.clone();
        async move { Ok::<_, Infallible>(svc) }
    });

    let mut listenfd = ListenFd::from_env();

    let server = if let Some(l) = listenfd.take_tcp_listener(0).unwrap() {
        Server::from_tcp(l)?
    } else {
        Server::bind(&args.host)
    };

    server.serve(make_svc).await?;

    Ok(())
}
