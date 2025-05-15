use fastrace::trace;

// This Tracing crate like-syntax
#[allow(unused_braces)]
#[trace]
fn f(a: u32) -> u32 {
    a
}

fn main() {}
