use fastrace::trace;

#[trace(name = "Name", short_name = false)]
async fn f(a: u32) -> u32 {
    a
}

#[tokio::main]
async fn main() {
    f(1).await;
}
