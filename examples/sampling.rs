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

use std::time::Duration;

use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::prelude::*;

fn main() {
    fastrace::set_reporter(ConsoleReporter, Config::default());

    lightweight_task();
    heavy_task();

    fastrace::flush();
}

fn lightweight_task() {
    let parent = SpanContext::random();
    let root = Span::root("lightweight work", parent);
    let _span_guard = root.set_local_parent();

    expensive_task(Duration::from_millis(1));

    // Cancel the trace to avoid reporting if it's too short.
    if root.elapsed() < Some(Duration::from_millis(100)) {
        root.cancel();
    }
}

fn heavy_task() {
    let parent = SpanContext::random();
    let root = Span::root("heavy work", parent);
    let _span_guard = root.set_local_parent();

    expensive_task(Duration::from_secs(1));

    // This trace will be reported.
    if root.elapsed() < Some(Duration::from_millis(100)) {
        root.cancel();
    }
}

#[trace]
fn expensive_task(time: Duration) {
    std::thread::sleep(time);
}
