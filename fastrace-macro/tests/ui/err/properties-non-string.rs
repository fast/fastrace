use fastrace::trace;

#[trace(properties = { a: "b" })]
fn f() {}

fn main() {}
