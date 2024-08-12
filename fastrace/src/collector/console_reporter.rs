// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::global_collector::Reporter;
use crate::collector::SpanRecord;

/// A console reporter that prints span records to the stderr.
pub struct ConsoleReporter;

impl Reporter for ConsoleReporter {
    fn report(&mut self, spans: Vec<SpanRecord>) {
        for span in spans {
            eprintln!("{:#?}", span);
        }
    }
}
