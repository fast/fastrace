use fastrace::trace;

#[trace(properties = { "key": "first", "key": "second" })]
fn f() {}

fn main() {}
