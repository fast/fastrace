# Changelog

## Unreleased

* Add helpers to extract the current fastrace `SpanContext` and convert it as the current OpenTelemetry `Context`.

## v0.15.0

* Recognise `SpanContext.is_remote` from the `span.parent_span_is_remote` properties on spans.
* Stop exporting OpenTelemetry-reserved properties (span kind, status, remote parent) as generic span attributes.

## v0.14.0

* Upgrade opentelemtry to 0.31.0.

## v0.13.0

* Recognise `Status` from the `span.status_code` and `span.status_description` properties on spans.

## v0.12.0

* Remove `SpanKind` argument from `OpenTelemetryReporter::new()`.
* Recognise `SpanKind` from the `span.kind` property on spans.

## v0.11.0

* Upgrade opentelemtry to 0.30.0.

## v0.10.0

* Upgrade opentelemtry to 0.29.0.

## v0.9.0

* Upgrade opentelemtry to 0.28.0.

## v0.8.1

* Reduce dependencies to futures 0.3.

## v0.8.0

* Upgrade opentelemtry to 0.27.0.
