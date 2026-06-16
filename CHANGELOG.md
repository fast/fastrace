# CHANGELOG

All significant changes to this project will be documented in this file.

## Unreleased

## v0.7.18

### Notable Changes

* Upgraded MSRV to 1.91.

## v0.7.17

### Breaking Changes

* Deprecated `Span::enter_with_parents()`. It now uses the first non-noop parent as the primary parent and converts additional parents to links.

### New Features

* Added `Span::with_link()` and `Span::add_link()`.
* Added `LocalSpan::with_link()` and `LocalSpan::add_link()`.
* Added `SpanRecord.links` and propagated links through collector post-processing.
* Added `serde::Serialize` and `serde::Deserialize` support for `SpanContext` using W3C `traceparent` format.

### Bug Fixes

* Fixed `#[trace]` macro span attachment by generating the wrapper function with the original function span.

## v0.7.16

### Improvements

* Deprecated `Config::tail_sampled()`.
* Spans are held until the root span finishes by default, and `Span::cancel()` discards spans collected up to the root span drop.

## v0.7.15

### Bug Fixes

* `#[trace]` macro now supports trait-object futures and preserves input tokens more faithfully to avoid compilation errors.

### Improvements

* Added `#[allow(unreachable_code)]` on the macro return hint to quiet new compiler warnings.

## v0.7.14

### Bug Fixes

* Fixed stale spans not being cleared after reporting.

## v0.7.13

### Bug Fixes

* Fixed memory leak when reporter is not set.

## v0.7.12

### Bug Fixes

* Propagated trace context no matter whether the reporter is set.
* Fixed an issue where `SpanContext::random()` returned a non-zero parent id.

## v0.7.10

### New Features

* `TraceContext::random()` now returns a `TraceContext` with random `TraceId` and `SpanId`.
* Added `Config::tail_sampled()`, which defaults to `false`.
* Added attribute `#[trace(crate = ::fastrace)]` to redirect the path to the `fastrace` crate.

### Improvements

* Deprecated `Config::max_spans_per_trace()` and `Config::report_before_root_finish()`.

## v0.7.9

### Breaking Changes

* Upgraded MSRV to 1.80.

### Improvements

* Improved performance.

## v0.7.8

### New Features

* Added `TraceId::random()` and `SpanId::random()`.
* Added `FromStr`, `Display`, and `serde` support for `TraceId` and `SpanId`.
* Added `Span::add_property()` and `Span::add_properties()`.
* Added `Span::add_event()` and `LocalSpan::add_event()`.

### Improvements

* Deprecated `Event::add_to_parent()` and `Event::add_to_local_parent()`.

## v0.7.6

### Improvements

* Reduced dependencies to `futures` 0.3.

## v0.7.5

### Improvements

* Optimized collect behavior when the span is not sampled.

## v0.7.4

### Improvements

* Upgraded `opentelemetry` to 0.26.0.

## v0.7.3

### Improvements

* Upgraded `opentelemetry` to 0.25.0.

## v0.7.2

### New Features

* Allowed `LocalSpan::add_property()` when the local parent is a `Span`.

## v0.7.1

### Improvements

* Lowered MSRV to 1.75.

## v0.7.0

### Breaking Changes

* Upgraded dependencies including `opentelemetry` and more.
* Removed deprecated methods `Config::batch_report_interval` and `Config::batch_report_max_spans`.
* Changed `Reporter::report()` to take `Vec<SpanRecord>` instead of `&[SpanRecord]`.

### New Features

* Added `SpanContext.sampled`, which is propagated through child spans.

### Improvements

* Deprecated `full_name!()` and renamed it to `full_path!()`.
* Deprecated `SpanContext::encode_w3c_traceparent_with_sampled()`.

## v0.6.8

### Breaking Changes

* Renamed project to `fastrace`.

## v0.6.7

### New Features

* Added `Config::report_interval` as the background collector interval.

### Improvements

* Deprecated `Config::batch_report_interval` and `Config::batch_report_max_spans`.
* Fixed a performance issue in object-pool that caused lock racing.

## v0.6.6

### Improvements

* Upgraded `opentelemetry`, `opentelemetry_sdk`, and `opentelemetry-otlp`.

## v0.6.5

### Improvements

* Upgraded to `opentelemetry` 0.22, `opentelemetry_sdk` 0.22.1, and `opentelemetry-otlp` 0.15.

## v0.6.4

### New Features

* Added `LocalSpan::add_property` and `LocalSpan::add_properties`.
* Added `Config::report_before_root_finish`.
* Added new crate `fastrace-futures`.

## v0.6.3

### New Features

* Added `LocalSpans::to_span_records()`.
* Added `#[trace(properties = { "k1": "v1", "k2": "v2" })]`.
* Added `func_name!()`, `full_name!()`, and `file_location!()` to `fastrace::prelude`.

## v0.6.2

### Improvements

* Improved documentation.

## v0.6.1

### New Features

* Macro now uses the full function path as the default span name. You can turn this off with `#[trace(short_name = true)]`.
* Added utility macros `func_name!()`, `full_name!()`, and `file_location!()` for span naming.
* Added `Span::elapsed()` that returns elapsed time since span creation.

## v0.6.0

### Improvements

* Span name and event name now accept both `&'static str` and `String` (`Into<Cow<'static, str>>`) instead of only `&'static str`.
* `with_property` and `with_properties` now accept `impl Into<Cow<'static, str>>` instead of only `Cow<'static, str>`.

## v0.5.1

### Bug Fixes

* Fixed panics due to destruction of Thread Local Storage values.

## v0.5.0

### Breaking Changes

* Removed `Collector` and replaced it with `Reporter`.
* Macro arguments must be named when provided, for example `#[trace(name = "name")]`.

### New Features

* Added `Event` type to represent single points in time during a span lifetime.
* Added `fastrace-opentelemetry` reporter to send spans to OpenTelemetry collectors.
* Allowed statically opting out of tracing by not enabling the `enable` feature.

## v0.4.0

### Breaking Changes

* Removed `LocalSpanGuard` and merged it into `LocalSpan`.
* Removed `LocalSpan::with_property`, `LocalSpan::with_properties`, `Span::with_property`, and `Span::with_properties`.
* Removed `LocalParentGuard`; `Span::set_local_parent` now returns `Option<Guard<impl FnOnce()>>`.

### New Features

* Added `LocalSpan::add_property`, `LocalSpan::add_properties`, `Span::add_property`, and `Span::add_properties`.

## v0.3.1

### New Features

* Added an async variant of Jaeger reporting function `fastrace::report()`.

### Improvements

* `LocalSpan::with_property` now takes `&mut self` instead of `self`.

## v0.3.0

### Breaking Changes

* `Collector::collect()` became async because span collection moved to a background thread to reduce tracing overhead.

### New Features

* Attribute macro `#[trace]` on async functions can automatically extract the caller local parent. Previously, the caller had to call `in_span()` manually.

## v0.2.0

### Breaking Changes

* Redesigned all APIs for a better ergonomic experience.
* `#[trace]` now automatically detects `async fn` and `async-trait`, and `#[trace_async]` was removed.
