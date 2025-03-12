// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::Duration;

use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::collector::TestReporter;
use fastrace::local::LocalCollector;
use fastrace::prelude::*;
use fastrace::util::tree::tree_str_from_span_records;
use pollster::block_on;
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

    let expected_graph = r#"
root []
    iter-span-0 [("tmp_property", "tmp_value")]
    iter-span-1 [("tmp_property", "tmp_value")]
    rec-span []
        rec-span []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
}

#[test]
#[serial]
fn single_thread_multiple_spans() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root1 = Span::root("root1", SpanContext::new(TraceId(12), SpanId::default()));
        let root2 = Span::root("root2", SpanContext::new(TraceId(13), SpanId::default()));
        let root3 = Span::root("root3", SpanContext::new(TraceId(14), SpanId::default()));

        let local_collector = LocalCollector::start();

        four_spans();

        let local_spans = local_collector.collect();

        root1.push_child_spans(local_spans.clone());
        root2.push_child_spans(local_spans.clone());
        root3.push_child_spans(local_spans);
    }

    fastrace::flush();

    let expected_graph1 = r#"
root1 []
    iter-span-0 [("tmp_property", "tmp_value")]
    iter-span-1 [("tmp_property", "tmp_value")]
    rec-span []
        rec-span []
"#;
    let expected_graph2 = r#"
root2 []
    iter-span-0 [("tmp_property", "tmp_value")]
    iter-span-1 [("tmp_property", "tmp_value")]
    rec-span []
        rec-span []
"#;
    let expected_graph3 = r#"
root3 []
    iter-span-0 [("tmp_property", "tmp_value")]
    iter-span-1 [("tmp_property", "tmp_value")]
    rec-span []
        rec-span []
"#;
    assert_eq!(
        tree_str_from_span_records(
            collected_spans
                .lock()
                .iter()
                .filter(|s| s.trace_id == TraceId(12))
                .cloned()
                .collect()
        ),
        expected_graph1
    );
    assert_eq!(
        tree_str_from_span_records(
            collected_spans
                .lock()
                .iter()
                .filter(|s| s.trace_id == TraceId(13))
                .cloned()
                .collect()
        ),
        expected_graph2
    );
    assert_eq!(
        tree_str_from_span_records(
            collected_spans
                .lock()
                .iter()
                .filter(|s| s.trace_id == TraceId(14))
                .cloned()
                .collect()
        ),
        expected_graph3
    );
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

    let expected_graph = r#"
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
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
}

#[test]
#[serial]
fn multiple_threads_multiple_spans() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    crossbeam::scope(|scope| {
        let root1 = Span::root("root1", SpanContext::new(TraceId(12), SpanId::default()));
        let root2 = Span::root("root2", SpanContext::new(TraceId(13), SpanId::default()));
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

    let expected_graph1 = r#"
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
"#;
    let expected_graph2 = r#"
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
"#;
    assert_eq!(
        tree_str_from_span_records(
            collected_spans
                .lock()
                .iter()
                .filter(|s| s.trace_id == TraceId(12))
                .cloned()
                .collect()
        ),
        expected_graph1
    );
    assert_eq!(
        tree_str_from_span_records(
            collected_spans
                .lock()
                .iter()
                .filter(|s| s.trace_id == TraceId(13))
                .cloned()
                .collect()
        ),
        expected_graph2
    );
}

#[test]
#[serial]
fn multiple_spans_without_local_spans() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root1 = Span::root("root1", SpanContext::new(TraceId(12), SpanId::default()));
        let root2 = Span::root("root2", SpanContext::new(TraceId(13), SpanId::default()));
        let mut root3 = Span::root("root3", SpanContext::new(TraceId(14), SpanId::default()));

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
    async fn work3<'a>(millis1: &'a u64, millis2: &u64) {
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

        block_on(
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

    let expected_graph = r#"
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
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
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
        block_on(do_something_async(100));
        do_something_short_name(100);
        block_on(do_something_async_short_name(100));
    }

    fastrace::flush();

    let expected_graph = r#"
root []
    do_something_async_short_name []
    do_something_short_name []
    lib::macro_example::{{closure}}::do_something []
    lib::macro_example::{{closure}}::do_something_async []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
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

    let expected_graph = r#"
root []
    span1 []
        span2 []
            span3 []
        span4 []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
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

    let expected_graph = r#"
root []
    span1 []
        span2 []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
}

#[test]
#[serial]
fn max_spans_per_trace() {
    #[trace(short_name = true)]
    fn recursive(n: usize) {
        if n > 1 {
            recursive(n - 1);
        }
    }

    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default().max_spans_per_trace(Some(5)));

    {
        let root = Span::root("root", SpanContext::random());

        {
            let _g = root.set_local_parent();
            recursive(3);
        }
        {
            let _g = root.set_local_parent();
            recursive(3);
        }
        {
            let _g = root.set_local_parent();
            recursive(3);
        }
        {
            let _g = root.set_local_parent();
            recursive(3);
        }
    }

    fastrace::flush();

    let expected_graph = r#"
root []
    recursive []
        recursive []
            recursive []
    recursive []
        recursive []
            recursive []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
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
fn test_add_property() {
    let (reporter, collected_spans) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::random());
        let _g = root.set_local_parent();
        LocalSpan::add_property(|| ("k1", "v1"));
        LocalSpan::add_properties(|| [("k2", "v2")]);
        let _span = LocalSpan::enter_with_local_parent("span");
        LocalSpan::add_property(|| ("k3", "v3"));
        LocalSpan::add_properties(|| [("k4", "v4"), ("k5", "v5")]);
    }

    fastrace::flush();

    let expected_graph = r#"
root [("k1", "v1"), ("k2", "v2")]
    span [("k3", "v3"), ("k4", "v4"), ("k5", "v5")]
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
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

        block_on(
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

    let expected_graph = r#"
root []
    bar []
    bar_async []
    foo [("k1", "v1"), ("a", "argument a is 1"), ("b", "Bar"), ("escaped1", "Bar{}"), ("escaped2", "{ \"a\": \"b\"}")]
    foo_async [("k1", "v1"), ("a", "argument a is 1"), ("b", "Bar"), ("escaped1", "Bar{}"), ("escaped2", "{ \"a\": \"b\"}")]
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
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
    let expected_graph = r#"
root []
    span []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );

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
