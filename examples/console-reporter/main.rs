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

use fastrace::Span;
use fastrace::collector::Config;
use fastrace::collector::ConsoleReporter;
use fastrace::collector::SpanContext;
use fastrace::local::LocalSpan;

fn func1(i: u64) {
    let _guard = LocalSpan::enter_with_local_parent("func1");
    std::thread::sleep(Duration::from_millis(i));
    func2(i);
}

#[fastrace::trace]
fn func2(i: u64) {
    std::thread::sleep(Duration::from_millis(i));
}

fn main() {
    fastrace::set_reporter(ConsoleReporter, Config::default());

    {
        let parent = SpanContext::random();
        let root = Span::root("root", parent);

        let _g = root.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent("biz-span")
            .with_property(|| ("biz-property", "biz-value"));

        for i in 1..=10 {
            func1(i);
        }
    }

    fastrace::flush();
}
