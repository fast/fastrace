// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("../err/*.rs");
    t.pass("../ok/*.rs");
}
