//! Thrift components defined in [jaeger.thrift].
//!
//! [jaeger.thrift]: https://github.com/uber/jaeger-idl/blob/main/thrift/jaeger.thrift

use thrift_codec::data::Field;
use thrift_codec::data::List;
use thrift_codec::data::Struct;
use thrift_codec::message::Message;

#[derive(Debug, Clone)]
pub struct EmitBatchNotification {
    pub batch: Batch,
}

impl From<EmitBatchNotification> for Message {
    fn from(f: EmitBatchNotification) -> Self {
        Message::oneway("emitBatch", 0, Struct::from((Struct::from(f.batch),)))
    }
}

/// `TagKind` denotes the kind of a `Tag`'s value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TagKind {
    String = 0,
    Double = 1,
    Bool = 2,
    Long = 3,
    Binary = 4,
}

/// `Tag` is a basic strongly typed key/value pair.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Tag {
    String { key: String, value: String },
    Double { key: String, value: f64 },
    Bool { key: String, value: bool },
    Long { key: String, value: i64 },
    Binary { key: String, value: Vec<u8> },
}

impl Tag {
    /// Returns the key of this tag.
    pub fn key(&self) -> &str {
        match *self {
            Tag::String { ref key, .. }
            | Tag::Double { ref key, .. }
            | Tag::Bool { ref key, .. }
            | Tag::Long { ref key, .. }
            | Tag::Binary { ref key, .. } => key,
        }
    }

    /// Returns the kind of this tag.
    pub fn kind(&self) -> TagKind {
        match *self {
            Tag::String { .. } => TagKind::String,
            Tag::Double { .. } => TagKind::Double,
            Tag::Bool { .. } => TagKind::Bool,
            Tag::Long { .. } => TagKind::Long,
            Tag::Binary { .. } => TagKind::Binary,
        }
    }
}

impl From<Tag> for Struct {
    fn from(f: Tag) -> Self {
        let mut fields = vec![Field::new(1, f.key()), Field::new(2, f.kind() as i32)];
        match f {
            Tag::String { value, .. } => fields.push(Field::new(3, value)),
            Tag::Double { value, .. } => fields.push(Field::new(4, value)),
            Tag::Bool { value, .. } => fields.push(Field::new(5, value)),
            Tag::Long { value, .. } => fields.push(Field::new(6, value)),
            Tag::Binary { value, .. } => fields.push(Field::new(7, value)),
        };
        Struct::new(fields)
    }
}

/// `Log` is a timed even with an arbitrary set of tags.
#[derive(Debug, Clone)]
pub struct Log {
    pub timestamp: i64,
    pub fields: Vec<Tag>,
}

impl From<Log> for Struct {
    fn from(f: Log) -> Self {
        Struct::from((
            f.timestamp,
            List::from(f.fields.into_iter().map(Struct::from).collect::<Vec<_>>()),
        ))
    }
}

/// Span reference kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpanRefKind {
    // ChildOf = 0,
    FollowsFrom = 1,
}

/// `SpanRef` describes causal relationship of the current span to another span (e.g. 'child-of')
#[derive(Debug, Clone)]
pub struct SpanRef {
    pub kind: SpanRefKind,
    pub trace_id_low: i64,
    pub trace_id_high: i64,
    pub span_id: i64,
}

impl From<SpanRef> for Struct {
    fn from(f: SpanRef) -> Self {
        Struct::from((f.kind as i32, f.trace_id_low, f.trace_id_high, f.span_id))
    }
}

/// `JaegerSpan` represents a named unit of work performed by a service.
#[derive(Debug, Clone)]
pub struct JaegerSpan {
    /// The least significant 64 bits of a trace id.
    pub trace_id_low: i64,

    /// The most significant 64 bits of a trace id; 0 when only 64 bit IDs are used.
    pub trace_id_high: i64,

    /// Unique span id (only unique within a given trace).
    pub span_id: i64,

    /// Since nearly all spans will have parents spans, `ChildOf` refs do not have to be explicit.
    ///
    /// Should be `0` if the current span is a root span.
    pub parent_span_id: i64,

    /// The name of operation.
    pub operation_name: String,

    /// Causal references to other spans.
    pub references: Vec<SpanRef>,

    /// A bit field used to propagate sampling decisions.
    ///
    /// `1` signifies a SAMPLED span, `2` signifies a DEBUG span.
    pub flags: i32,

    /// Start time of this span.
    pub start_time: i64,

    /// Duration of this span.
    pub duration: i64,

    /// Tag list.
    pub tags: Vec<Tag>,

    /// Log list.
    pub logs: Vec<Log>,
}

impl From<JaegerSpan> for Struct {
    fn from(f: JaegerSpan) -> Self {
        let mut fields = Vec::with_capacity(11);
        fields.push(Field::new(1, f.trace_id_low));
        fields.push(Field::new(2, f.trace_id_high));
        fields.push(Field::new(3, f.span_id));
        fields.push(Field::new(4, f.parent_span_id));
        fields.push(Field::new(5, f.operation_name));
        if !f.references.is_empty() {
            fields.push(Field::new(
                6,
                List::from(
                    f.references
                        .into_iter()
                        .map(Struct::from)
                        .collect::<Vec<_>>(),
                ),
            ));
        }
        fields.push(Field::new(7, f.flags));
        fields.push(Field::new(8, f.start_time));
        fields.push(Field::new(9, f.duration));
        if !f.tags.is_empty() {
            fields.push(Field::new(
                10,
                List::from(f.tags.into_iter().map(Struct::from).collect::<Vec<_>>()),
            ));
        }
        if !f.logs.is_empty() {
            fields.push(Field::new(
                11,
                List::from(f.logs.into_iter().map(Struct::from).collect::<Vec<_>>()),
            ));
        }
        Struct::new(fields)
    }
}

/// `Process` describes the traced process/service that emits spans.
#[derive(Debug, Clone)]
pub struct Process {
    /// The name of this service.
    pub service_name: String,

    /// Tag list.
    pub tags: Vec<Tag>,
}

impl From<Process> for Struct {
    fn from(f: Process) -> Self {
        let tags = List::from(f.tags.into_iter().map(Struct::from).collect::<Vec<_>>());
        if tags.is_empty() {
            Struct::from((f.service_name,))
        } else {
            Struct::from((f.service_name, tags))
        }
    }
}

/// `Batch` is a collection of spans reported out of process.
#[derive(Debug, Clone)]
pub struct Batch {
    pub process: Process,
    pub spans: Vec<JaegerSpan>,
}

impl From<Batch> for Struct {
    fn from(f: Batch) -> Self {
        Struct::from((
            Struct::from(f.process),
            List::from(f.spans.into_iter().map(Struct::from).collect::<Vec<_>>()),
        ))
    }
}
