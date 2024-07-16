// The libraries may have tracing instrument embedded in the code for tracing purposes. However,
// if the executable does not enable fastrace, it will be statically disabled. This results in
// zero overhead to the libraries, achieved through conditional compilation with the "enable"
// feature.
//
// The following test is designed to confirm that fastrace compiles when it's statically disabled
// in the executable.

use std::time::Duration;

use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;

fn main() {
    use fastrace::local::LocalCollector;
    use fastrace::prelude::*;

    fastrace::set_reporter(
        ConsoleReporter,
        Config::default()
            .max_spans_per_trace(Some(100))
            .report_interval(Duration::from_millis(10))
            .report_before_root_finish(true),
    );

    let mut root = Span::root("root", SpanContext::new(TraceId(0), SpanId(0)))
        .with_property(|| ("k1", "v1"))
        .with_properties(|| [("k2", "v2")]);

    Event::add_to_parent("event", &root, || []);
    Event::add_to_local_parent("event", || []);

    let _g = root.set_local_parent();

    Event::add_to_local_parent("event", || []);

    let _span1 = LocalSpan::enter_with_local_parent("span1")
        .with_property(|| ("k", "v"))
        .with_properties(|| [("k", "v")]);

    let _span2 = LocalSpan::enter_with_local_parent("span2");

    LocalSpan::add_property(|| ("k", "v"));
    LocalSpan::add_properties(|| [("k", "v")]);

    let local_collector = LocalCollector::start();
    let _ = LocalSpan::enter_with_local_parent("span3");
    let local_spans = local_collector.collect();
    assert_eq!(local_spans.to_span_records(SpanContext::random()), vec![]);

    let span3 = Span::enter_with_parent("span3", &root);
    let span4 = Span::enter_with_local_parent("span4");
    let span5 = Span::enter_with_parents("span5", [&root, &span3, &span4]);

    span5.push_child_spans(local_spans);

    assert!(SpanContext::current_local_parent().is_none());
    assert!(SpanContext::from_span(&span5).is_none());

    assert!(root.elapsed().is_none());

    root.cancel();

    fastrace::flush();
}
