# CHANGELOG

All significant changes to this project will be documented in this file.

## Unreleased

## v0.17.0

### Notable Changes

* Upgraded MSRV to 1.85 and Edition to 2024.

### New Features

* Added `OpenTelemetryReporter::with_block_on()` for exporters that require a runtime-specific
  executor, such as OTLP HTTP exporters using async `reqwest::Client`.

## v0.16.0

### New Features

* Added OpenTelemetry link export support by mapping `SpanRecord.links` to OTel span links.

## v0.15.1

### New Features

* Added a bridge to extract current fastrace `SpanContext` and convert it into current OpenTelemetry `Context`.

## v0.15.0

### New Features

* Recognized `SpanContext.is_remote` from `span.parent_span_is_remote` span property.

### Improvements

* Stopped exporting OpenTelemetry-reserved properties (`span.kind`, status fields, remote parent flag) as generic span attributes.

## v0.14.0

### Improvements

* Upgraded `opentelemetry` to 0.31.0.

## v0.13.0

### New Features

* Recognized span status from `span.status_code` and `span.status_description` properties.

## v0.12.0

### Breaking Changes

* Removed `SpanKind` argument from `OpenTelemetryReporter::new()`.

### New Features

* Recognized `SpanKind` from `span.kind` span property.

## v0.11.0

### Improvements

* Upgraded `opentelemetry` to 0.30.0.

## v0.10.0

### Improvements

* Upgraded `opentelemetry` to 0.29.0.

## v0.9.0

### Improvements

* Upgraded `opentelemetry` to 0.28.0.

## v0.8.1

### Improvements

* Reduced dependencies to `futures` 0.3.

## v0.8.0

### Improvements

* Upgraded `opentelemetry` to 0.27.0.
