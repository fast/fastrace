# Contribution Guide

## Get Started

This is a Rust project, so [rustup](https://rustup.rs/) is the best place to start.

This is a pure rust project, so only `cargo` is needed.

- `cargo check` to analyze the current package and report errors.
- `cargo build` to compile the current package.
- `cargo clippy` to catch common mistakes and improve code.
- `cargo test` to run unit tests.
- `cargo bench` to run benchmark tests.

Useful tips:

- Check/Build/Test/Clippy all code: `cargo <cmd> --all-targets --workspace`
- Test specific function: `cargo test multiple_local_parent`

## For features/questions/discussions

Please open [new issues](https://github.com/fastracelabs/fastrace/issues/new/choose).
