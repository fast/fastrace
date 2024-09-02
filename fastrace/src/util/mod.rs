// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

pub mod legacy_spsc;
pub mod object_pool;
pub mod spsc;
#[doc(hidden)]
pub mod tree;

use std::borrow::Cow;
use std::cell::RefCell;

use crate::collector::CollectTokenItem;
use crate::local::raw_span::RawSpan;
use crate::util::object_pool::GlobalVecPool;
use crate::util::object_pool::LocalVecPool;
use crate::util::object_pool::ReusableVec;

static RAW_SPANS_POOL: GlobalVecPool<RawSpan> = GlobalVecPool::new();
static COLLECT_TOKEN_ITEMS_POOL: GlobalVecPool<CollectTokenItem> = GlobalVecPool::new();
static PROPERTIES_POOL: GlobalVecPool<(Cow<'static, str>, Cow<'static, str>)> =
    GlobalVecPool::new();

thread_local! {
    static RAW_SPANS_PULLER: RefCell<LocalVecPool<RawSpan>> = RefCell::new(RAW_SPANS_POOL.new_local(512));
    static COLLECT_TOKEN_ITEMS_PULLER: RefCell<LocalVecPool<CollectTokenItem>>  = RefCell::new(COLLECT_TOKEN_ITEMS_POOL.new_local(512));
    #[allow(clippy::type_complexity)]
    static PROPERTIES_PULLER: RefCell<LocalVecPool<(Cow<'static, str>, Cow<'static, str>)>>  = RefCell::new(PROPERTIES_POOL.new_local(512));
}

pub type RawSpans = ReusableVec<RawSpan>;
pub type CollectToken = ReusableVec<CollectTokenItem>;
pub type Properties = ReusableVec<(Cow<'static, str>, Cow<'static, str>)>;

impl Default for RawSpans {
    fn default() -> Self {
        RAW_SPANS_PULLER
            .try_with(|puller| puller.borrow_mut().take())
            .unwrap_or_else(|_| Self::new(&RAW_SPANS_POOL, Vec::new()))
    }
}

impl Default for Properties {
    fn default() -> Self {
        PROPERTIES_PULLER
            .try_with(|puller| puller.borrow_mut().take())
            .unwrap_or_else(|_| Self::new(&PROPERTIES_POOL, Vec::new()))
    }
}

fn new_collect_token(items: impl IntoIterator<Item = CollectTokenItem>) -> CollectToken {
    let mut token = COLLECT_TOKEN_ITEMS_PULLER
        .try_with(|puller| puller.borrow_mut().take())
        .unwrap_or_else(|_| CollectToken::new(&COLLECT_TOKEN_ITEMS_POOL, Vec::new()));
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
