// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use fastrace::trace;

#[trace(enter_on_poll = true, properties = { "a": "b" })]
fn f() {}

fn main() {}
