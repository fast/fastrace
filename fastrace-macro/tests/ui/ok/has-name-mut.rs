// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use fastrace::trace;

#[trace(name = "test-span")]
fn f(a: u32) -> u32 {
    a
}

fn main() {
    f(1);
}
