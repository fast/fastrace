// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::prelude::*;

fn main() {
    fastrace::set_reporter(ConsoleReporter, Config::default());

    {
        let parent = SpanContext::random();
        let root = Span::root("root", parent);
        let _g = root.set_local_parent();
        let _g = LocalSpan::enter_with_local_parent("child");

        // do something ...
    }

    fastrace::flush();
}
