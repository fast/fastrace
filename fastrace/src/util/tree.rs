// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! A module for relationship checking in test

use std::collections::HashMap;
use std::fmt;

use crate::collector::SpanId;
use crate::collector::SpanRecord;
use crate::collector::SpanSet;
use crate::util::CollectToken;
use crate::util::RawSpans;

type TreeChildren = HashMap<
    Option<SpanId>,
    (
        String,
        Vec<SpanId>,
        Vec<(String, String)>,
        Vec<(String, Vec<(String, String)>)>,
    ),
>;

#[derive(Debug, PartialOrd, PartialEq, Ord, Eq)]
pub struct Tree {
    name: String,
    children: Vec<Tree>,
    properties: Vec<(String, String)>,
    events: Vec<(String, Vec<(String, String)>)>,
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_depth(f, 0)
    }
}

impl Tree {
    fn fmt_with_depth(&self, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
        write!(
            f,
            "{:indent$}{} {:?}",
            "",
            self.name,
            self.properties,
            indent = depth * 4
        )?;
        // TODO: also optionally print properties.
        if !self.events.is_empty() {
            write!(f, " {:?}", self.events)?;
        }
        writeln!(f)?;
        for child in &self.children {
            child.fmt_with_depth(f, depth + 1)?;
        }
        Ok(())
    }
}

impl Tree {
    pub fn sort(&mut self) {
        for child in &mut self.children {
            child.sort();
        }
        self.children.as_mut_slice().sort_unstable();
    }

    pub fn from_raw_spans(raw_spans: RawSpans) -> Vec<Tree> {
        let mut children: TreeChildren = HashMap::new();

        children.insert(None, ("".into(), vec![], vec![], vec![]));
        for span in &raw_spans {
            children.insert(
                Some(span.id),
                (
                    span.name.to_string(),
                    vec![],
                    span.properties
                        .as_ref()
                        .map(|properties| {
                            properties
                                .iter()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    vec![],
                ),
            );
        }
        for span in &raw_spans {
            children
                .get_mut(&span.parent_id)
                .as_mut()
                .unwrap()
                .1
                .push(span.id);
        }

        let mut t = Self::build_tree(None, &mut children);
        t.sort();
        t.children
    }

    /// Return a vector of collect id -> Tree
    pub fn from_span_sets(span_sets: &[(SpanSet, CollectToken)]) -> Vec<(usize, Tree)> {
        let mut collect = HashMap::<
            usize,
            HashMap<
                Option<SpanId>,
                (
                    String,
                    Vec<SpanId>,
                    Vec<(String, String)>,
                    Vec<(String, Vec<(String, String)>)>,
                ),
            >,
        >::new();

        for (span_set, token) in span_sets {
            for item in token.iter() {
                collect
                    .entry(item.collect_id)
                    .or_default()
                    .insert(Some(SpanId(0)), ("".into(), vec![], vec![], vec![]));
                match span_set {
                    SpanSet::Span(span) => {
                        collect.entry(item.collect_id).or_default().insert(
                            Some(span.id),
                            (
                                span.name.to_string(),
                                vec![],
                                span.properties
                                    .as_ref()
                                    .map(|properties| {
                                        properties
                                            .iter()
                                            .map(|(k, v)| (k.to_string(), v.to_string()))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                vec![],
                            ),
                        );
                    }
                    SpanSet::LocalSpansInner(spans) => {
                        for span in spans.spans.iter() {
                            collect.entry(item.collect_id).or_default().insert(
                                Some(span.id),
                                (
                                    span.name.to_string(),
                                    vec![],
                                    span.properties
                                        .as_ref()
                                        .map(|properties| {
                                            properties
                                                .iter()
                                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                    vec![],
                                ),
                            );
                        }
                    }
                    SpanSet::SharedLocalSpans(spans) => {
                        for span in spans.spans.iter() {
                            collect.entry(item.collect_id).or_default().insert(
                                Some(span.id),
                                (
                                    span.name.to_string(),
                                    vec![],
                                    span.properties
                                        .as_ref()
                                        .map(|properties| {
                                            properties
                                                .iter()
                                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                    vec![],
                                ),
                            );
                        }
                    }
                }
            }
        }

        for (span_set, token) in span_sets {
            for item in token.iter() {
                match span_set {
                    SpanSet::Span(span) => {
                        let parent_id = span.parent_id.unwrap_or(item.parent_id);
                        collect
                            .get_mut(&item.collect_id)
                            .as_mut()
                            .unwrap()
                            .get_mut(&Some(parent_id))
                            .as_mut()
                            .unwrap()
                            .1
                            .push(span.id);
                    }
                    SpanSet::LocalSpansInner(spans) => {
                        for span in spans.spans.iter() {
                            let parent_id = span.parent_id.unwrap_or(item.parent_id);
                            collect
                                .get_mut(&item.collect_id)
                                .as_mut()
                                .unwrap()
                                .get_mut(&Some(parent_id))
                                .as_mut()
                                .unwrap()
                                .1
                                .push(span.id);
                        }
                    }
                    SpanSet::SharedLocalSpans(spans) => {
                        for span in spans.spans.iter() {
                            let parent_id = span.parent_id.unwrap_or(item.parent_id);
                            collect
                                .get_mut(&item.collect_id)
                                .as_mut()
                                .unwrap()
                                .get_mut(&Some(parent_id))
                                .as_mut()
                                .unwrap()
                                .1
                                .push(span.id);
                        }
                    }
                }
            }
        }

        let mut res = collect
            .into_iter()
            .map(|(id, mut children)| {
                let mut tree = Self::build_tree(Some(SpanId(0)), &mut children);
                tree.sort();
                assert_eq!(tree.children.len(), 1);
                (id, tree.children.pop().unwrap())
            })
            .collect::<Vec<(usize, Tree)>>();
        res.sort_unstable();
        res
    }

    pub fn from_span_records(span_records: Vec<SpanRecord>) -> Tree {
        let mut children: TreeChildren = HashMap::new();

        children.insert(Some(SpanId(0)), ("".into(), vec![], vec![], vec![]));
        for span in &span_records {
            children.insert(
                Some(span.span_id),
                (
                    span.name.to_string(),
                    vec![],
                    span.properties
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                    span.events
                        .iter()
                        .map(|e| {
                            (
                                e.name.to_string(),
                                e.properties
                                    .iter()
                                    .map(|(k, v)| (k.to_string(), v.to_string()))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
            );
        }
        for span in &span_records {
            children
                .get_mut(&Some(span.parent_id))
                .as_mut()
                .unwrap()
                .1
                .push(span.span_id);
        }

        let mut t = Self::build_tree(Some(SpanId(0)), &mut children);
        t.sort();
        assert_eq!(t.children.len(), 1);
        t.children.remove(0)
    }

    fn build_tree(id: Option<SpanId>, raw: &mut TreeChildren) -> Tree {
        let (name, children, properties, events) = raw.get(&id).cloned().unwrap();
        Tree {
            name,
            children: children
                .into_iter()
                .map(|id| Self::build_tree(Some(id), raw))
                .collect(),
            properties,
            events,
        }
    }
}

pub fn tree_str_from_raw_spans(raw_spans: RawSpans) -> String {
    Tree::from_raw_spans(raw_spans)
        .iter()
        .map(|t| format!("\n{t}"))
        .collect::<Vec<_>>()
        .join("")
}

pub fn tree_str_from_span_sets(span_sets: &[(SpanSet, CollectToken)]) -> String {
    Tree::from_span_sets(span_sets)
        .iter()
        .map(|(id, t)| format!("\n#{id}\n{t}"))
        .collect::<Vec<_>>()
        .join("")
}

pub fn tree_str_from_span_records(span_records: Vec<SpanRecord>) -> String {
    format!("\n{}", Tree::from_span_records(span_records))
}
