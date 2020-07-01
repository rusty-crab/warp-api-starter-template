use crate::auth;
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
