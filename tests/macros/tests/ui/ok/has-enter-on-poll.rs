// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use fastrace::trace;

#[trace(enter_on_poll = true)]
async fn f(a: u32) -> u32 {
    a
}

#[tokio::main]
async fn main() {
    f(1).await;
}
