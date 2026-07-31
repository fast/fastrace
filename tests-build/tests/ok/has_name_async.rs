use fastrace::trace;

#[trace(name = "test-span")]
async fn f(a: u32) -> u32 {
    a
}

#[tokio::test]
async fn test() {
    assert_eq!(f(1).await, 1);
}
