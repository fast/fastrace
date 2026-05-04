use fastrace::trace;

#[trace(poll_span = true, properties = { "a": "argument a is {a:?}" })]
async fn f(a: u32) -> u32 {
    a
}

#[tokio::main]
async fn main() {
    f(1).await;
}
