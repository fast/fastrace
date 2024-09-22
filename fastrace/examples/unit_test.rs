// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![allow(dead_code)]
#![allow(unused_imports)]

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
    where
        F: FnOnce() -> Result<()> + 'static,
    {
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
