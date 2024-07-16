use std::borrow::Cow;

use minstant::Instant;

use crate::collector::SpanId;
use crate::util::Properties;

#[derive(Debug)]
pub struct RawSpan {
    pub id: SpanId,
    pub parent_id: SpanId,
    pub begin_instant: Instant,
    pub name: Cow<'static, str>,
    pub properties: Properties,
    pub is_event: bool,

    // Will write this field at post processing
    pub end_instant: Instant,
}

impl RawSpan {
    #[inline]
    pub(crate) fn begin_with(
        id: SpanId,
        parent_id: SpanId,
        begin_instant: Instant,
        name: impl Into<Cow<'static, str>>,
        is_event: bool,
    ) -> Self {
        RawSpan {
            id,
            parent_id,
            begin_instant,
            name: name.into(),
            properties: Properties::default(),
            is_event,
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
        let mut properties = Properties::default();
        properties.extend(self.properties.iter().cloned());

        RawSpan {
            id: self.id,
            parent_id: self.parent_id,
            begin_instant: self.begin_instant,
            name: self.name.clone(),
            properties,
            is_event: self.is_event,
            end_instant: self.end_instant,
        }
    }
}
