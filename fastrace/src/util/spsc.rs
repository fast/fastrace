// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::VecDeque;

use rtrb::Consumer;
use rtrb::Producer;
use rtrb::PushError;
use rtrb::RingBuffer;

pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = RingBuffer::new(capacity);
    (
        Sender {
            tx,
            pending_messages: VecDeque::new(),
        },
        Receiver { rx },
    )
}

pub struct Sender<T> {
    tx: Producer<T>,
    pending_messages: VecDeque<T>,
}

pub struct Receiver<T> {
    rx: Consumer<T>,
}

#[derive(Debug)]
pub struct ChannelClosed;

impl<T> Sender<T> {
    #[inline]
    pub fn is_under_pressure(&self) -> bool {
        let capacity = self.tx.buffer().capacity();
        self.tx.slots() * 2 < capacity
    }

    pub fn send(&mut self, value: T) {
        while let Some(value) = self.pending_messages.pop_front() {
            if let Err(PushError::Full(value)) = self.tx.push(value) {
                self.pending_messages.push_front(value);
                break;
            }
        }

        if let Err(PushError::Full(value)) = self.tx.push(value) {
            self.pending_messages.push_back(value);
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        for command in self.pending_messages.drain(..) {
            drop(self.tx.push(command));
        }
    }
}

impl<T> Receiver<T> {
    pub fn try_recv(&mut self) -> Result<Option<T>, ChannelClosed> {
        match self.rx.pop() {
            Ok(val) => Ok(Some(val)),
            Err(_) if self.rx.is_abandoned() => Err(ChannelClosed),
            Err(_) => Ok(None),
        }
    }
}
