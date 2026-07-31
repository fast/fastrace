trait MyTrait {
    async fn work(&self) -> usize;
}

struct MyStruct;

impl MyTrait for MyStruct {
    #[fastrace::trace]
    async fn work(&self) -> usize {
        1
    }
}

#[tokio::test]
async fn test() {
    assert_eq!(MyStruct.work().await, 1);
}
