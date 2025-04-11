// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! `fastrace` is a high-performance, ergonomic, library-level timeline tracing library for Rust.
//!
//! Unlike most tracing libraries which are primarily designed for instrumenting executables,
//! `fastrace` also accommodates the need for library instrumentation. It stands out due to
//! its extreme lightweight and fast performance compared to other tracing libraries. Moreover,
//! it has zero overhead when not enabled in the executable, making it a worry-free choice for
//! libraries concerned about unnecessary performance loss.
//!
//! # Getting Started
//!
//! ## In Libraries
//!
//! Libraries should include `fastrace` as a dependency without enabling any extra features.
//!
//! ```toml
//! [dependencies]
//! fastrace = "0.7"
//! ```
//!
//! Add a [`trace`] attribute to the function you want to trace. In this example, a
//! [`SpanRecord`] will be collected every time the function is called, if a tracing context
//! is set up by the caller.
//!
//! ```
//! # struct HttpRequest;
//! # struct Error;
//! #[fastrace::trace]
//! pub fn send_request(req: HttpRequest) -> Result<(), Error> {
//!     // ...
//!     # Ok(())
//! }
//! ```
//!
//! Libraries are able to set up an individual tracing context, regardless of whether
//! the caller has set up a tracing context or not. This can be achieved by using
//! [`Span::root()`] to start a new trace and [`Span::set_local_parent()`] to set up a
//! local context for the current thread.
//!
//! The [`func_path!()`] macro can detect the function's full name, which is used as
//! the name of the root span.
//!
//! ```
//! use fastrace::prelude::*;
//! # struct HttpRequest;
//! # struct Error;
//!
//! pub fn send_request(req: HttpRequest) -> Result<(), Error> {
//!     let root = Span::root(func_path!(), SpanContext::random());
//!     let _guard = root.set_local_parent();
//!
//!     // ...
//!     # Ok(())
//! }
//! ```
//!
//! ## In Applications
//!
//! Applications should include `fastrace` as a dependency with the `enable` feature
//! set. To disable `fastrace` statically, simply remove the `enable` feature.
//!
//! ```toml
//! [dependencies]
//! fastrace = { version ="0.7", features = ["enable"] }
//! ```
//!
//! Applications should initialize a [`Reporter`] implementation early in the program's runtime.
//! Span records generated before the reporter is initialized will be ignored. Before
//! terminating, [`flush()`] should be called to ensure all collected span records are reported.
//!
//! ```
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//! use fastrace::prelude::*;
//!
//! fn main() {
//!     fastrace::set_reporter(ConsoleReporter, Config::default());
//!
//!     loop {
//!         let root = Span::root("worker-loop", SpanContext::random());
//!         let _guard = root.set_local_parent();
//!
//!         handle_request();
//!         # break;
//!     }
//!
//!     fastrace::flush();
//! }
//! # fn handle_request() {}
//! ```
//!
//! # Key Concepts
//!
//! `fastrace` operates through three types: [`Span`], [`LocalSpan`], and [`Event`], each
//! representing a different type of tracing record. The macro [`trace`] is available to
//! manage these types automatically. For [`Future`] instrumentation, necessary utilities
//! are provided by [`FutureExt`].
//!
//! ## Span
//!
//! A [`Span`] represents an individual unit of work. It contains:
//! - A name
//! - A start timestamp and duration
//! - A set of key-value properties
//! - A reference to a parent `Span`
//!
//! A new `Span` can be started through [`Span::root()`], requiring the trace id and the
//! parent span id from a remote source. If there's no remote parent span, the parent span
//! id is typically set to its default value of zero.
//!
//! Once we have the root `Span`, we can create a child `Span` using [`Span::enter_with_parent()`],
//! thereby establishing the reference relationship between the spans.
//!
//! `Span` is thread-safe and can be sent across threads.
//! ```
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//! use fastrace::prelude::*;
//!
//! fastrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root_span = Span::root("root", SpanContext::random());
//!
//!     {
//!         let child_span = Span::enter_with_parent("a child span", &root_span);
//!
//!         // ...
//!
//!         // child_span ends here.
//!     }
//!
//!     // root_span ends here.
//! }
//!
//! fastrace::flush();
//! ```
//!
//! Sometimes, passing a `Span` through a function to create a child `Span` can be inconvenient.
//! We can employ a thread-local approach to avoid an explicit argument passing in the function.
//! In fastrace, [`Span::set_local_parent()`] and [`Span::enter_with_local_parent()`] serve this
//! purpose.
//!
//! [`Span::set_local_parent()`] method sets __a local context of the `Span`__ for the current
//! thread. [`Span::enter_with_local_parent()`] accesses the parent `Span` from the local context
//! and creates a child `Span` with it.
//!
//! ```
//! use fastrace::prelude::*;
//!
//! {
//!     let root_span = Span::root("root", SpanContext::random());
//!     let _guard = root_span.set_local_parent();
//!
//!     foo();
//!
//!     // root_span ends here.
//! }
//!
//! fn foo() {
//!     // The parent of this span is `root`.
//!     let _child_span = Span::enter_with_local_parent("a child span");
//!
//!     // ...
//!
//!     // _child_span ends here.
//! }
//! ```
//!
//! ## Local Span
//!
//! In a clear single-thread execution flow, where we can ensure that the `Span` does
//! not cross threads, meaning:
//! - The `Span` is not sent to or shared by other threads
//! - In asynchronous code, the lifetime of the `Span` doesn't cross an `.await` point
//!
//! we can use `LocalSpan` as a substitute for `Span` to effectively reduce overhead
//! and greatly enhance performance.
//!
//! However, there is a precondition: The creation of `LocalSpan` must take place
//! within __a local context of a `Span`__, which is established by invoking the
//! [`Span::set_local_parent()`] method.
//!
//! If the code spans multiple function calls, this isn't always straightforward to
//! confirm if the precondition is met. As such, it's recommended to invoke
//! [`Span::set_local_parent()`] immediately after the creation of `Span`.
//!
//! After __a local context of a `Span`__ is set using [`Span::set_local_parent()`],
//! use [`LocalSpan::enter_with_local_parent()`] to start a `LocalSpan`, which then
//! becomes the new local parent.
//!
//! If no local context is set, the [`LocalSpan::enter_with_local_parent()`] will do nothing.
//! ```
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//! use fastrace::prelude::*;
//!
//! fastrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root = Span::root("root", SpanContext::random());
//!     let _guard = root.set_local_parent();
//!
//!     {
//!         // The parent of this span is `root`.
//!         let _span1 = LocalSpan::enter_with_local_parent("a child span");
//!
//!         foo();
//!     }
//! }
//!
//! fn foo() {
//!     // The parent of this span is `span1`.
//!     let _span2 = LocalSpan::enter_with_local_parent("a child span of child span");
//! }
//!
//! fastrace::flush();
//! ```
//!
//! ## Event
//!
//! [`Event`] represents a single point in time where something occurred during the execution of a
//! program.
//!
//! An `Event` can be seen as a log record attached to a span.
//! ```
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//! use fastrace::prelude::*;
//!
//! fastrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root = Span::root("root", SpanContext::random());
//!     let _guard = root.set_local_parent();
//!
//!     root.add_event(Event::new("event in root"));
//!
//!     {
//!         let _span1 = LocalSpan::enter_with_local_parent("a child span");
//!
//!         LocalSpan::add_event(Event::new("event in span1"));
//!     }
//! }
//!
//! fastrace::flush();
//! ```
//!
//! ## Macro
//!
//! The attribute-macro [`trace`] helps to reduce boilerplate.
//!
//! Note: For successful tracing a function using the [`trace`] macro, the function call should
//! occur within __a local context of a `Span`__.
//!
//! For more detailed usage instructions, please refer to [`trace`].
//!
//! ```
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//! use fastrace::prelude::*;
//! use pollster::block_on;
//!
//! #[trace]
//! fn do_something(i: u64) {
//!     std::thread::sleep(std::time::Duration::from_millis(i));
//! }
//!
//! #[trace]
//! async fn do_something_async(i: u64) {
//!     futures_timer::Delay::new(std::time::Duration::from_millis(i)).await;
//! }
//!
//! fastrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root = Span::root("root", SpanContext::random());
//!     let _guard = root.set_local_parent();
//!
//!     do_something(100);
//!
//!     block_on(
//!         async {
//!             do_something_async(100).await;
//!         }
//!         .in_span(Span::enter_with_local_parent("aync_job")),
//!     );
//! }
//!
//! fastrace::flush();
//! ```
//!
//! ## Reporter
//!
//! [`Reporter`] is responsible for reporting the span records to a remote agent,
//! such as Jaeger.
//!
//! Executables should initialize a reporter implementation early in the program's
//! runtime. Span records generated before the reporter is initialized will be ignored.
//!
//! For an easy start, `fastrace` offers a [`ConsoleReporter`] that prints span
//! records to stderr. For more advanced use, crates like `fastrace-jaeger`, `fastrace-datadog`,
//! and `fastrace-opentelemetry` are available.
//!
//! By default, the reporter is triggered every 500 milliseconds. The reporter can also be
//! triggered manually by calling [`flush()`]. See [`Config`] for customizing the reporting
//! behavior.
//!
//! ```
//! use std::time::Duration;
//!
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//!
//! fastrace::set_reporter(
//!     ConsoleReporter,
//!     Config::default().report_interval(Duration::from_secs(1)),
//! );
//!
//! fastrace::flush();
//! ```
//!
//! # Performance
//!
//! `fastrace` is designed to be fast and lightweight, considering four scenarios:
//!
//! - **No Tracing**: If the feature `enable` is not set in the application, `fastrace` will be
//!   completely optimized away from the final executable binary, achieving zero overhead.
//!
//! - **Sample Tracing**: If `enable` is set in the application, but only a small portion of the
//!   traces are enabled via [`Span::root()`], while the other portions are started with
//!   placeholders using [`Span::noop()`]. The overhead in this case is very small - merely an
//!   integer load, comparison, and jump.
//!
//! - **Full Tracing with Tail Sampling**: If `enable` is set in the application, and all traces are
//!   enabled, however, only a select few interesting tracing records (e.g., P99) are reported,
//!   while normal traces are dismissed by using [`Span::cancel()`] to avoid being reported, the
//!   overhead of collecting traces is still very small. This could be useful when you are
//!   interested in examining program's tail latency.
//!
//! - **Full Tracing**: If `enable` is set in the application, and all traces are reported,
//!   `fastrace` performs 10x to 100x faster than other tracing libraries in this case.
//!
//!
//! [`Span`]: crate::Span
//! [`LocalSpan`]: crate::local::LocalSpan
//! [`SpanRecord`]: crate::collector::SpanRecord
//! [`FutureExt`]: crate::future::FutureExt
//! [`trace`]: crate::trace
//! [`LocalCollector`]: crate::local::LocalCollector
//! [`Span::root()`]: crate::Span::root
//! [`Span::noop()`]: crate::Span::noop
//! [`Span::cancel()`]: crate::Span::cancel
//! [`Span::enter_with_parent()`]: crate::Span::enter_with_parent
//! [`Span::set_local_parent()`]: crate::Span::set_local_parent
//! [`LocalSpan::enter_with_local_parent()`]: crate::local::LocalSpan::enter_with_local_parent
//! [`Event`]: crate::Event
//! [`Reporter`]: crate::collector::Reporter
//! [`ConsoleReporter`]: crate::collector::ConsoleReporter
//! [`Config`]: crate::collector::Config
//! [`Future`]: std::future::Future

// Suppress a false-positive lint from clippy
#![allow(clippy::needless_doctest_main)]
#![cfg_attr(not(feature = "enable"), allow(dead_code))]
#![cfg_attr(not(feature = "enable"), allow(unused_mut))]
#![cfg_attr(not(feature = "enable"), allow(unused_imports))]
#![cfg_attr(not(feature = "enable"), allow(unused_variables))]
#![cfg_attr(target_family = "wasm", allow(dead_code))]

pub mod collector;
mod event;
pub mod future;
pub mod local;
mod macros;
mod span;
#[doc(hidden)]
pub mod util;

pub use fastrace_macro::trace;

pub use crate::collector::global_collector::flush;
pub use crate::collector::global_collector::set_reporter;
pub use crate::event::Event;
pub use crate::span::Span;

pub mod prelude {
    //! A "prelude" for crates using `fastrace`.
    #[doc(no_inline)]
    pub use crate::collector::SpanContext;
    #[doc(no_inline)]
    pub use crate::collector::SpanId;
    #[doc(no_inline)]
    pub use crate::collector::SpanRecord;
    #[doc(no_inline)]
    pub use crate::collector::TraceId;
    #[doc(no_inline)]
    pub use crate::event::Event;
    #[doc(no_inline)]
    pub use crate::file_location;
    #[allow(deprecated)]
    #[doc(no_inline)]
    pub use crate::full_name;
    #[doc(no_inline)]
    pub use crate::func_name;
    #[doc(no_inline)]
    pub use crate::func_path;
    #[doc(no_inline)]
    pub use crate::future::FutureExt as _;
    #[doc(no_inline)]
    pub use crate::local::LocalSpan;
    #[doc(no_inline)]
    pub use crate::span::Span;
    #[doc(no_inline)]
    pub use crate::trace;
}
