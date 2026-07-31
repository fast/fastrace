use fastrace::trace;

#[trace(short_name = true)]
async fn f(a: u32) -> u32 {
    a
}

#[tokio::test]
async fn test() {
    assert_eq!(f(1).await, 1);
}
