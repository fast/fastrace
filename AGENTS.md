# Fastrace Agent Guide

## Overview
- Fastrace is a high-performance tracing library focused on low overhead when disabled and fast collection when enabled. The workspace bundles the core tracer, a proc-macro, async/stream helpers, and reporters for Datadog, Jaeger, and OpenTelemetry.

## Workspace Map
- `fastrace/` core library (types, collectors, local spans, future helpers). Critical files: `src/span.rs`, `src/local/*`, `src/collector/*`, `src/future.rs`, `src/macros.rs`.
- `fastrace-macro/` implements the `#[trace]` attribute and codegen rules.
- Reporter crates: `fastrace-jaeger/`, `fastrace-datadog/`, `fastrace-opentelemetry/`.
- Async/stream integration: `fastrace-futures/`.
- Examples in `examples/*.rs`; integration tests in `fastrace/tests`; UI macro tests under `tests/macros`.
- `tests/statically-disable/` ensures the crate still compiles as a no-op when `enable` feature is absent.

## Runtime & Data Flow
- Applications call `set_reporter` (e.g., `ConsoleReporter`, Datadog/Jaeger/OTel) to spin up a background global collector thread (`collector/global_collector.rs`). It polls per-thread SPSC queues (`util/spsc.rs`) at `Config.report_interval`.
- `Span::root` allocates a collect_id (if sampled) and stores a `CollectToken` describing parent linkage and sampling. Dropping a span records end time and submits a `SpanSet` to the global collector; root spans also emit `CommitCollect`.
- `LocalParentGuard` from `Span::set_local_parent` installs a thread-local `LocalSpanStack`; dropping the guard drains collected `LocalSpan`s and submits them as `SpanSet::LocalSpansInner`.
- The collector converts `RawSpan`s into `SpanRecord`s using a single `Anchor` for timestamp to unix conversion. Events and property-only raw entries are re-attached to their parent in `mount_danglings`.
- Tail sampling: `Config.tail_sampled(true)` holds spans until root commit; `Span::cancel()` on root triggers `DropCollect` to discard the trace.

## Key Types & Behaviors
- `Span` (`fastrace/src/span.rs`): thread-safe span, can be cross-thread; supports `enter_with_parent(s)`, `enter_with_local_parent`, `add_event`, `add_properties`, `push_child_spans`, `elapsed`, `cancel`.
- `LocalSpan` (`local/local_span.rs`): single-thread fast path; requires an active local parent (`Span::set_local_parent` or nested `LocalSpan`). Stack discipline enforced; dropping out of order panics in tests.
- `LocalCollector` (`local/local_collector.rs`): start/collect local spans when parent may be set later; returns `LocalSpans` attachable via `Span::push_child_spans` or convertible to `SpanRecord`s without a reporter.
- ID types (`collector/id.rs`): `TraceId(u128)`, `SpanId(u64)`; thread-local generator (`SpanId::next_id`) and W3C traceparent encode/decode helpers. `SpanContext` carries trace, parent span id, and `sampled` flag.
- `Config` (`collector/mod.rs`): `report_interval`, `tail_sampled`; note `max_spans_per_trace` deprecated no-op.
- `Reporter` trait (`collector/global_collector.rs`): synchronous `report(Vec<SpanRecord>)`. TestReporter collects into a shared Vec; ConsoleReporter prints.
- Async adapters: `FutureExt::{in_span, enter_on_poll}` (`src/future.rs`); Stream/Sink adapters in `fastrace-futures`.
- Proc macro `#[trace]` (`fastrace-macro/src/lib.rs`): wraps sync fns with `LocalSpan`; async fns with `Span::enter_with_local_parent` plus `in_span` by default or `enter_on_poll=true`. Options: `name`, `short_name`, `enter_on_poll`, `properties={k:"fmt"}`, `crate=path`.
- `enable` feature gates runtime collection; workspace members (examples/tests) request it, so full builds usually compile with tracing enabled. Ensure any changes keep the no-feature path compiling (many functions are #[cfg(feature="enable")] guarded).

## Reporters
- Datadog (`fastrace-datadog/src/lib.rs`): maps `SpanRecord` to msgpack payload; send to `http://{agent}/v0.4/traces`.
- Jaeger (`fastrace-jaeger/src/lib.rs`): UDP compact-thrift batches; splits batches to fit 8k.
- OpenTelemetry (`fastrace-opentelemetry/src/lib.rs`): maps properties to OTel semantics (`span.kind`, `span.status_code`, `span.status_description`, `span.parent_span_is_remote`); filters those keys out of attributes; uses provided exporter + instrumentation scope.

## Testing & Examples
- Core integration tests (`fastrace/tests/lib.rs`) use `TestReporter` snapshots (insta) and serial execution; cover multi-thread spans, macro behavior, properties/events, sampling, and LocalCollector workflow.
- Macro UI tests (`tests/macros`) exercise error cases via trybuild.
- Examples: `cargo run --example basic|synchronous|asynchronous` demonstrate setup; `examples/logging.rs` shows log integration; `examples/sampling.rs` tail-sampling.
- Statically-disabled check: `cargo run --package test-statically-disable` builds with `enable` off.

## Development Workflow (executed 2025-12-15)
- `cargo test --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo run --example asynchronous`
- `cargo run --example synchronous`
- `cargo run --example basic`
- `cargo run --package test-statically-disable`

## Gotchas & Tips
- Always call `set_reporter` early; spans emitted before reporter init are dropped. Call `flush()` before shutdown to drain SPSC queues.
- `LocalSpan` does nothing without a local parent; `Span::enter_with_local_parent` falls back to noop if no stack token exists.
- Default local stack capacity: 4096 span lines and span queue size 10240; overflows drop spans silently (return None).
- Tail sampling only effective when `Config.tail_sampled(true)` and `Span::cancel()` invoked on the root.
- Keep both code paths healthy: when `enable` is off, public APIs stay callable but must remain no-ops without panicking.

## Where to Start
- Trace lifecycle: `fastrace/src/span.rs` (Span), `fastrace/src/local/*` (LocalSpan stack), `fastrace/src/collector/global_collector.rs` (queue + reporting).
- Macro instrumentation rules: `fastrace-macro/src/lib.rs`.
- Reporter mapping details: respective reporter crates above.
