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

use fastrace::Span;
use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::collector::SpanContext;
use fastrace::local::LocalSpan;
use logforth::append;
use logforth::diagnostic;
use logforth::layout;

#[logcall::logcall("debug")]
#[fastrace::trace]
fn plus(a: u64, b: u64) -> Result<u64, std::io::Error> {
    Ok(a + b)
}

fn main() {
    fastrace::set_reporter(ConsoleReporter, Config::default());

    // Set up a custom logger.
    //
    // Logforth (https://docs.rs/logforth/) is easy to start and integrated with Fastrace.
    logforth::starter_log::builder()
        .dispatch(|d| {
            d.diagnostic(diagnostic::FastraceDiagnostic::default())
                .append(append::Stderr::default().with_layout(layout::TextLayout::default()))
        })
        .dispatch(|d| d.append(append::FastraceEvent::default()))
        .apply();

    do_main();

    fastrace::flush();
}

fn do_main() {
    let parent = SpanContext::random();
    let root = Span::root("root", parent);
    let _span_guard = root.set_local_parent();

    log::info!("event in root span");

    let _local_span_guard = LocalSpan::enter_with_local_parent("child");

    log::info!("event in child span");

    plus(1, 2).unwrap();
}
