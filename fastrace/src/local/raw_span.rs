// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;

use fastant::Instant;

use crate::collector::SpanId;
use crate::util::Properties;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RawKind {
    Span,
    Event,
    Properties,
}

#[derive(Debug)]
pub struct RawSpan {
    pub id: SpanId,
    pub parent_id: Option<SpanId>,
    pub begin_instant: Instant,
    pub name: Cow<'static, str>,
    pub properties: Option<Properties>,
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
            properties: None,
            raw_kind,
            end_instant: Instant::ZERO,
        }
    }

    #[inline]
    pub(crate) fn end_with(&mut self, end_instant: Instant) {
        self.end_instant = end_instant;
    }
}

impl Clone for RawSpan {
    fn clone(&self) -> Self {
        let properties = self.properties.as_ref().map(|properties| {
            let mut new_properties = Properties::default();
            new_properties.extend(properties.iter().cloned());
            new_properties
        });

        RawSpan {
            id: self.id,
            parent_id: self.parent_id,
            begin_instant: self.begin_instant,
            name: self.name.clone(),
            properties,
            raw_kind: self.raw_kind,
            end_instant: self.end_instant,
        }
    }
}
