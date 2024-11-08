// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::prelude::*;
use log::info;
use logforth::append;
use logforth::filter::EnvFilter;

/// An example of automatically logging function arguments and return values.
#[logcall::logcall("debug", input = "a = {a:?}, b = {b:?}")]
#[trace]
fn plus(a: u64, b: u64) -> Result<u64, std::io::Error> {
    Ok(a + b)
}

fn main() {
    fastrace::set_reporter(ConsoleReporter, Config::default());

    // Set up a custom logger. [`logforth`](https://github.com/fast/logforth)
    // is easy to start and integrated with `fastrace`.
    logforth::builder()
        .dispatch(|d| {
            d.filter(EnvFilter::from_default_env())
                .append(append::Stderr::default())
        })
        .dispatch(|d| d.append(append::FastraceEvent::default()))
        .apply();

    {
        let parent = SpanContext::random();
        let root = Span::root("root", parent);
        let _span_guard = root.set_local_parent();

        info!("event in root span");

        let _local_span_guard = LocalSpan::enter_with_local_parent("child");

        info!("event in child span");

        plus(1, 2).unwrap();
    };

    fastrace::flush();
}
