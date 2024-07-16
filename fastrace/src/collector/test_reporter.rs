use std::sync::Arc;

use parking_lot::Mutex;

use super::global_collector::Reporter;
use super::SpanRecord;

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
    fn report(&mut self, spans: &[SpanRecord]) {
        self.spans.lock().extend_from_slice(spans);
    }
}
