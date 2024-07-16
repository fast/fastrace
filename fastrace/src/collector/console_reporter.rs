use crate::collector::global_collector::Reporter;
use crate::collector::SpanRecord;

/// A console reporter that prints span records to the stderr.
pub struct ConsoleReporter;

impl Reporter for ConsoleReporter {
    fn report(&mut self, spans: &[SpanRecord]) {
        for span in spans {
            eprintln!("{:#?}", span);
        }
    }
}
