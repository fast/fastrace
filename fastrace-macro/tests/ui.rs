// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/err/*.rs");
    t.pass("tests/ui/ok/*.rs");
}
