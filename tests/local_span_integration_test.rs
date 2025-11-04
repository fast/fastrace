use fastrace::prelude::{Span, SpanContext};
use fastrace::local::LocalSpan;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use fastrace::collector::{Reporter, Config, SpanRecord};

struct TestReporter {
    reported_spans: Arc<Mutex<Vec<fastrace::collector::SpanRecord>>>,
}

impl fastrace::collector::Reporter for TestReporter {
    fn report(&mut self, spans: Vec<fastrace::collector::SpanRecord>) {
        self.reported_spans.lock().unwrap().extend(spans);
    }
}

#[tokio::test]
async fn async_local_span_submit_partial() {
    let reported_spans = Arc::new(Mutex::new(Vec::new()));
    let reporter = TestReporter { reported_spans: reported_spans.clone() };

    fastrace::set_reporter(reporter, fastrace::collector::Config::default());

    let root = Span::root("async_root", SpanContext::random());
    let _guard = root.set_local_parent();

    tokio::task::LocalSet::new()
        .run_until(async move {
            tokio::task::spawn_local(async move {
                let _span1 = LocalSpan::enter_with_local_parent("async_task_1");
                sleep(Duration::from_millis(50)).await;
                LocalSpan::add_property(|| ("step", "1"));
                LocalSpan::submit_partial();
                sleep(Duration::from_millis(50)).await;
                let _span2 = LocalSpan::enter_with_local_parent("async_task_2");
                sleep(Duration::from_millis(30)).await;
                LocalSpan::add_property(|| ("step", "2"));
                LocalSpan::submit_partial();
                sleep(Duration::from_millis(50)).await;
                LocalSpan::add_property(|| ("status", "finished"));
            })
            .await
            .unwrap();
        })
        .await;

    drop(root); // Explicitly drop root to trigger its submission

    let submitted = reported_spans.lock().unwrap();
    // Expect 3 spans: root, async_task_1, async_task_2
    assert_eq!(submitted.len(), 3);

    // Verify properties of the submitted spans
    let root_span = submitted.iter().find(|s| s.name == "async_root").unwrap();
    assert!(root_span.properties.iter().any(|(k, v)| k == "status" && v == "finished"));

    let task1_span = submitted.iter().find(|s| s.name == "async_task_1").unwrap();
    assert!(task1_span.properties.iter().any(|(k, v)| k == "step" && v == "1"));

    let task2_span = submitted.iter().find(|s| s.name == "async_task_2").unwrap();
    assert!(task2_span.properties.iter().any(|(k, v)| k == "step" && v == "2"));
}