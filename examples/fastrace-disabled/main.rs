// Copyright 2024 FastLabs Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! The libraries may have tracing instrument embedded in the code for tracing purposes.
//!
//! However, if the executable does not enable fastrace, it will be statically disabled.
//! This results in zero overhead to libraries, achieved through conditional compilation with
//! the "enable" feature.
//!
//! The following test is designed to confirm that fastrace do compile when it's statically
//! disabled.

use std::time::Duration;

use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;

fn main() {
    use fastrace::local::LocalCollector;
    use fastrace::prelude::*;

    fastrace::set_reporter(
        ConsoleReporter,
        Config::default().report_interval(Duration::from_millis(10)),
    );

    let root = Span::root("root", SpanContext::new(TraceId(0), SpanId(0)))
        .with_property(|| ("k0", "v0"))
        .with_properties(|| [("k1", "v1")])
        .with_link(SpanContext::new(TraceId(1), SpanId(1)));

    root.add_property(|| ("k1.5", "v1.5"));
    root.add_properties(|| [("k2", "v2")]);
    root.add_event(
        Event::new("event")
            .with_property(|| ("k0", "v0"))
            .with_properties(|| [("k1", "v1")]),
    );
    root.add_link(SpanContext::new(TraceId(1), SpanId(1)));

    let _g = root.set_local_parent();

    LocalSpan::add_event(
        Event::new("event")
            .with_property(|| ("k0", "v0"))
            .with_properties(|| [("k1", "v1")]),
    );

    let _span1 = LocalSpan::enter_with_local_parent("span1").with_property(|| ("k0", "v0"));
    let _span2 = LocalSpan::enter_with_local_parent("span2")
        .with_property(|| ("k1", "v1"))
        .with_properties(|| [("k", "v")]);
    let _span3 = LocalSpan::enter_with_local_parent("span3")
        .with_link(SpanContext::new(TraceId(1), SpanId(1)));

    LocalSpan::add_property(|| ("k0", "v0"));
    LocalSpan::add_properties(|| [("k", "v")]);
    LocalSpan::add_link(SpanContext::new(TraceId(1), SpanId(1)));

    let local_collector = LocalCollector::start();
    let _ = LocalSpan::enter_with_local_parent("span3");
    let local_spans = local_collector.collect();
    assert_eq!(local_spans.to_span_records(SpanContext::random()), vec![]);

    let span3 = Span::enter_with_parent("span3", &root);
    let span4 = Span::enter_with_local_parent("span4");

    span4.push_child_spans(local_spans);

    assert!(SpanContext::current_local_parent().is_none());
    assert!(SpanContext::from_span(&span3).is_none());
    assert!(SpanContext::from_span(&span4).is_none());

    assert!(root.elapsed().is_none());

    let root = root;
    root.cancel();

    fastrace::flush();
}
