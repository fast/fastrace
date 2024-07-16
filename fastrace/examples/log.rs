use std::io::Write;

use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::prelude::*;
use log::info;

#[logcall::logcall("debug")]
#[trace]
fn plus(a: u64, b: u64) -> Result<u64, std::io::Error> {
    Ok(a + b)
}

fn main() {
    fastrace::set_reporter(ConsoleReporter, Config::default());
    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            // Add a event to the current local span representing the log record
            Event::add_to_local_parent(record.level().as_str(), || {
                [("message".into(), record.args().to_string().into())]
            });

            // Output the log to stdout as usual
            writeln!(buf, "[{}] {}", record.level(), record.args())
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
