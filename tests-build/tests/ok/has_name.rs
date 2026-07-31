use fastrace::trace;

#[trace(name = "test-span")]
fn f(a: u32) -> u32 {
    a
}

#[test]
fn test() {
    assert_eq!(f(1), 1);
}
