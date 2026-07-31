#[async_trait::async_trait]
trait MyTrait {
    async fn work(&self) -> usize;
}

struct MyStruct;

#[async_trait::async_trait]
impl MyTrait for MyStruct {
    #[logcall::logcall("info")]
    #[fastrace::trace]
    async fn work(&self) -> usize {
        1
    }
}

#[tokio::test]
async fn test() {
    assert_eq!(MyStruct.work().await, 1);
}
