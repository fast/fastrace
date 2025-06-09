// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! An attribute macro designed to eliminate boilerplate code for [`fastrace`](https://crates.io/crates/fastrace).

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "enable"), allow(dead_code))]
#![cfg_attr(not(feature = "enable"), allow(unreachable_code))]

#[macro_use]
extern crate proc_macro_error2;

use std::collections::HashSet;

use proc_macro2::Span;
use quote::quote_spanned;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::*;

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
#[proc_macro_error]
pub fn trace(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    #[cfg(not(feature = "enable"))]
    {
        parse_macro_input!(args as Args);
        return item;
    }

    let args = parse_macro_input!(args as Args);
    let input = parse_macro_input!(item as ItemFn);

    let func_name = input.sig.ident.to_string();
    // check for async_trait-like patterns in the block, and instrument
    // the future instead of the wrapper
    let func_body = if let Some(internal_fun) =
        get_async_trait_info(&input.block, input.sig.asyncness.is_some())
    {
        // let's rewrite some statements!
        match internal_fun.kind {
            // async-trait <= 0.1.43
            AsyncTraitKind::Function => {
                unimplemented!(
                    "Please upgrade the crate `async-trait` to a version higher than 0.1.44"
                )
            }
            // async-trait >= 0.1.44
            AsyncTraitKind::Async(async_expr) => {
                // fallback if we couldn't find the '__async_trait' binding, might be
                // useful for crates exhibiting the same behaviors as async-trait
                let instrumented_block =
                    gen_block(&func_name, &async_expr.block, true, false, &args);
                let async_attrs = &async_expr.attrs;
                quote::quote! {
                    Box::pin(#(#async_attrs) * #instrumented_block)
                }
            }
        }
    } else {
        gen_block(
            &func_name,
            &input.block,
            input.sig.asyncness.is_some(),
            input.sig.asyncness.is_some(),
            &args,
        )
    };

    let ItemFn {
        attrs, vis, sig, ..
    } = input;

    let Signature {
        output: return_type,
        inputs: params,
        unsafety,
        constness,
        abi,
        ident,
        asyncness,
        generics:
            Generics {
                params: gen_params,
                where_clause,
                ..
            },
        ..
    } = sig;

    quote::quote!(
        #(#attrs) *
        #vis #constness #unsafety #asyncness #abi fn #ident<#gen_params>(#params) #return_type
        #where_clause
        {
            #func_body
        }
    )
    .into()
}

struct Args {
    name: Option<String>,
    short_name: bool,
    enter_on_poll: bool,
    properties: Vec<(String, String)>,
    crate_path: Path,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            name: None,
            short_name: false,
            enter_on_poll: false,
            properties: Vec::new(),
            crate_path: parse_quote!(::fastrace),
        }
    }
}

struct Property {
    key: String,
    value: String,
}

impl Parse for Property {
    fn parse(input: ParseStream) -> Result<Self> {
        let key: LitStr = input.parse()?;
        input.parse::<Token![:]>()?;
        let value: LitStr = input.parse()?;
        Ok(Property {
            key: key.value(),
            value: value.value(),
        })
    }
}

impl Parse for Args {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name = None;
        let mut short_name = false;
        let mut enter_on_poll = false;
        let mut properties = Vec::new();
        let mut crate_path = parse_quote!(::fastrace);
        let mut seen = HashSet::new();

        while !input.is_empty() {
            let key: Path = input.parse()?;
            let key = key
                .get_ident()
                .ok_or_else(|| Error::new(key.span(), "expected identifier"))?;
            if seen.contains(key) {
                return Err(Error::new(key.span(), "duplicate argument"));
            }
            seen.insert(key.clone());
            input.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "name" => {
                    let parsed_name: LitStr = input.parse()?;
                    name = Some(parsed_name.value());
                }
                "short_name" => {
                    let parsed_short_name: LitBool = input.parse()?;
                    short_name = parsed_short_name.value;
                }
                "enter_on_poll" => {
                    let parsed_enter_on_poll: LitBool = input.parse()?;
                    enter_on_poll = parsed_enter_on_poll.value;
                }
                "properties" => {
                    let content;
                    let _brace_token = braced!(content in input);
                    let property_list = content.parse_terminated(Property::parse, Token![,])?;
                    for property in property_list {
                        if properties.iter().any(|(k, _)| k == &property.key) {
                            return Err(Error::new(Span::call_site(), "duplicate property key"));
                        }
                        properties.push((property.key, property.value));
                    }
                }
                "crate" => {
                    let parsed_crate_path: Path = input.parse()?;
                    crate_path = parsed_crate_path;
                }
                _ => return Err(Error::new(Span::call_site(), "unexpected identifier")),
            }
            if !input.is_empty() {
                let _ = input.parse::<Token![,]>();
            }
        }

        Ok(Args {
            name,
            short_name,
            enter_on_poll,
            properties,
            crate_path,
        })
    }
}

fn gen_name(span: Span, func_name: &str, args: &Args) -> proc_macro2::TokenStream {
    let crate_path = &args.crate_path;
    match &args.name {
        Some(name) if name.is_empty() => {
            abort_call_site!("`name` can not be empty")
        }
        Some(_) if args.short_name => {
            abort_call_site!("`name` and `short_name` can not be used together")
        }
        Some(name) => {
            quote_spanned!(span=>
                #name
            )
        }
        None if args.short_name => {
            quote_spanned!(span=>
                #func_name
            )
        }
        None => {
            quote_spanned!(span=>
                #crate_path::func_path!()
            )
        }
    }
}

fn gen_properties(span: Span, args: &Args) -> proc_macro2::TokenStream {
    if args.properties.is_empty() {
        return quote::quote!();
    }

    if args.enter_on_poll {
        abort_call_site!("`enter_on_poll` can not be used with `properties`")
    }

    let properties = args.properties.iter().map(|(k, v)| {
        let k = k.as_str();
        let v = v.as_str();

        let (v, need_format) = unescape_format_string(v);

        if need_format {
            quote_spanned!(span=>
                (std::borrow::Cow::from(#k), std::borrow::Cow::from(format!(#v)))
            )
        } else {
            quote_spanned!(span=>
                (std::borrow::Cow::from(#k), std::borrow::Cow::from(#v))
            )
        }
    });
    let properties = Punctuated::<_, Token![,]>::from_iter(properties);
    quote_spanned!(span=>
        .with_properties(|| [ #properties ])
    )
}

fn unescape_format_string(s: &str) -> (String, bool) {
    let unescaped_delete = s.replace("{{", "").replace("}}", "");
    let contains_valid_format_string =
        unescaped_delete.contains('{') || unescaped_delete.contains('}');
    if contains_valid_format_string {
        (s.to_string(), true)
    } else {
        let unescaped_replace = s.replace("{{", "{").replace("}}", "}");
        (unescaped_replace, false)
    }
}

/// Instrument a block
fn gen_block(
    func_name: &str,
    block: &Block,
    async_context: bool,
    async_keyword: bool,
    args: &Args,
) -> proc_macro2::TokenStream {
    let name = gen_name(block.span(), func_name, args);
    let properties = gen_properties(block.span(), args);
    let crate_path = &args.crate_path;

    // Generate the instrumented function body.
    // If the function is an `async fn`, this will wrap it in an async block.
    // Otherwise, this will enter the span and then perform the rest of the body.
    if async_context {
        let block = if args.enter_on_poll {
            quote_spanned!(block.span()=>
                #crate_path::future::FutureExt::enter_on_poll(
                    async move { #block },
                    #name
                )
            )
        } else {
            quote_spanned!(block.span()=>
                {
                    let __span__ = #crate_path::Span::enter_with_local_parent( #name ) #properties;
                    #crate_path::future::FutureExt::in_span(
                        async move { #block },
                        __span__,
                    )
                }
            )
        };

        if async_keyword {
            quote_spanned!(block.span()=>
                #block.await
            )
        } else {
            block
        }
    } else {
        if args.enter_on_poll {
            abort_call_site!("`enter_on_poll` can not be applied on non-async function");
        }

        quote_spanned!(block.span()=>
            let __guard__ = #crate_path::local::LocalSpan::enter_with_local_parent( #name ) #properties;
            #block
        )
    }
}

enum AsyncTraitKind<'a> {
    // old construction. Contains the function
    Function,
    // new construction. Contains a reference to the async block
    Async(&'a ExprAsync),
}

struct AsyncTraitInfo<'a> {
    // statement that must be patched
    _source_stmt: &'a Stmt,
    kind: AsyncTraitKind<'a>,
}

// Get the AST of the inner function we need to hook, if it was generated
// by async-trait.
// When we are given a function annotated by async-trait, that function
// is only a placeholder that returns a pinned future containing the
// user logic, and it is that pinned future that needs to be instrumented.
// Were we to instrument its parent, we would only collect information
// regarding the allocation of that future, and not its own span of execution.
// Depending on the version of async-trait, we inspect the block of the function
// to find if it matches the pattern
// `async fn foo<...>(...) {...}; Box::pin(foo<...>(...))` (<=0.1.43), or if
// it matches `Box::pin(async move { ... }) (>=0.1.44). We the return the
// statement that must be instrumented, along with some other information.
// 'gen_body' will then be able to use that information to instrument the
// proper function/future.
// (this follows the approach suggested in
// https://github.com/dtolnay/async-trait/issues/45#issuecomment-571245673)
fn get_async_trait_info(block: &Block, block_is_async: bool) -> Option<AsyncTraitInfo<'_>> {
    // are we in an async context? If yes, this isn't an async_trait-like pattern
    if block_is_async {
        return None;
    }

    // list of async functions declared inside the block
    let inside_funs = block.stmts.iter().filter_map(|stmt| {
        if let Stmt::Item(Item::Fn(fun)) = &stmt {
            // If the function is async, this is a candidate
            if fun.sig.asyncness.is_some() {
                return Some((stmt, fun));
            }
        }
        None
    });

    // last expression of the block (it determines the return value
    // of the block, so that if we are working on a function whose
    // `trait` or `impl` declaration is annotated by async_trait,
    // this is quite likely the point where the future is pinned)
    let (last_expr_stmt, last_expr) = block.stmts.iter().rev().find_map(|stmt| {
        if let Stmt::Expr(expr, None) = stmt {
            Some((stmt, expr))
        } else {
            None
        }
    })?;

    // is the last expression a function call?
    let (outside_func, outside_args) = match last_expr {
        Expr::Call(ExprCall { func, args, .. }) => (func, args),
        _ => return None,
    };

    // is it a call to `Box::pin()`?
    let path = match outside_func.as_ref() {
        Expr::Path(path) => &path.path,
        _ => return None,
    };
    if !path_to_string(path).ends_with("Box::pin") {
        return None;
    }

    // Does the call take an argument? If it doesn't,
    // it's not going to compile anyway, but that's no reason
    // to (try to) perform an out-of-bounds access
    if outside_args.is_empty() {
        return None;
    }

    // Is the argument to Box::pin an async block that
    // captures its arguments?
    if let Expr::Async(async_expr) = &outside_args[0] {
        // check that the move 'keyword' is present
        async_expr.capture?;

        return Some(AsyncTraitInfo {
            _source_stmt: last_expr_stmt,
            kind: AsyncTraitKind::Async(async_expr),
        });
    }

    // Is the argument to Box::pin a function call itself?
    let func = match &outside_args[0] {
        Expr::Call(ExprCall { func, .. }) => func,
        _ => return None,
    };

    // "stringify" the path of the function called
    let func_name = match **func {
        Expr::Path(ref func_path) => path_to_string(&func_path.path),
        _ => return None,
    };

    // Was that function defined inside the current block?
    // If so, retrieve the statement where it was declared and the function itself
    let (stmt_func_declaration, _) = inside_funs
        .into_iter()
        .find(|(_, fun)| fun.sig.ident == func_name)?;

    Some(AsyncTraitInfo {
        _source_stmt: stmt_func_declaration,
        kind: AsyncTraitKind::Function,
    })
}

// Return a path as a String
fn path_to_string(path: &Path) -> String {
    use std::fmt::Write;
    // some heuristic to prevent too many allocations
    let mut res = String::with_capacity(path.segments.len() * 5);
    for i in 0..path.segments.len() {
        write!(res, "{}", path.segments[i].ident).expect("writing to a String should never fail");
        if i < path.segments.len() - 1 {
            res.push_str("::");
        }
    }
    res
}
