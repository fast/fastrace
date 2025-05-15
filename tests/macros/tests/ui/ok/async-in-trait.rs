// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

trait MyTrait {
    async fn work(&self) -> usize;
}

struct MyStruct;

impl MyTrait for MyStruct {
    // #[logcall::logcall("info")]
    #[fastrace::trace]
    async fn work(&self) -> usize {
        unimplemented!()
    }
}

fn main() {}
