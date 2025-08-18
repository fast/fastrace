use std::future::Future;
use std::pin::Pin;

use fastrace::trace;

#[derive(Debug)]
pub struct InnerError;

#[derive(Debug)]
pub struct OuterError(InnerError);

pub type MyFuture = Pin<Box<dyn Future<Output = Result<u32, OuterError>> + Send>>;

#[trace]
pub async fn f() -> Result<MyFuture, OuterError> {
    let inner = async { Err(InnerError) };

    let mapped = async move { inner.await.map_err(OuterError) };

    Ok(Box::pin(mapped))
}

#[tokio::main]
async fn main() {
    let _ = f().await;
}
