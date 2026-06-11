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

use std::collections::HashSet;

use proc_macro2::Span;
use syn::Error;
use syn::LitBool;
use syn::LitStr;
use syn::Path;
use syn::Result;
use syn::Token;
use syn::braced;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_quote;
use syn::spanned::Spanned;

#[cfg_attr(not(feature = "enable"), allow(dead_code))]
pub struct Property {
    pub key: LitStr,
    pub value: LitStr,
}

impl Parse for Property {
    fn parse(input: ParseStream) -> Result<Self> {
        let key: LitStr = input.parse()?;
        input.parse::<Token![:]>()?;
        let value: LitStr = input.parse()?;
        Ok(Property { key, value })
    }
}

#[cfg_attr(not(feature = "enable"), allow(dead_code))]
pub struct Args {
    pub name: Option<LitStr>,
    pub short_name: bool,
    pub enter_on_poll: bool,
    pub properties: Vec<Property>,
    pub crate_path: Path,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name = None;
        let mut short_name = false;
        let mut enter_on_poll = false;
        let mut properties = vec![];
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
                    name = Some(parsed_name);
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
                        if properties.iter().any(|p: &Property| p.key == property.key) {
                            return Err(Error::new(
                                Span::call_site(),
                                format!("duplicate property key: {}", property.key.value()),
                            ));
                        }
                        properties.push(property);
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
