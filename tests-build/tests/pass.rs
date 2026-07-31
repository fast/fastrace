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

#![deny(warnings)]

use std::future::Future;
use std::pin::Pin;

use async_stream::stream;
use async_trait::async_trait;
use custom_fastrace::trace as custom_trace;
use fastrace as custom_fastrace;
use fastrace::trace;
use futures::Stream;

trait NativeTrait {
    async fn work(&self) -> usize;
}

struct Native;

impl NativeTrait for Native {
    #[trace]
    async fn work(&self) -> usize {
        1
    }
}

#[async_trait]
trait ErasedTrait {
    async fn work(&self) -> usize;
}

struct Erased;

#[async_trait]
impl ErasedTrait for Erased {
    #[logcall::logcall("info")]
    #[trace]
    async fn work(&self) -> usize {
        2
    }
}

#[derive(Debug)]
struct InnerError;

#[derive(Debug)]
struct OuterError;

type BoxedFuture = Pin<Box<dyn Future<Output = Result<u32, OuterError>> + Send>>;

#[async_trait]
trait BoxedFutureFactory {
    async fn make() -> Result<BoxedFuture, OuterError>;
}

struct Factory;

#[async_trait]
impl BoxedFutureFactory for Factory {
    #[trace]
    async fn make() -> Result<BoxedFuture, OuterError> {
        make_boxed_future().await
    }
}

#[trace]
async fn make_boxed_future() -> Result<BoxedFuture, OuterError> {
    let inner = async { Err::<u32, _>(InnerError) };
    let mapped = async move { inner.await.map_err(|_error| OuterError) };
    Ok(Box::pin(mapped))
}

#[custom_trace(crate = custom_fastrace)]
async fn custom_crate_path(value: u32) -> u32 {
    value
}

#[custom_trace(crate = ::fastrace, short_name = true)]
fn absolute_crate_path() -> i32 {
    42
}

#[trace(enter_on_poll = true)]
async fn enter_on_poll(value: u32) -> u32 {
    value
}

#[trace(short_name = true)]
fn sync_generic<'a, T, E>(value: &'a T) -> Result<&'a T, E>
where
    T: ?Sized,
{
    Ok(value)
}

#[trace(short_name = true)]
async fn async_generic<T>(value: T) -> impl AsRef<str>
where
    T: Into<String>,
{
    value.into()
}

struct Worker(String);

impl Worker {
    #[trace(short_name = true)]
    fn sync_method(&self) -> &str {
        &self.0
    }

    #[trace(short_name = true)]
    async fn async_method(&mut self, suffix: &str) -> String {
        self.0.push_str(suffix);
        self.0.clone()
    }
}

#[trace(name = "Name", short_name = false)]
async fn named_and_long(value: u32) -> u32 {
    value
}

#[trace(name = "test-span")]
async fn named_async(value: u32) -> u32 {
    value
}

#[trace(name = "test-span")]
async fn named_async_mut(mut value: u32) -> u32 {
    value += 1;
    value
}

#[trace(name = "test-span")]
fn named_sync(value: u32) -> u32 {
    value
}

#[trace(name = "test-span")]
fn named_sync_mut(mut value: u32) -> u32 {
    value += 1;
    value
}

#[trace]
fn no_arguments(value: u32) -> u32 {
    value
}

#[trace(short_name = true)]
async fn short_name(value: u32) -> u32 {
    value
}

#[trace(short_name = true)]
pub(crate) unsafe extern "C" fn unsafe_extern(value: u32) -> u32 {
    value
}

#[derive(Debug)]
struct Input {
    value: i64,
}

#[trace(short_name = true, properties = { "k1": "v1", "a": "argument a is {a:?}", "b": "{b:?}", "escaped1": "{c:?}{{}}", "escaped2": "{{ \"a\": \"b\"}}" })]
async fn async_properties(a: i64, b: &Input, c: Input) -> i64 {
    a
}

#[trace(short_name = true, properties = {})]
async fn empty_properties(value: u32) -> u32 {
    value
}

#[trace(short_name = true, properties = { "literal": "value", "input": "{input:?}", "escaped": "{{input}}" })]
fn sync_properties(input: &Input) -> i64 {
    input.value
}

#[trace]
async fn traced_stream() -> impl Stream<Item = i64> {
    stream! {
        for value in 0..100 {
            yield value;
        }
    }
}

#[tokio::test]
async fn compile_pass() {
    assert_eq!(Native.work().await, 1);
    assert_eq!(Erased.work().await, 2);

    let future = Factory::make().await.unwrap();
    assert!(future.await.is_err());
    let future = make_boxed_future().await.unwrap();
    assert!(future.await.is_err());

    assert_eq!(custom_crate_path(1).await, 1);
    assert_eq!(absolute_crate_path(), 42);
    assert_eq!(enter_on_poll(1).await, 1);
    assert_eq!(sync_generic::<_, ()>("value"), Ok("value"));
    assert_eq!(async_generic("value").await.as_ref(), "value");

    let mut worker = Worker(String::from("fast"));
    assert_eq!(worker.sync_method(), "fast");
    assert_eq!(worker.async_method("race").await, "fastrace");

    assert_eq!(named_and_long(1).await, 1);
    assert_eq!(named_async(1).await, 1);
    assert_eq!(named_async_mut(1).await, 2);
    assert_eq!(named_sync(1), 1);
    assert_eq!(named_sync_mut(1), 2);
    assert_eq!(no_arguments(1), 1);
    assert_eq!(short_name(1).await, 1);
    assert_eq!(unsafe { unsafe_extern(1) }, 1);

    assert_eq!(
        async_properties(1, &Input { value: 2 }, Input { value: 3 }).await,
        1
    );
    assert_eq!(empty_properties(1).await, 1);
    assert_eq!(sync_properties(&Input { value: 7 }), 7);

    let _ = traced_stream().await;
}
