// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use fastrace::trace;

#[trace(name = "Name", short_name = true)]
fn f() {}

fn main() {}
