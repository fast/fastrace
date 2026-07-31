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
