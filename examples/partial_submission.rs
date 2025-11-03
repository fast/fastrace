use std::time::Duration;

use fastrace::Span;
use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::prelude::*;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    // Initialize fastrace with a ConsoleReporter
    fastrace::set_reporter(
        ConsoleReporter,
        Config::default().report_interval(Duration::from_millis(100)),
    );

    println!("--- Synchronous Partial Submission Example ---");
    sync_partial_submission();
    fastrace::flush(); // Flush to ensure synchronous spans are reported

    println!(
        "
--- Asynchronous Partial Submission Example ---"
    );
    async_partial_submission().await;
    fastrace::flush(); // Flush to ensure asynchronous spans are reported

    // Give some time for the reporter to process and print
    sleep(Duration::from_millis(200)).await;
}

fn sync_partial_submission() {
    let root_span = Span::root("sync_root", SpanContext::random());
    root_span.add_property(|| ("stage", "start"));

    // Simulate some work
    std::thread::sleep(Duration::from_millis(50));
    root_span.add_property(|| ("progress", "25%"));
    root_span.submit_partial(); // Submit partial data

    std::thread::sleep(Duration::from_millis(50));
    root_span.add_property(|| ("progress", "50%"));
    root_span.submit_partial(); // Submit another partial data

    let child_span = Span::enter_with_parent("sync_child", &root_span);
    std::thread::sleep(Duration::from_millis(30));
    child_span.add_property(|| ("child_status", "done"));
    child_span.submit_partial(); // Partial submission for child

    std::thread::sleep(Duration::from_millis(50));
    root_span.add_property(|| ("stage", "complete"));
    // root_span and child_span will be fully submitted when they drop
}

async fn async_partial_submission() {
    let root_span = Span::root("async_root", SpanContext::random());
    let _guard = root_span.set_local_parent(); // Set local parent for LocalSpans

    tokio::task::LocalSet::new()
        .run_until(async move {
            tokio::task::spawn_local(async move {
                let _span1 = LocalSpan::enter_with_local_parent("async_task_1");
                sleep(Duration::from_millis(50)).await;
                LocalSpan::add_property(|| ("step", "1"));
                LocalSpan::submit_partial(); // Submit partial data for local spans

                sleep(Duration::from_millis(50)).await;
                let _span2 = LocalSpan::enter_with_local_parent("async_task_2");
                sleep(Duration::from_millis(30)).await;
                LocalSpan::add_property(|| ("step", "2"));
                LocalSpan::submit_partial(); // Submit another partial data for local spans

                sleep(Duration::from_millis(50)).await;
                LocalSpan::add_property(|| ("status", "finished"));
                // LocalSpans will be fully submitted when _guard drops
            })
            .await
            .unwrap();
        })
        .await;
}
