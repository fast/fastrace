// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::Arc;

use parking_lot::Mutex;

use crate::collector::global_collector::Reporter;
use crate::collector::SpanRecord;

pub struct TestReporter {
    pub spans: Arc<Mutex<Vec<SpanRecord>>>,
}

impl TestReporter {
    pub fn new() -> (Self, Arc<Mutex<Vec<SpanRecord>>>) {
        let spans = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                spans: spans.clone(),
            },
            spans,
        )
    }
}

impl Reporter for TestReporter {
    fn report(&mut self, mut spans: Vec<SpanRecord>) {
        self.spans.lock().append(&mut spans);
    }
}
