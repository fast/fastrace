// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! A module for relationship checking in test

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Display;
use std::fmt::Formatter;

use crate::collector::SpanId;
use crate::collector::SpanRecord;
use crate::collector::SpanSet;
use crate::util::CollectToken;
use crate::util::RawSpans;

type TreeChildren = HashMap<
    SpanId,
    (
        Cow<'static, str>,
        Vec<SpanId>,
        Vec<(Cow<'static, str>, Cow<'static, str>)>,
    ),
>;

#[derive(Debug, PartialOrd, PartialEq, Ord, Eq)]
pub struct Tree {
    name: Cow<'static, str>,
    children: Vec<Tree>,
    properties: Vec<(Cow<'static, str>, Cow<'static, str>)>,
}

impl Display for Tree {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.fmt_with_depth(f, 0)
    }
}

impl Tree {
    fn fmt_with_depth(&self, f: &mut Formatter<'_>, depth: usize) -> std::fmt::Result {
        writeln!(
            f,
            "{:indent$}{} {:?}",
            "",
            self.name,
            self.properties,
            indent = depth * 4
        )?;
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

        let spans = raw_spans.into_inner();
        children.insert(SpanId::default(), ("".into(), vec![], vec![]));
        for span in &spans {
            children.insert(
                span.id,
                (span.name.clone(), vec![], span.properties.clone()),
            );
        }
        for span in &spans {
            children
                .get_mut(&span.parent_id)
                .as_mut()
                .unwrap()
                .1
                .push(span.id);
        }

        let mut t = Self::build_tree(SpanId::default(), &mut children);
        t.sort();
        t.children
    }

    /// Return a vector of collect id -> Tree
    pub fn from_span_sets(span_sets: &[(SpanSet, CollectToken)]) -> Vec<(usize, Tree)> {
        let mut collect = HashMap::<
            usize,
            HashMap<
                SpanId,
                (
                    Cow<'static, str>,
                    Vec<SpanId>,
                    Vec<(Cow<'static, str>, Cow<'static, str>)>,
                ),
            >,
        >::new();

        for (span_set, token) in span_sets {
            for item in token.iter() {
                collect
                    .entry(item.collect_id)
                    .or_default()
                    .insert(SpanId::default(), ("".into(), vec![], vec![]));
                match span_set {
                    SpanSet::Span(span) => {
                        collect.entry(item.collect_id).or_default().insert(
                            span.id,
                            (span.name.clone(), vec![], span.properties.clone()),
                        );
                    }
                    SpanSet::LocalSpansInner(spans) => {
                        for span in spans.spans.iter() {
                            collect.entry(item.collect_id).or_default().insert(
                                span.id,
                                (span.name.clone(), vec![], span.properties.clone()),
                            );
                        }
                    }
                    SpanSet::SharedLocalSpans(spans) => {
                        for span in spans.spans.iter() {
                            collect.entry(item.collect_id).or_default().insert(
                                span.id,
                                (span.name.clone(), vec![], span.properties.clone()),
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
                        let parent_id = if span.parent_id == SpanId::default() {
                            item.parent_id
                        } else {
                            span.parent_id
                        };
                        collect
                            .get_mut(&item.collect_id)
                            .as_mut()
                            .unwrap()
                            .get_mut(&parent_id)
                            .as_mut()
                            .unwrap()
                            .1
                            .push(span.id);
                    }
                    SpanSet::LocalSpansInner(spans) => {
                        for span in spans.spans.iter() {
                            let parent_id = if span.parent_id == SpanId::default() {
                                item.parent_id
                            } else {
                                span.parent_id
                            };
                            collect
                                .get_mut(&item.collect_id)
                                .as_mut()
                                .unwrap()
                                .get_mut(&parent_id)
                                .as_mut()
                                .unwrap()
                                .1
                                .push(span.id);
                        }
                    }
                    SpanSet::SharedLocalSpans(spans) => {
                        for span in spans.spans.iter() {
                            let parent_id = if span.parent_id == SpanId::default() {
                                item.parent_id
                            } else {
                                span.parent_id
                            };
                            collect
                                .get_mut(&item.collect_id)
                                .as_mut()
                                .unwrap()
                                .get_mut(&parent_id)
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
                let mut tree = Self::build_tree(SpanId::default(), &mut children);
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

        children.insert(SpanId::default(), ("".into(), vec![], vec![]));
        for span in &span_records {
            children.insert(
                span.span_id,
                (span.name.clone(), vec![], span.properties.clone()),
            );
        }
        for span in &span_records {
            children
                .get_mut(&span.parent_id)
                .as_mut()
                .unwrap()
                .1
                .push(span.span_id);
        }

        let mut t = Self::build_tree(SpanId::default(), &mut children);
        t.sort();
        assert_eq!(t.children.len(), 1);
        t.children.remove(0)
    }

    fn build_tree(id: SpanId, raw: &mut TreeChildren) -> Tree {
        let (name, children, properties) = raw.get(&id).cloned().unwrap();
        Tree {
            name,
            children: children
                .into_iter()
                .map(|id| Self::build_tree(id, raw))
                .collect(),
            properties,
        }
    }
}

pub fn tree_str_from_raw_spans(raw_spans: RawSpans) -> String {
    Tree::from_raw_spans(raw_spans)
        .iter()
        .map(|t| format!("\n{}", t))
        .collect::<Vec<_>>()
        .join("")
}

pub fn tree_str_from_span_sets(span_sets: &[(SpanSet, CollectToken)]) -> String {
    Tree::from_span_sets(span_sets)
        .iter()
        .map(|(id, t)| format!("\n#{}\n{}", id, t))
        .collect::<Vec<_>>()
        .join("")
}

pub fn tree_str_from_span_records(span_records: Vec<SpanRecord>) -> String {
    format!("\n{}", Tree::from_span_records(span_records))
}
