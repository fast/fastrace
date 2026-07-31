use fastrace::trace;
use futures::Stream;

#[trace]
async fn stream() -> impl Stream<Item = i64> {
    async_stream::stream! {
        for i in 0..100 {
            yield i;
        }
    }
}

#[tokio::test]
async fn test() {
    let _ = stream().await;
}
