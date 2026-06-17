// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;

use fastant::Instant;

use crate::collector::SpanContext;
use crate::collector::SpanId;
use crate::util::Properties;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RawKind {
    Span,
    Event,
    Properties,
    Link,
}

#[derive(Debug, Clone)]
pub struct RawSpan {
    pub id: SpanId,
    pub parent_id: Option<SpanId>,
    pub begin_instant: Instant,
    pub name: Cow<'static, str>,
    pub properties: Properties,
    pub links: Vec<SpanContext>,
    pub raw_kind: RawKind,

    // Will write this field at post processing
    pub end_instant: Instant,
}

impl RawSpan {
    #[inline]
    pub(crate) fn begin_with(
        id: SpanId,
        parent_id: Option<SpanId>,
        begin_instant: Instant,
        name: impl Into<Cow<'static, str>>,
        raw_kind: RawKind,
    ) -> Self {
        RawSpan {
            id,
            parent_id,
            begin_instant,
            name: name.into(),
            properties: Properties::default(),
            links: Vec::new(),
            raw_kind,
            end_instant: Instant::ZERO,
        }
    }

    #[inline]
    pub(crate) fn end_with(&mut self, end_instant: Instant) {
        self.end_instant = end_instant;
    }
}
