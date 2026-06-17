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

use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::quote;
use quote::quote_spanned;
use syn::Block;
use syn::Error;
use syn::Expr;
use syn::ExprAsync;
use syn::ExprCall;
use syn::Generics;
use syn::Item;
use syn::ItemFn;
use syn::LitStr;
use syn::Path;
use syn::Result;
use syn::ReturnType;
use syn::Signature;
use syn::Stmt;
use syn::Token;
use syn::Type;
use syn::TypeInfer;
use syn::parse_quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit_mut;
use syn::visit_mut::VisitMut;

use crate::args::Args;

pub(crate) fn gen_trace(args: Args, input: ItemFn) -> Result<TokenStream> {
    let func_name = &input.sig.ident;

    // Check for async_trait-like patterns in the block, and instrument
    // the future instead of the wrapper.
    let func_body = if let Some(internal_fun) =
        get_async_trait_info(&input.block, input.sig.asyncness.is_some())
    {
        match internal_fun.kind {
            // async-trait <= 0.1.43
            AsyncTraitKind::Function => {
                unimplemented!(
                    "Please upgrade the crate `async-trait` to a version higher than 0.1.44"
                )
            }
            // async-trait >= 0.1.44
            AsyncTraitKind::Async(async_expr) => {
                let instrumented_block =
                    gen_block(func_name, &async_expr.block, true, false, &args, None)?;
                let async_attrs = &async_expr.attrs;
                quote! {
                    Box::pin(#(#async_attrs) * #instrumented_block)
                }
            }
        }
    } else {
        let output_ty = match &input.sig.output {
            ReturnType::Type(_, ty) => (**ty).clone(),
            ReturnType::Default => parse_quote! { () },
        };
        gen_block(
            func_name,
            &input.block,
            input.sig.asyncness.is_some(),
            input.sig.asyncness.is_some(),
            &args,
            Some(output_ty),
        )?
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

    let fn_span = ident.span();
    Ok(quote_spanned!(fn_span=>
        #(#attrs) *
        #vis #constness #unsafety #asyncness #abi fn #ident<#gen_params>(#params) #return_type
        #where_clause
        {
            #func_body
        }
    ))
}

fn gen_span_name(func_name: &Ident, args: &Args) -> Result<TokenStream> {
    let Args {
        name,
        short_name,
        crate_path,
        ..
    } = args;

    if let Some(span_name) = name {
        if span_name.value().is_empty() {
            Err(Error::new(Span::call_site(), "`name` can not be empty"))
        } else if *short_name {
            Err(Error::new(
                Span::call_site(),
                "`name` and `short_name` can not be used together",
            ))
        } else {
            Ok(name.into_token_stream())
        }
    } else {
        if *short_name {
            Ok(LitStr::new(&func_name.to_string(), func_name.span()).into_token_stream())
        } else {
            Ok(quote!(#crate_path::func_path!()))
        }
    }
}

fn gen_properties(args: &Args) -> Result<TokenStream> {
    if args.properties.is_empty() {
        return Ok(quote!());
    }

    if args.enter_on_poll {
        return Err(Error::new(
            Span::call_site(),
            "`enter_on_poll` can not be used with `properties`",
        ));
    }

    let properties = args.properties.iter().map(|p| {
        let k = &p.key;
        let v = &p.value;
        quote!(
            (std::borrow::Cow::from(#k), match format_args!(#v) {
                __f => if let Some(__s) = __f.as_str() {
                    std::borrow::Cow::from(__s)
                } else {
                    std::borrow::Cow::from(std::string::ToString::to_string(&__f))
                }
            })
        )
    });
    let properties = Punctuated::<_, Token![,]>::from_iter(properties);
    Ok(quote!(
        .with_properties(|| [ #properties ])
    ))
}

fn gen_block(
    func_name: &Ident,
    block: &Block,
    async_context: bool,
    async_keyword: bool,
    args: &Args,
    output_ty: Option<Type>,
) -> Result<TokenStream> {
    let name = gen_span_name(func_name, args)?;
    let properties = gen_properties(args)?;
    let crate_path = &args.crate_path;

    let output_ty_hint = if let Some(mut ty) = output_ty {
        // Replaces `impl Trait` with `_`, so that it can be used as the type
        // in the LHS of `let` statements.
        struct EraseImplTrait;

        impl VisitMut for EraseImplTrait {
            fn visit_type_mut(&mut self, ty: &mut Type) {
                if let Type::ImplTrait(..) = ty {
                    *ty = Type::Infer(TypeInfer {
                        underscore_token: Token![_](ty.span()),
                    });
                } else {
                    visit_mut::visit_type_mut(self, ty);
                }
            }
        }

        EraseImplTrait.visit_type_mut(&mut ty);
        ty
    } else {
        parse_quote!(_)
    };

    // Generate the instrumented function body.
    // If the function is an `async fn`, this will wrap it in an async block.
    // Otherwise, this will enter the span and then perform the rest of the body.
    if async_context {
        let block = if args.enter_on_poll {
            quote!(
                #crate_path::future::FutureExt::enter_on_poll(
                    async move { #block },
                    #name
                )
            )
        } else {
            quote!(
                {
                    let __span__ = #crate_path::Span::enter_with_local_parent( #name ) #properties;
                    #crate_path::future::FutureExt::in_span(
                        async move {
                            let __ret__: #output_ty_hint = #block;
                            #[allow(unreachable_code)]
                            __ret__
                        },
                        __span__,
                    )
                }
            )
        };

        if async_keyword {
            Ok(quote!(
                #block.await
            ))
        } else {
            Ok(block)
        }
    } else {
        if args.enter_on_poll {
            Err(Error::new(
                Span::call_site(),
                "`enter_on_poll` can not be applied on non-async function",
            ))
        } else {
            Ok(quote!(
                let __guard__ = #crate_path::local::LocalSpan::enter_with_local_parent( #name ) #properties;
                #block
            ))
        }
    }
}

enum AsyncTraitKind<'a> {
    // old construction. Contains the function
    Function,
    // new construction. Contains a reference to the async block
    Async(&'a ExprAsync),
}

struct AsyncTraitInfo<'a> {
    // source statement to be patched
    #[expect(unused)]
    stmt: &'a Stmt,
    kind: AsyncTraitKind<'a>,
}

/// Get the AST of the inner function we need to hook, if it was generated by `async-trait`.
///
/// When we are given a function annotated by `async-trait`, that function is only a placeholder
/// that returns a pinned future containing the user logic, and it is that pinned future that needs
/// to be instrumented. Were we to instrument its parent, we would only collect information
/// regarding the allocation of that future, and not its own span of execution.
///
/// Depending on the version of async-trait, we inspect the block of the function to find if it
/// matches the pattern:
///
/// ```rust,ignore
/// // for async-trait <=0.1.43
/// async fn foo<...>(...) {...}
/// Box::pin(foo<...>(...))
///
/// // for async-trait >= 0.1.44
/// Box::pin(async move { ... })
/// ```
///
/// We then return the statement to be instrumented, along with some other information.
/// [`gen_block`] will then be able to use that information to instrument the proper
/// function or future.
///
/// This follows the approach suggested in https://github.com/dtolnay/async-trait/issues/45#issuecomment-571245673.
fn get_async_trait_info(block: &Block, block_is_async: bool) -> Option<AsyncTraitInfo<'_>> {
    // Are we in an async context? If yes, this isn't an async_trait-like pattern
    if block_is_async {
        return None;
    }

    // list of async functions declared inside the block
    let inside_fns = block.stmts.iter().filter_map(|stmt| {
        if let Stmt::Item(Item::Fn(fun)) = &stmt {
            // If the function is async, this is a candidate
            if fun.sig.asyncness.is_some() {
                return Some((stmt, fun));
            }
        }
        None
    });

    // Last expression of the block
    //
    // This determines the return value of the block. Thus, if we are working on a function whose
    // `trait` or `impl` declaration is annotated by async_trait, this is quite likely the point
    // where the future is pinned.
    let (last_expr_stmt, last_expr) = block.stmts.iter().rev().find_map(|stmt| {
        if let Stmt::Expr(expr, None) = stmt {
            Some((stmt, expr))
        } else {
            None
        }
    })?;

    // Is the last expression a function call?
    let (outside_func, outside_args) = match last_expr {
        Expr::Call(ExprCall { func, args, .. }) => (func, args),
        _ => return None,
    };

    // Is it a call to `Box::pin()`?
    let path = match outside_func.as_ref() {
        Expr::Path(path) => &path.path,
        _ => return None,
    };
    if !path_to_string(path).ends_with("Box::pin") {
        return None;
    }

    // Does the call take an argument?
    //
    // If it doesn't, it's not going to compile anyway, but that's no reason to perform an
    // out-of-bounds access
    if outside_args.is_empty() {
        return None;
    }

    // Is the argument to Box::pin an async block that captures its arguments?
    if let Expr::Async(async_expr) = &outside_args[0] {
        // check that the move 'keyword' is present
        async_expr.capture?;

        return Some(AsyncTraitInfo {
            stmt: last_expr_stmt,
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
    //
    // If so, retrieve the statement where it was declared and the function itself.
    let (stmt_func_declaration, _) = inside_fns
        .into_iter()
        .find(|(_, fun)| fun.sig.ident == func_name)?;

    Some(AsyncTraitInfo {
        stmt: stmt_func_declaration,
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
