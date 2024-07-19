// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::SpanSet;
use crate::util::CollectToken;

#[derive(Debug)]
pub enum CollectCommand {
    StartCollect(StartCollect),
    DropCollect(DropCollect),
    CommitCollect(CommitCollect),
    SubmitSpans(SubmitSpans),
}

#[derive(Debug)]
pub struct StartCollect {
    pub collect_id: usize,
}

#[derive(Debug)]
pub struct DropCollect {
    pub collect_id: usize,
}

#[derive(Debug)]
pub struct CommitCollect {
    pub collect_id: usize,
}

#[derive(Debug)]
pub struct SubmitSpans {
    pub spans: SpanSet,
    pub collect_token: CollectToken,
}
