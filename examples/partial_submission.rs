// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::Duration;

use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::prelude::*;

fn main() {
    fastrace::set_reporter(ConsoleReporter, Config::default());

    {
        let root = Span::root("long-running-operation", SpanContext::random());
        let _guard = root.set_local_parent();

        println!("Starting long-running operation...");
        
        // Simulate some work
        std::thread::sleep(Duration::from_millis(100));
        
        // Submit partial update to show progress
        println!("Submitting partial update #1...");
        root.submit_partial();
        
        // Do more work
        std::thread::sleep(Duration::from_millis(100));
        
        // Submit another partial update
        println!("Submitting partial update #2...");
        root.submit_partial();
        
        // Final work
        std::thread::sleep(Duration::from_millis(100));
        
        println!("Completing operation...");
        // Root span will be submitted when dropped
    }

    // Example with LocalSpan
    {
        let root = Span::root("batch-processing", SpanContext::random());
        let _guard = root.set_local_parent();

        println!("\nStarting batch processing...");
        
        for i in 1..=3 {
            let _span = LocalSpan::enter_with_local_parent(format!("batch-{}", i));
            
            // Simulate work
            std::thread::sleep(Duration::from_millis(50));
            
            // Submit partial update showing progress through batches
            println!("Submitting partial update for batch {}...", i);
            LocalSpan::submit_partial();
        }
        
        println!("Batch processing complete.");
    }

    fastrace::flush();
}
