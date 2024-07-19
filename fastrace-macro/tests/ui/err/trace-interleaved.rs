// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use fastrace::trace;

#[allow(unused_braces)]
#[trace(struct)]
#[warn(unused_braces)]
fn f() {}

fn main() {}
