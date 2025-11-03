# fastrace: Architecture and Span Lifecycle

This document outlines the core architecture and span lifecycle within the `fastrace` library, addressing how spans are created, managed, and submitted to the global collector.

## 1. Introduction

`fastrace` is a high-performance, low-overhead tracing library for Rust, designed for both application and library-level instrumentation. Its primary goal is to minimize performance impact while providing comprehensive timeline tracing capabilities.

## 2. Core Concepts

*   **`Span`**: Represents a unit of work. It is thread-safe and can be sent across threads. `Span`s form the hierarchical structure of a trace.
*   **`LocalSpan`**: An optimized, non-thread-safe span for operations within a single thread. It offers lower overhead than `Span` but requires a local parent context.
*   **`Event`**: A single point-in-time occurrence within a span, akin to a log record attached to a span.
*   **`SpanContext`**: Contains `TraceId`, `SpanId`, and a `sampled` flag, used for context propagation.
*   **`RawSpan`**: The internal, raw data structure representing a span or event before it's processed into a `SpanRecord`. It includes `id`, `parent_id`, `begin_instant`, `end_instant`, `name`, `properties`, and `raw_kind` (Span, Event, Properties). For reference, the structure is defined in `fastrace/src/local/raw_span.rs`:
    ```rust
    pub struct RawSpan {
        pub id: SpanId,
        pub parent_id: Option<SpanId>,
        pub begin_instant: Instant,
        pub name: Cow<'static, str>,
        pub properties: Option<Properties>,
        pub raw_kind: RawKind,
        pub end_instant: Instant,
    }
    ```
*   **`SpanRecord`**: The finalized, reportable representation of a span, containing all necessary information for a `Reporter`.

## 3. Span Lifecycle and Data Flow

The lifecycle of a span in `fastrace` involves several components working in concert:

### 3.1. Span Creation

*   **`Span::root(name, parent_context)`**: Initiates a new trace or attaches to a remote parent. It creates a `SpanInner` which holds a `RawSpan` and a `CollectToken`. It also registers a new collection context with the `GlobalCollector` if `parent_context.sampled` is true.
*   **`Span::enter_with_parent(name, &parent_span)`**: Creates a child `Span` with an explicit parent.
*   **`Span::enter_with_local_parent(name)`**: Creates a child `Span` using the current thread-local parent. If no local parent is set, it returns a no-op span.
*   **`LocalSpan::enter_with_local_parent(name)`**: Creates a `LocalSpan` using the current thread-local parent. This is the most performant option for single-threaded, synchronous operations.

### 3.2. Thread-Local Storage and Span Management

`fastrace` uses thread-local storage to manage `LocalSpan`s efficiently:

*   **`LOCAL_SPAN_STACK`**: A thread-local `Rc<RefCell<LocalSpanStack>>` that acts as a stack of `SpanLine`s. Each `SpanLine` represents an active tracing context within the thread.
*   **`LocalSpanStack`**: Manages a `Vec<SpanLine>`. When `Span::set_local_parent()` is called, a new `SpanLine` is registered on this stack, making the `Span` the new local parent.
*   **`SpanLine`**: Contains a `SpanQueue` (a `Vec<RawSpan>`) and a `CollectToken`. It's responsible for:
    *   Starting (`start_span`) and finishing (`finish_span`) `RawSpan`s.
    *   Adding events (`add_event`) and properties (`add_properties`, `with_properties`) to the current `RawSpan` or its parent.
    *   Tracking the `current_parent_id` for nested `LocalSpan`s.
*   **`SpanQueue`**: A simple `Vec<RawSpan>` that stores the raw span data for a given `SpanLine`.

### 3.3. Span Finalization and Submission (The Role of `Drop`)

The core mechanism for span finalization and submission relies heavily on Rust's RAII pattern and `Drop` implementations:

*   **`Span`'s `Drop` Implementation**:
    *   When a `Span` (specifically its `SpanInner`) is dropped, its `end_instant` is recorded.
    *   It then calls `inner.submit_spans()`, which sends a `CollectCommand::SubmitSpans` containing `SpanSet::Span(self.raw_span)` and its `CollectToken` to the `GlobalCollector` via an SPSC channel.
    *   If the `Span` was a root span (i.e., `collect_id` is `Some`), it also calls `collect.commit_collect(collect_id)` to signal the `GlobalCollector` that this trace is complete.

*   **`LocalParentGuard`'s `Drop` Implementation**:
    *   `LocalParentGuard` is returned by `Span::set_local_parent()`. Its `Drop` implementation is crucial for collecting all `LocalSpan`s created within its scope.
    *   When `LocalParentGuard` is dropped, it calls `inner.collector.collect_spans_and_token()`. This retrieves all `RawSpan`s from the `LocalCollector`'s associated `SpanLine`.
    *   These collected `RawSpan`s are then wrapped into a `LocalSpansInner` and submitted to the `GlobalCollector` as `CollectCommand::SubmitSpans` with `SpanSet::LocalSpansInner(spans)` and the relevant `CollectToken`.

*   **`LocalSpan`'s `Drop` Implementation**:
    *   When a `LocalSpan` is dropped, it calls `span_stack.exit_span(span_handle)`, which marks its corresponding `RawSpan` in the `SpanQueue` with an `end_instant`. It does *not* directly submit to the `GlobalCollector`; instead, its `RawSpan` is collected by the `LocalParentGuard` when its scope ends.

### 3.4. `GlobalCollector` and Reporting

*   **`GlobalCollector`**: A central, background thread (or handled synchronously in WASM) that processes commands from all `Span`s and `LocalParentGuard`s.
*   **SPSC Channel**: `Span`s and `LocalParentGuard`s send `CollectCommand`s (StartCollect, DropCollect, CommitCollect, SubmitSpans) to the `GlobalCollector` via a single-producer, single-consumer (SPSC) channel.
*   **`handle_commands()`**: The `GlobalCollector`'s main loop. It continuously drains the SPSC channel, processing commands:
    *   `StartCollect`: Registers a new active collector.
    *   `DropCollect`: Removes an active collector (used for `Span::cancel()`).
    *   `CommitCollect`: Signals that a root span has completed, triggering the processing of its collected spans.
    *   `SubmitSpans`: Receives `SpanSet`s (containing `RawSpan`s or `LocalSpansInner`) and their `CollectToken`s. These are stored in `active_collectors` or `stale_spans` based on `collect_id` and `tail_sampled` configuration.
*   **`report_interval`**: The `GlobalCollector` periodically calls `handle_commands()` (every `report_interval`) to process pending commands and report spans.
*   **`Reporter`**: Once `RawSpan`s are processed into `SpanRecord`s (in `postprocess_span_collection`), they are passed to the configured `Reporter` (e.g., `JaegerReporter`, `ConsoleReporter`) for external transmission.
*   **`fastrace::flush()`**: Manually triggers `GlobalCollector::handle_commands()` to process all pending commands immediately, ensuring all collected spans are reported.

### 3.4.1. Optimized Span Submission

To further reduce overhead, especially when a span has multiple parents, the internal mechanism for submitting spans to the `GlobalCollector` has been optimized:

*   **Shared `SpanSet`**: When `Span::submit_spans()` or `LocalSpan::submit_partial()` is called, the `SpanSet` (which contains the `RawSpan` data) is now wrapped in an `Arc<SpanSet>` once. This `Arc` allows the `SpanSet` data to be shared efficiently without cloning the entire data structure.
*   **Individual Commands**: Instead of sending a single `SubmitSpans` command with a `Vec<CollectTokenItem>`, the `GlobalCollect::submit_spans` method now sends individual `SubmitSpans` commands for each `CollectTokenItem`. Each command contains a clone of the `Arc<SpanSet>` (which is just a pointer copy, not a deep clone of the span data) and a single `CollectTokenItem`.
*   **Simplified `SpanCollection`**: Internally, the `GlobalCollector`'s `SpanCollection` enum has been simplified to a single `Collected(Arc<SpanSet>, TraceId, SpanId)` variant. This streamlines the handling of submitted spans within the collector.

This optimization significantly reduces the cloning overhead associated with `SpanSet`s, particularly in scenarios where a single span or set of local spans needs to be associated with multiple parent contexts. By leveraging `Arc`, the underlying span data is shared, leading to improved performance.

## 4. Addressing the User's Concern: Spans Not Collected Until Scope Changes

The user observed that spans are not immediately visible in the `GlobalCollector` even with `tail_sampled` off, and only appear after the enclosing scope changes. This behavior is a direct consequence of `fastrace`'s RAII-based design for span finalization:

*   **`LocalSpan`s are buffered**: `LocalSpan`s are not individually submitted to the `GlobalCollector` upon creation or completion. Instead, their `RawSpan` data is buffered within the thread-local `SpanQueue` of their `SpanLine`.
*   **`LocalParentGuard` is the trigger**: The actual submission of these buffered `LocalSpan`s to the `GlobalCollector` occurs when the `LocalParentGuard` (created by `Span::set_local_parent()`) goes out of scope and its `Drop` implementation is executed. This `Drop` collects all `RawSpan`s from its associated `LocalCollector` and sends them as a batch to the `GlobalCollector`.
*   **`Span`s submit on drop**: Similarly, a `Span` itself only submits its own `RawSpan` data to the `GlobalCollector` when it is dropped.
*   **`report_interval` for batching**: Even after spans are submitted to the `GlobalCollector`'s SPSC channel, they are not necessarily reported immediately to Jaeger. The `GlobalCollector` processes commands and reports spans in batches, according to its `report_interval`. Setting `report_interval` to 0ms can reduce this delay, but it doesn't change *when* spans are submitted to the `GlobalCollector`'s internal queue.

Therefore, the "scope change" is critical because it triggers the `Drop` implementations of `Span`s and `LocalParentGuard`s, which are responsible for collecting and submitting the span data to the `GlobalCollector`.

## 5. Earlier Span Submission with `submit_partial()`

`fastrace` now provides `Span::submit_partial()` and `LocalSpan::submit_partial()` methods to allow for earlier (e.g., "mid-span") submission of span data to the `GlobalCollector`. This addresses scenarios where long-running operations might benefit from intermediate trace updates.

### 5.1. How `submit_partial()` Works

The `submit_partial()` methods work by taking a snapshot of the current state of the `Span` or `LocalSpan` and sending this partial data to the `GlobalCollector`.

*   **`Span::submit_partial()`**: When called on a `Span`, it clones the current `RawSpan` data and sends it to the `GlobalCollector` via the SPSC channel as a `CollectCommand::SubmitSpans` with `SpanSet::SharedLocalSpans`. This allows the `GlobalCollector` to process and potentially report the span's current state even if the span has not yet ended.
*   **`LocalSpan::submit_partial()`**: Similarly, when called on a `LocalSpan`, it collects all `RawSpan`s from the current `SpanLine` and submits them to the `GlobalCollector` as `CollectCommand::SubmitSpans` with `SpanSet::SharedLocalSpans`. This effectively flushes the thread-local buffer of `LocalSpan`s up to that point.

### 5.2. Considerations and Best Practices

1.  **Overhead**: Calling `submit_partial()` involves cloning span data and sending it over an SPSC channel. Frequent calls can introduce overhead, potentially impacting `fastrace`'s low-overhead design. Use it judiciously for long-running spans where intermediate visibility is genuinely beneficial.
2.  **Partial Data**: The submitted data represents the span's state at the time of the call. If the span continues to execute and more properties or events are added, these will only be included in subsequent `submit_partial()` calls or the final submission when the span drops.
3.  **Duration**: The `end_instant` of a partially submitted span will reflect the time of the `submit_partial()` call. The final `end_instant` will be set when the span naturally drops. The `GlobalCollector` is designed to handle updates to spans, so later submissions with a final `end_instant` will supersede earlier partial submissions.
4.  **Asynchronous Contexts**: `submit_partial()` is particularly useful in asynchronous operations where a single logical operation might span a significant amount of time, and immediate feedback on its progress is desired.

### 5.3. Recommendation

For scenarios requiring earlier visibility of long-running spans, `Span::submit_partial()` and `LocalSpan::submit_partial()` provide a direct mechanism. While they introduce some overhead due to data cloning and channel communication, this is a controlled trade-off for improved observability. For general use cases, relying on the RAII-based `Drop` implementations for final span submission remains the most performant approach. The `fastrace::flush()` mechanism can still be used to ensure all *completed* spans are processed and reported promptly.
