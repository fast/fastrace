# Changelog

## Unreleased

## v0.6.8

- Project rename to `fastrace`.

## v0.6.7

- Add `Config::report_interval`: The background collector working interval.
- Deprecate `Config::batch_report_interval` and `Config::batch_report_max_spans`.
- Fix a performance issue in object-pool which was causing lock racing.

## v0.6.6

- Update to opentelemetry, opentelemetry_sdk, and opentelemetry-otlp.

## v0.6.5

- Update to opentelemetry 0.22, opentelemetry_sdk 0.22.1, and opentelemetry-otlp: 0.15.

## v0.6.4

- Add `LocalSpan::add_property` and `LocalSpan::add_properties`.
- Add `Config::report_before_root_finish`.
- Add new crate `fastrace-futures`.

## v0.6.3

- Add `LocalSpans::to_span_records()`.
- Add `#[trace(properties = { "k1": "v1", "k2": "v2" })]`.
- Add  `func_name!()`, `full_name!()`, and `file_location!()` to `fastrace::prelude`.

## v0.6.2

- Improve documentation.

## v0.6.1

- Macro will use the full path of the function as span name instead of the only function name. You can turn it off by setting `#[trace(short_name = true)]`.
- Add utility macros `func_name!()`, `full_name!()`, and `file_location!()` to generate names for use in span.
- Add `Span::elapsed()` that returns the elapsed time since the span is created.

## v0.6.0

- Span name and event name now accept both `&'static str` and `String` (`Into<Cow<'static, str>>`), which previously only accept `&'static str`.
- `with_property` and `with_properties` now accept `impl Into<Cow<'static, str>>`, which previously accept `Cow<'static, str>`.

## v0.5.1

- Fix panics due to destruction of Thread Local Storage value

## v0.5.0

- Add `Event` type to represent single points in time during the span's lifetime.
- Add `fastrace-opentelementry` reporter that reports spans to OpenTelemetry collector.
- Removed `Collector` and raplaced it with `Reporter`.
- The macro arguments must be named if any, e.g. `#[trace(name="name")]`.
- Allow to statically opt-out of tracing by not setting `enable` feature.

## v0.4.0

- Remove `LocalSpanGuard` and merge it into `LocalSpan`.
- Remove `LocalSpan::with_property`, `LocalSpan::with_properties`, `Span::with_property` and `Span::with_properties`.
- Add `LocalSpan::add_property`, `LocalSpan::add_properties`, `Span::add_property` and `Span::add_properties`.
- Remove `LocalParentGuard`. `Span::set_local_parent` returns a general `Option<Guard<impl FnOnce()>>` instead. 

## v0.3.1

- Add an async variant of jaeger reporting function `fastrace::report()`.
- `LocalSpan::with_property` now no longer takes `self` but `&mut self` instead.

## v0.3.0

- `Collector::collect()` becomes an async function because the span collection work is moved to a background thread to extremely reduce the performance overhead on the code being tracing.
- Attribute macro `#[trace]` on async function becomes able to automatically extract the local parent in the caller's context. Previously, the caller must manually call `in_span()`.

## v0.2.0

- All API get redesigned for better egnormic experience.
- Attribute macro `#[trace]` automatically detects `async fn` and crate `async-trait`, and since that, `#[trace_async]` is removed.
