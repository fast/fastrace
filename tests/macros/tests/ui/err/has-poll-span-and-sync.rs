use fastrace::trace;

#[trace(poll_span = true)]
fn f() {}

fn main() {}
