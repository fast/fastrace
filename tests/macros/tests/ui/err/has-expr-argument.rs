// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use fastrace::trace;

#[trace(true)]
fn f() {}

fn main() {}
