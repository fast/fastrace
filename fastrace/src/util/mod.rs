// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

pub mod legacy_spsc;
pub mod object_pool;
pub mod spsc;
#[doc(hidden)]
pub mod tree;

use std::borrow::Cow;
use std::cell::RefCell;

use once_cell::sync::Lazy;

use crate::collector::CollectTokenItem;
use crate::local::raw_span::RawSpan;
use crate::util::object_pool::Pool;
use crate::util::object_pool::Puller;
use crate::util::object_pool::Reusable;

static RAW_SPANS_POOL: Lazy<Pool<Vec<RawSpan>>> = Lazy::new(|| Pool::new(Vec::new, Vec::clear));
static COLLECT_TOKEN_ITEMS_POOL: Lazy<Pool<Vec<CollectTokenItem>>> =
    Lazy::new(|| Pool::new(Vec::new, Vec::clear));
#[allow(clippy::type_complexity)]
static PROPERTIES_POOL: Lazy<Pool<Vec<(Cow<'static, str>, Cow<'static, str>)>>> =
    Lazy::new(|| Pool::new(Vec::new, Vec::clear));

thread_local! {
    static RAW_SPANS_PULLER: RefCell<Puller<'static, Vec<RawSpan>>> = RefCell::new(RAW_SPANS_POOL.puller(512));
    static COLLECT_TOKEN_ITEMS_PULLER: RefCell<Puller<'static, Vec<CollectTokenItem>>>  = RefCell::new(COLLECT_TOKEN_ITEMS_POOL.puller(512));
    #[allow(clippy::type_complexity)]
    static PROPERTIES_PULLER: RefCell<Puller<'static, Vec<(Cow<'static, str>, Cow<'static, str>)>>>  = RefCell::new(PROPERTIES_POOL.puller(512));
}

pub type RawSpans = Reusable<'static, Vec<RawSpan>>;
pub type CollectToken = Reusable<'static, Vec<CollectTokenItem>>;
pub type Properties = Reusable<'static, Vec<(Cow<'static, str>, Cow<'static, str>)>>;

impl Default for RawSpans {
    fn default() -> Self {
        RAW_SPANS_PULLER
            .try_with(|puller| puller.borrow_mut().pull())
            .unwrap_or_else(|_| Reusable::new(&*RAW_SPANS_POOL, vec![]))
    }
}

impl Default for Properties {
    fn default() -> Self {
        PROPERTIES_PULLER
            .try_with(|puller| puller.borrow_mut().pull())
            .unwrap_or_else(|_| Reusable::new(&*PROPERTIES_POOL, vec![]))
    }
}

fn new_collect_token(items: impl IntoIterator<Item = CollectTokenItem>) -> CollectToken {
    let mut token = COLLECT_TOKEN_ITEMS_PULLER
        .try_with(|puller| puller.borrow_mut().pull())
        .unwrap_or_else(|_| Reusable::new(&*COLLECT_TOKEN_ITEMS_POOL, vec![]));
    token.extend(items);
    token
}

impl FromIterator<RawSpan> for RawSpans {
    fn from_iter<T: IntoIterator<Item = RawSpan>>(iter: T) -> Self {
        let mut raw_spans = RawSpans::default();
        raw_spans.extend(iter);
        raw_spans
    }
}

impl FromIterator<CollectTokenItem> for CollectToken {
    fn from_iter<T: IntoIterator<Item = CollectTokenItem>>(iter: T) -> Self {
        new_collect_token(iter)
    }
}

impl<'a> FromIterator<&'a CollectTokenItem> for CollectToken {
    fn from_iter<T: IntoIterator<Item = &'a CollectTokenItem>>(iter: T) -> Self {
        new_collect_token(iter.into_iter().copied())
    }
}

impl From<CollectTokenItem> for CollectToken {
    fn from(item: CollectTokenItem) -> Self {
        new_collect_token([item])
    }
}
