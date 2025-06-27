use crate::{command::Envelope, message};
use async_trait::async_trait;
use std::future::Future;

#[async_trait]
#[allow(dead_code)]
pub trait Handler<T>: Send + Sync
where
    T: message::Message,
{
    type Error: std::error::Error;

    async fn handle(&self, command: Envelope<T>) -> Result<(), Self::Error>;
}

#[async_trait]
impl<T, Err, F, Fut> Handler<T> for F
where
    T: message::Message + Send + Sync + 'static,
    Err: std::error::Error,
    F: Send + Sync + Fn(Envelope<T>) -> Fut,
    Fut: Send + Sync + Future<Output = Result<(), Err>>,
{
    type Error = Err;

    async fn handle(&self, command: Envelope<T>) -> Result<(), Self::Error> {
        self(command).await
    }
}
