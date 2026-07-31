use fastrace::trace;

#[trace(short_name = true)]
pub(crate) unsafe extern "C" fn f(value: u32) -> u32 {
    value
}

#[test]
fn test() {
    assert_eq!(unsafe { f(7) }, 7);
}
