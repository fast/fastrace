use fastrace::trace;

#[trace(name = "test-span")]
async fn f(mut a: u32) -> u32 {
    a += 1;
    a
}

#[tokio::test]
async fn test() {
    assert_eq!(f(1).await, 2);
}
