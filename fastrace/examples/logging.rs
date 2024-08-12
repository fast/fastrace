// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::io::Write;

use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::prelude::*;
use log::info;

/// An example of automatically logging function arguments and return values.
#[logcall::logcall("debug", input = "a = {a:?}, b = {b:?}")]
#[trace]
fn plus(a: u64, b: u64) -> Result<u64, std::io::Error> {
    Ok(a + b)
}

fn main() {
    fastrace::set_reporter(ConsoleReporter, Config::default());

    // Setup a custom logger. `env_logger` is a commonly used logger in Rust and it's easy to
    // integrate with `fastrace`.
    //
    // For more fine-grained logging, We recommend using [`logforth`](https://github.com/cratesland/logforth).
    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            // Convert every log to an event in the current local parent span
            Event::add_to_local_parent(record.level().as_str(), || {
                [("message".into(), record.args().to_string().into())]
            });

            // Attach the current trace id to the log message
            if let Some(current) = SpanContext::current_local_parent() {
                writeln!(
                    buf,
                    "[{}] {} {}",
                    record.level(),
                    current.trace_id.0,
                    record.args()
                )
            } else {
                writeln!(buf, "[{}] {}", record.level(), record.args())
            }
        })
        .filter_level(log::LevelFilter::Debug)
        .init();

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
