use fastrace::trace;

#[trace(properties = { "key": true })]
fn f() {}

fn main() {}
