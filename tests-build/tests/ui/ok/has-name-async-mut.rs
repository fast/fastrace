#![allow(unused_mut)]

use fastrace::trace;

#[trace(name = "test-span")]
async fn f(mut a: u32) -> u32 {
    a
}

#[tokio::test]
async fn test() {
    f(1).await;
}
