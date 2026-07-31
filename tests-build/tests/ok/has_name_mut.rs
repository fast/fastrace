use fastrace::trace;

#[trace(name = "test-span")]
fn f(mut a: u32) -> u32 {
    a += 1;
    a
}

#[test]
fn test() {
    assert_eq!(f(1), 2);
}
