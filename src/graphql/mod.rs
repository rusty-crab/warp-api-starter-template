mod context;
mod mutation;
mod query;

pub use context::Context;
use futures::channel::mpsc::channel;
use juniper::FieldResult;
use mutation::Mutation;
use query::Query;

pub type Schema = juniper::RootNode<'static, Query, Mutation, Subscription>;
pub fn schema() -> Schema {
    Schema::new(Query, Mutation, Subscription)
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
