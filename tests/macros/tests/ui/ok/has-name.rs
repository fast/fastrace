use fastrace::trace;

#[trace(name = "test-span")]
fn f(a: u32) -> u32 {
    a
}

fn main() {
    f(1);
}
