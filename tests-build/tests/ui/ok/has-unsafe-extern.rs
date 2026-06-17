use fastrace::trace;

#[trace(short_name = true)]
pub(crate) unsafe extern "C" fn f(value: u32) -> u32 {
    value
}

fn main() {
    let _ = unsafe { f(7) };
}
