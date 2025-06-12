// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::Duration;

use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::collector::TestReporter;
use fastrace::local::LocalCollector;
use fastrace::prelude::*;
use fastrace::util::tree::tree_str_from_span_records;
use serial_test::serial;
use tokio::runtime::Builder;

fn four_spans() {
    {
        // wide
        for i in 0..2 {
            let _span = LocalSpan::enter_with_local_parent(format!("iter-span-{i}"))
                .with_property(|| ("tmp_property", "tmp_value"));
        }
    }

    {
        #[trace(name = "rec-span")]
        fn rec(mut i: u32) {
            i -= 1;

            if i > 0 {
                rec(i);
            }
        }

        // deep
        rec(2);
    }
}

#[test]
#[serial]
fn single_thread_single_span() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::random());
        let _g = root.set_local_parent();

        four_spans();
    };

    fastrace::flush();

    let graph = tree_str_from_span_records(collected_spans.lock().clone());
    insta::assert_snapshot!(graph, @r###"
    root []
        iter-span-0 [("tmp_property", "tmp_value")]
        iter-span-1 [("tmp_property", "tmp_value")]
        rec-span []
            rec-span []
    "###);
}

#[test]
#[serial]
fn single_thread_multiple_spans() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root1 = Span::root("root1", SpanContext::new(TraceId(12), SpanId(0)));
        let root2 = Span::root("root2", SpanContext::new(TraceId(13), SpanId(0)));
        let root3 = Span::root("root3", SpanContext::new(TraceId(14), SpanId(0)));

        let local_collector = LocalCollector::start();

        four_spans();

        let local_spans = local_collector.collect();

        root1.push_child_spans(local_spans.clone());
        root2.push_child_spans(local_spans.clone());
        root3.push_child_spans(local_spans);
    }

    fastrace::flush();

    let graph1 = tree_str_from_span_records(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(12))
            .cloned()
            .collect(),
    );
    insta::assert_snapshot!(graph1, @r###"
    root1 []
        iter-span-0 [("tmp_property", "tmp_value")]
        iter-span-1 [("tmp_property", "tmp_value")]
        rec-span []
            rec-span []
    "###);

    let graph2 = tree_str_from_span_records(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(13))
            .cloned()
            .collect(),
    );
    insta::assert_snapshot!(graph2, @r###"
    root2 []
        iter-span-0 [("tmp_property", "tmp_value")]
        iter-span-1 [("tmp_property", "tmp_value")]
        rec-span []
            rec-span []
    "###);

    let graph3 = tree_str_from_span_records(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(14))
            .cloned()
            .collect(),
    );
    insta::assert_snapshot!(graph3, @r###"
    root3 []
        iter-span-0 [("tmp_property", "tmp_value")]
        iter-span-1 [("tmp_property", "tmp_value")]
        rec-span []
            rec-span []
    "###);
}

#[test]
#[serial]
fn multiple_threads_single_span() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    crossbeam::scope(|scope| {
        let root = Span::root("root", SpanContext::random());
        let _g = root.set_local_parent();

        let mut handles = vec![];

        for _ in 0..4 {
            let child_span = Span::enter_with_local_parent("cross-thread");
            let h = scope.spawn(move |_| {
                let _g = child_span.set_local_parent();
                four_spans();
            });
            handles.push(h);
        }

        four_spans();

        handles.into_iter().for_each(|h| h.join().unwrap());
    })
    .unwrap();

    fastrace::flush();

    let graph = tree_str_from_span_records(collected_spans.lock().clone());
    insta::assert_snapshot!(graph, @r###"
    root []
        cross-thread []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            rec-span []
                rec-span []
        cross-thread []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            rec-span []
                rec-span []
        cross-thread []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            rec-span []
                rec-span []
        cross-thread []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            rec-span []
                rec-span []
        iter-span-0 [("tmp_property", "tmp_value")]
        iter-span-1 [("tmp_property", "tmp_value")]
        rec-span []
            rec-span []
    "###);
}

#[test]
#[serial]
fn multiple_threads_multiple_spans() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    crossbeam::scope(|scope| {
        let root1 = Span::root("root1", SpanContext::new(TraceId(12), SpanId(0)));
        let root2 = Span::root("root2", SpanContext::new(TraceId(13), SpanId(0)));
        let local_collector = LocalCollector::start();

        let mut handles = vec![];

        for _ in 0..4 {
            let merged = Span::enter_with_parents("merged", vec![&root1, &root2]);
            let _g = merged.set_local_parent();
            let _local = LocalSpan::enter_with_local_parent("local");
            let h = scope.spawn(move |_| {
                let local_collector = LocalCollector::start();

                four_spans();

                let local_spans = local_collector.collect();
                merged.push_child_spans(local_spans);
            });

            handles.push(h);
        }

        four_spans();

        handles.into_iter().for_each(|h| h.join().unwrap());

        let local_spans = local_collector.collect();
        root1.push_child_spans(local_spans.clone());
        root2.push_child_spans(local_spans);
    })
    .unwrap();

    fastrace::flush();

    let graph1 = tree_str_from_span_records(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(12))
            .cloned()
            .collect(),
    );
    insta::assert_snapshot!(graph1, @r###"
    root1 []
        iter-span-0 [("tmp_property", "tmp_value")]
        iter-span-1 [("tmp_property", "tmp_value")]
        merged []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            local []
            rec-span []
                rec-span []
        merged []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            local []
            rec-span []
                rec-span []
        merged []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            local []
            rec-span []
                rec-span []
        merged []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            local []
            rec-span []
                rec-span []
        rec-span []
            rec-span []
    "###);

    let graph2 = tree_str_from_span_records(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(13))
            .cloned()
            .collect(),
    );
    insta::assert_snapshot!(graph2, @r###"
    root2 []
        iter-span-0 [("tmp_property", "tmp_value")]
        iter-span-1 [("tmp_property", "tmp_value")]
        merged []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            local []
            rec-span []
                rec-span []
        merged []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            local []
            rec-span []
                rec-span []
        merged []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            local []
            rec-span []
                rec-span []
        merged []
            iter-span-0 [("tmp_property", "tmp_value")]
            iter-span-1 [("tmp_property", "tmp_value")]
            local []
            rec-span []
                rec-span []
        rec-span []
            rec-span []
    "###);
}

#[test]
#[serial]
fn multiple_spans_without_local_spans() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default().tail_sampled(true));

    {
        let root1 = Span::root("root1", SpanContext::new(TraceId(12), SpanId::default()));
        let root2 = Span::root("root2", SpanContext::new(TraceId(13), SpanId::default()));
        let root3 = Span::root("root3", SpanContext::new(TraceId(14), SpanId::default()));

        let local_collector = LocalCollector::start();

        let local_spans = local_collector.collect();
        root1.push_child_spans(local_spans.clone());
        root2.push_child_spans(local_spans.clone());
        root3.push_child_spans(local_spans);

        root3.cancel();
    }

    fastrace::flush();

    assert_eq!(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(12))
            .count(),
        1
    );
    assert_eq!(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(13))
            .count(),
        1
    );
    assert_eq!(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(14))
            .count(),
        0
    );
}

#[test]
#[serial]
fn test_macro() {
    use async_trait::async_trait;

    #[async_trait]
    trait Foo {
        async fn run(&self, millis: &u64);
    }

    struct Bar;

    #[async_trait]
    impl Foo for Bar {
        #[trace(name = "run")]
        async fn run(&self, millis: &u64) {
            let _g = Span::enter_with_local_parent("run-inner");
            work(millis).await;
            let _g = LocalSpan::enter_with_local_parent("local-span");
        }
    }

    #[trace(short_name = true, enter_on_poll = true)]
    async fn work(millis: &u64) {
        let _g = Span::enter_with_local_parent("work-inner");
        tokio::time::sleep(Duration::from_millis(*millis))
            .enter_on_poll("sleep")
            .await;
    }

    impl Bar {
        #[trace(short_name = true)]
        async fn work2(&self, millis: &u64) {
            let _g = Span::enter_with_local_parent("work-inner");
            tokio::time::sleep(Duration::from_millis(*millis))
                .enter_on_poll("sleep")
                .await;
        }
    }

    #[trace(short_name = true)]
    async fn work3(millis1: &u64, millis2: &u64) {
        let _g = Span::enter_with_local_parent("work-inner");
        tokio::time::sleep(Duration::from_millis(*millis1))
            .enter_on_poll("sleep")
            .await;
        tokio::time::sleep(Duration::from_millis(*millis2))
            .enter_on_poll("sleep")
            .await;
    }

    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::random());

        let runtime = Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();

        pollster::block_on(
            runtime.spawn(
                async {
                    Bar.run(&100).await;
                    Bar.work2(&100).await;
                    work3(&100, &100).await;
                }
                .in_span(root),
            ),
        )
        .unwrap();
    }

    fastrace::flush();

    let graph = tree_str_from_span_records(collected_spans.lock().clone());
    insta::assert_snapshot!(graph, @r###"
    root []
        run []
            local-span []
            run-inner []
            work []
                sleep []
            work []
                sleep []
                work-inner []
        work2 []
            sleep []
            sleep []
            work-inner []
        work3 []
            sleep []
            sleep []
            sleep []
            sleep []
            work-inner []
    "###);
}

#[test]
#[serial]
fn macro_example() {
    #[trace(short_name = true)]
    fn do_something_short_name(i: u64) {
        std::thread::sleep(Duration::from_millis(i));
    }

    #[trace(short_name = true)]
    async fn do_something_async_short_name(i: u64) {
        futures_timer::Delay::new(Duration::from_millis(i)).await;
    }

    #[trace]
    fn do_something(i: u64) {
        std::thread::sleep(Duration::from_millis(i));
    }

    #[trace]
    async fn do_something_async(i: u64) {
        futures_timer::Delay::new(Duration::from_millis(i)).await;
    }

    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::random());
        let _g = root.set_local_parent();
        do_something(100);
        pollster::block_on(do_something_async(100));
        do_something_short_name(100);
        pollster::block_on(do_something_async_short_name(100));
    }

    fastrace::flush();

    let graph = tree_str_from_span_records(collected_spans.lock().clone());
    insta::assert_snapshot!(graph,@r###"
    root []
        do_something_async_short_name []
        do_something_short_name []
        lib::macro_example::{{closure}}::do_something []
        lib::macro_example::{{closure}}::do_something_async::{{closure}} []
    "###);
}

#[test]
#[serial]
fn multiple_local_parent() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::random());
        let _g = root.set_local_parent();
        let _g = LocalSpan::enter_with_local_parent("span1");
        let span2 = Span::enter_with_local_parent("span2");
        {
            let _g = span2.set_local_parent();
            let _g = LocalSpan::enter_with_local_parent("span3");
        }
        let _g = LocalSpan::enter_with_local_parent("span4");
    }

    fastrace::flush();

    let graph = tree_str_from_span_records(collected_spans.lock().clone());
    insta::assert_snapshot!(graph, @r###"
    root []
        span1 []
            span2 []
                span3 []
            span4 []
    "###);
}

#[test]
#[serial]
fn early_local_collect() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let local_collector = LocalCollector::start();
        let _g1 = LocalSpan::enter_with_local_parent("span1");
        let _g2 = LocalSpan::enter_with_local_parent("span2");
        drop(_g2);
        let local_spans = local_collector.collect();

        let root = Span::root("root", SpanContext::random());
        root.push_child_spans(local_spans);
    }

    fastrace::flush();

    let graph = tree_str_from_span_records(collected_spans.lock().clone());
    insta::assert_snapshot!(graph, @r###"
    root []
        span1 []
            span2 []
    "###);
}

#[test]
#[serial]
fn test_elapsed() {
    fastrace::set_reporter(ConsoleReporter, Config::default());

    {
        let root = Span::root("root", SpanContext::random());

        std::thread::sleep(Duration::from_millis(50));

        assert!(root.elapsed().unwrap() >= Duration::from_millis(50));
    }

    fastrace::flush();
}

#[test]
#[serial]
fn test_property() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::random())
            .with_property(|| ("k1", "v1"))
            .with_properties(|| [("k2", "v2"), ("k3", "v3")]);
        root.add_property(|| ("k4", "v4"));
        root.add_properties(|| [("k5", "v5"), ("k6", "v6")]);
        let _g = root.set_local_parent();
        LocalSpan::add_property(|| ("k7", "v7"));
        LocalSpan::add_properties(|| [("k8", "v8"), ("k9", "v9")]);
        let _span = LocalSpan::enter_with_local_parent("span")
            .with_property(|| ("k10", "v10"))
            .with_properties(|| [("k11", "v11"), ("k12", "v12")]);
        LocalSpan::add_property(|| ("k13", "v13"));
        LocalSpan::add_properties(|| [("k14", "v14"), ("k15", "v15")]);
    }

    fastrace::flush();

    let graph = tree_str_from_span_records(collected_spans.lock().clone());
    insta::assert_snapshot!(graph, @r###"
    root [("k1", "v1"), ("k2", "v2"), ("k3", "v3"), ("k4", "v4"), ("k5", "v5"), ("k6", "v6"), ("k7", "v7"), ("k8", "v8"), ("k9", "v9")]
        span [("k10", "v10"), ("k11", "v11"), ("k12", "v12"), ("k13", "v13"), ("k14", "v14"), ("k15", "v15")]
    "###);
}

#[test]
#[serial]
fn test_event() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::random());
        root.add_event(
            Event::new("event1 in root")
                .with_property(|| ("k1", "v1"))
                .with_properties(|| [("k2", "v2"), ("k3", "v3")]),
        );
        let _g = root.set_local_parent();
        LocalSpan::add_event(
            Event::new("event2 in root")
                .with_property(|| ("k4", "v4"))
                .with_properties(|| [("k5", "v5"), ("k6", "v6")]),
        );
        let _span = LocalSpan::enter_with_local_parent("span");
        LocalSpan::add_event(
            Event::new("event3 in span")
                .with_property(|| ("k7", "v7"))
                .with_properties(|| [("k8", "v8"), ("k9", "v9")]),
        );
    }

    fastrace::flush();

    let graph = tree_str_from_span_records(collected_spans.lock().clone());
    insta::assert_snapshot!(graph, @r###"
    root [] [("event1 in root", [("k1", "v1"), ("k2", "v2"), ("k3", "v3")]), ("event2 in root", [("k4", "v4"), ("k5", "v5"), ("k6", "v6")])]
        span [] [("event3 in span", [("k7", "v7"), ("k8", "v8"), ("k9", "v9")])]
    "###);
}

#[test]
#[serial]
fn test_macro_properties() {
    #[allow(clippy::drop_non_drop)]
    #[trace(short_name = true, properties = { "k1": "v1", "a": "argument a is {a:?}", "b": "{b:?}", "escaped1": "{c:?}{{}}", "escaped2": "{{ \"a\": \"b\"}}" })]
    fn foo(a: i64, b: &Bar, c: Bar) {
        drop(c);
    }

    #[allow(clippy::drop_non_drop)]
    #[trace(short_name = true, properties = { "k1": "v1", "a": "argument a is {a:?}", "b": "{b:?}", "escaped1": "{c:?}{{}}", "escaped2": "{{ \"a\": \"b\"}}" })]
    async fn foo_async(a: i64, b: &Bar, c: Bar) {
        drop(c);
    }

    #[trace(short_name = true, properties = {})]
    fn bar() {}

    #[trace(short_name = true, properties = {})]
    async fn bar_async() {}

    #[derive(Debug)]
    struct Bar;

    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::random());
        let _g = root.set_local_parent();
        foo(1, &Bar, Bar);
        bar();

        let runtime = Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();

        pollster::block_on(
            runtime.spawn(
                async {
                    foo_async(1, &Bar, Bar).await;
                    bar_async().await;
                }
                .in_span(root),
            ),
        )
        .unwrap();
    }

    fastrace::flush();

    let graph = tree_str_from_span_records(collected_spans.lock().clone());
    insta::assert_snapshot!(graph, @r###"
    root []
        bar []
        bar_async []
        foo [("k1", "v1"), ("a", "argument a is 1"), ("b", "Bar"), ("escaped1", "Bar{}"), ("escaped2", "{ \"a\": \"b\"}")]
        foo_async [("k1", "v1"), ("a", "argument a is 1"), ("b", "Bar"), ("escaped1", "Bar{}"), ("escaped2", "{ \"a\": \"b\"}")]
    "###);
}

#[test]
#[serial]
fn test_not_sampled() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());
    {
        let root = Span::root("root", SpanContext::random().sampled(true));
        let _g = root.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent("span");
    }
    fastrace::flush();

    let graph = tree_str_from_span_records(collected_spans.lock().clone());
    insta::assert_snapshot!(graph,@r###"
    root []
        span []
    "###);

    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());
    {
        let root = Span::root("root", SpanContext::random().sampled(false));
        let _g = root.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent("span");
    }
    fastrace::flush();
    assert!(collected_spans.lock().is_empty());
}
