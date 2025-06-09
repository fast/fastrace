use fastrace as custom_fastrace;
use custom_fastrace::trace;

#[trace(crate = custom_fastrace)]
async fn f(a: u32) -> u32 {
    a
}

#[trace(crate = ::fastrace, short_name = true)]
fn sync_func() -> i32 {
    42
}

#[tokio::main]
async fn main() {
    f(1).await;
    sync_func();
}
