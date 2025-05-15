use fastrace::trace;

#[trace(short_name = true, properties = { "k1": "v1", "a": "argument a is {a:?}", "b": "{b:?}", "escaped1": "{c:?}{{}}", "escaped2": "{{ \"a\": \"b\"}}" })]
async fn f(a: i64, b: &Bar, c: Bar) -> i64 {
    drop(c);
    a
}

#[derive(Debug)]
struct Bar;

#[trace(short_name = true, properties = {})]
async fn g(a: u32) -> u32 {
    a
}

#[tokio::main]
async fn main() {
    f(1, &Bar, Bar).await;
    g(1).await;
}
