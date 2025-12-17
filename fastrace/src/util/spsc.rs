// Copyright 2024 FastLabs Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// This file is derived from [1] under the original license header:
// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.
// [1]: https://github.com/tikv/minitrace-rust/blob/v0.6.4/minitrace/src/util/spsc.rs

use std::collections::VecDeque;

use rtrb::Consumer;
use rtrb::Producer;
use rtrb::PushError;
use rtrb::RingBuffer;

pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = RingBuffer::new(capacity);
    let pending_msgs = VecDeque::new();
    (Sender { tx, pending_msgs }, Receiver { rx })
}

pub struct Sender<T> {
    tx: Producer<T>,
    pending_msgs: VecDeque<T>,
}

pub struct Receiver<T> {
    rx: Consumer<T>,
}

#[derive(Debug)]
pub struct ChannelClosed;

impl<T> Sender<T> {
    #[inline]
    pub(crate) fn is_under_pressure(&self) -> bool {
        let capacity = self.tx.buffer().capacity();
        self.tx.slots() * 2 < capacity
    }

    pub fn send(&mut self, value: T) {
        while let Some(pending_value) = self.pending_msgs.pop_front() {
            if let Err(PushError::Full(pending_value)) = self.tx.push(pending_value) {
                self.pending_msgs.push_front(pending_value);
                self.pending_msgs.push_back(value);
                return;
            }
        }

        if let Err(PushError::Full(value)) = self.tx.push(value) {
            self.pending_msgs.push_back(value);
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        for msg in std::mem::take(&mut self.pending_msgs) {
            self.tx.push(msg).ok();
        }
    }
}

impl<T> Receiver<T> {
    pub fn try_recv(&mut self) -> Result<Option<T>, ChannelClosed> {
        if let Ok(val) = self.rx.pop() {
            Ok(Some(val))
        } else if self.rx.is_abandoned() {
            Err(ChannelClosed)
        } else {
            Ok(None)
        }
    }
}
