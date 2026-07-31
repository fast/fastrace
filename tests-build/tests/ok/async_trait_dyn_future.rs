use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use fastrace::trace;

#[derive(Debug)]
struct InnerError;

#[derive(Debug)]
struct OuterError;

type MyFuture = Pin<Box<dyn Future<Output = Result<u32, OuterError>> + Send>>;

#[async_trait]
trait MyTrait {
    async fn f() -> Result<MyFuture, OuterError>;
}

struct MyStruct;

#[async_trait]
impl MyTrait for MyStruct {
    #[trace]
    async fn f() -> Result<MyFuture, OuterError> {
        let inner = async { Err::<u32, _>(InnerError) };
        let mapped = async move { inner.await.map_err(|_| OuterError) };
        Ok(Box::pin(mapped))
    }
}

#[tokio::test]
async fn test() {
    let future = MyStruct::f().await.unwrap();
    assert!(future.await.is_err());
}
