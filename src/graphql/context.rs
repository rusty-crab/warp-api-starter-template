use crate::{environment::Environment, session::Session};
use shrinkwraprs::Shrinkwrap;

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

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub fn is_authenticated(&self) -> bool {
        self.session.is_some()
    }
}

impl juniper::Context for Context {}
