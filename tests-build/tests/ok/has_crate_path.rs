use custom_fastrace::trace;
use fastrace as custom_fastrace;

#[trace(crate = custom_fastrace)]
async fn f(a: u32) -> u32 {
    a
}

#[trace(crate = ::fastrace, short_name = true)]
fn sync_func() -> i32 {
    42
}

#[tokio::test]
async fn test() {
    assert_eq!(f(1).await, 1);
    assert_eq!(sync_func(), 42);
}
