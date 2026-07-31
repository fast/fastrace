use fastrace::trace;

#[trace]
fn f(a: u32) -> u32 {
    a
}

#[test]
fn test() {
    assert_eq!(f(1), 1);
}
