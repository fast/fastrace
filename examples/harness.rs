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

//! This example shows how to write a test harness to set up fastrace.

use fastrace::prelude::*;
use test_harness::test;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test(harness = test_util::setup_fastrace)]
#[trace]
fn test_sync() -> Result<()> {
    std::thread::sleep(std::time::Duration::from_millis(50));
    Ok(())
}

#[test(harness = test_util::setup_fastrace_async)]
#[trace]
async fn test_async() -> Result<()> {
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(())
}

#[cfg(test)]
mod test_util {
    use fastrace::collector::Config;
    use fastrace::collector::ConsoleReporter;
    use fastrace::prelude::*;

    use super::*;

    pub fn setup_fastrace<F>(test: F)
    where F: FnOnce() -> Result<()> + 'static {
        fastrace::set_reporter(ConsoleReporter, Config::default());
        {
            let root = Span::root(closure_name::<F>(), SpanContext::random());
            let _guard = root.set_local_parent();
            test().expect("test success");
        }
        fastrace::flush();
    }

    pub fn setup_fastrace_async<F, Fut>(test: F)
    where
        F: FnOnce() -> Fut + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        fastrace::set_reporter(ConsoleReporter, Config::default());
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(3)
            .enable_all()
            .build()
            .unwrap();
        let root = Span::root(closure_name::<F>(), SpanContext::random());
        rt.block_on(test().in_span(root)).unwrap();
        fastrace::flush();
    }

    pub fn closure_name<F: std::any::Any>() -> &'static str {
        let func_path = std::any::type_name::<F>();
        func_path
            .rsplit("::")
            .find(|name| *name != "{{closure}}")
            .unwrap()
    }
}

fn main() {}
