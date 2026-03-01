// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

pub mod command_bus;
pub mod spsc;
#[doc(hidden)]
pub mod tree;

use std::borrow::Cow;

use crate::local::raw_span::RawSpan;

pub type RawSpans = Vec<RawSpan>;
pub type Properties = Vec<(Cow<'static, str>, Cow<'static, str>)>;
