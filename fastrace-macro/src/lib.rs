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

// This crate is derived from [1] under the original license header:
// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.
// [1]: https://github.com/tikv/minitrace-rust/blob/v0.6.4/minitrace-macro/src/lib.rs

//! An attribute macro designed to eliminate boilerplate code for [`fastrace`].
//!
//! [`fastrace`]: https://crates.io/crates/fastrace

#![recursion_limit = "256"]

mod args;
#[cfg(feature = "enable")]
mod impls;

/// An attribute macro designed to eliminate boilerplate code.
///
/// This macro automatically creates a span for the annotated function. The span name defaults to
/// the function name but can be customized by passing a string literal as an argument using the
/// `name` parameter.
///
/// The `#[trace]` attribute requires a local parent context to function correctly. Ensure that
/// the function annotated with `#[trace]` is called within __a local context of a `Span`__, which
/// is established by invoking the `Span::set_local_parent()` method.
///
/// ## Arguments
///
/// * `name` - The name of the span. Defaults to the full path of the function.
/// * `short_name` - Whether to use the function name without path as the span name. Defaults to
///   `false`.
/// * `enter_on_poll` - Whether to enter the span on poll. If set to `false`, `in_span` will be
///   used. Only available for `async fn`. Defaults to `false`.
/// * `properties` - A list of key-value pairs to be added as properties to the span. The value can
///   be a format string, where the function arguments are accessible. Defaults to `{}`.
/// * `crate` - The path to the fastrace crate. Defaults to `::fastrace`.
///
/// # Examples
///
/// ```
/// use fastrace::prelude::*;
///
/// #[trace]
/// fn simple() {
///     // ...
/// }
///
/// #[trace(short_name = true)]
/// async fn simple_async() {
///     // ...
/// }
///
/// #[trace(name = "qux", enter_on_poll = true)]
/// async fn baz() {
///     // ...
/// }
///
/// #[trace(properties = { "k1": "v1", "a": "argument `a` is {a:?}" })]
/// async fn properties(a: u64) {
///     // ...
/// }
/// ```
///
/// The code snippets above will be expanded to:
///
/// ```
/// # use fastrace::prelude::*;
/// # use fastrace::local::LocalSpan;
/// fn simple() {
///     let __guard__ = LocalSpan::enter_with_local_parent("example::simple");
///     // ...
/// }
///
/// async fn simple_async() {
///     let __span__ = Span::enter_with_local_parent("simple_async");
///     async {
///         // ...
///     }
///     .in_span(__span__)
///     .await
/// }
///
/// async fn baz() {
///     async {
///         // ...
///     }
///     .enter_on_poll("qux")
///     .await
/// }
///
/// async fn properties(a: u64) {
///     let __span__ = Span::enter_with_local_parent("example::properties").with_properties(|| {
///         [
///             (std::borrow::Cow::from("k1"), std::borrow::Cow::from("v1")),
///             (
///                 std::borrow::Cow::from("a"),
///                 std::borrow::Cow::from(format!("argument `a` is {a:?}")),
///             ),
///         ]
///     });
///     async {
///         // ...
///     }
///     .in_span(__span__)
///     .await
/// }
/// ```
#[proc_macro_attribute]
pub fn trace(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    #[cfg(not(feature = "enable"))]
    {
        use syn::parse_macro_input;

        // simply check the attributes
        parse_macro_input!(args as args::Args);
        item
    }

    #[cfg(feature = "enable")]
    {
        use syn::ItemFn;
        use syn::parse_macro_input;

        let args = parse_macro_input!(args as args::Args);
        let input = parse_macro_input!(item as ItemFn);
        match impls::gen_trace(args, input) {
            Ok(tokens) => tokens.into(),
            Err(err) => err.to_compile_error().into(),
        }
    }
}
