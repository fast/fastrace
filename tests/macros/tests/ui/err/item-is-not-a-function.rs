// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use fastrace::trace;

#[trace("test-span")]
struct S;

fn main() {}
