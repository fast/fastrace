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
        todo!()
    }
}

fn main() {}
