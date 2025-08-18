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

#[tokio::main]
async fn main() {
    let _ = stream().await;
}
