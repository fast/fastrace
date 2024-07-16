use fastrace::trace;

#[trace(enter_on_poll = true, properties = { "a": "b" })]
fn f() {}

fn main() {}
