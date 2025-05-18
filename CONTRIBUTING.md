# Contribution Guide

## Getting started

This is a Rust project, so [rustup](https://rustup.rs/) is the best place to start.

This is a pure rust project, so only `cargo` is needed.

- `cargo check` to analyze the current package and report errors.
- `cargo build` to compile the current package.
- `cargo clippy` to catch common mistakes and improve code.
- `cargo test` to run unit tests*.
- `cargo bench` to run benchmark tests.

Useful tips:

- Check/Build/Test/Clippy all code: `cargo <cmd> --all-targets --workspace`
- Test specific function: `cargo test multiple_local_parent`

### *Testing 

The core `fastrace` crate uses [Insta Snapshot testing](https://insta.rs/) locally. Be aware that to properly run tests,
you will ideally need to have `cargo-insta` installed in your local rust toolchain to be able to review changes to tests
where the snapshots have changed, and thus prevent CI from breaking in such instances.

For more information on `cargo-insta` [read the official documentation here.](https://insta.rs/docs/cli/)

## For features, questions, or discussions

Please open [a new issue](https://github.com/fast/fastrace/issues/new/choose).
