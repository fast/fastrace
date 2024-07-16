#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/err/*.rs");
    t.pass("tests/ui/ok/*.rs");
}
