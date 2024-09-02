// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use rtrb::Consumer;
use rtrb::Producer;
use rtrb::PushError;
use rtrb::RingBuffer;

pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = RingBuffer::new(capacity);
    (
        Sender {
            tx,
            pending_messages: Vec::new(),
        },
        Receiver { rx },
    )
}

pub struct Sender<T> {
    tx: Producer<T>,
    pending_messages: Vec<T>,
}

pub struct Receiver<T> {
    rx: Consumer<T>,
}

#[derive(Debug)]
pub struct ChannelFull;

#[derive(Debug)]
pub struct ChannelClosed;

impl<T> Sender<T> {
    pub fn send(&mut self, value: T) -> Result<(), ChannelFull> {
        while let Some(value) = self.pending_messages.pop() {
            if let Err(PushError::Full(value)) = self.tx.push(value) {
                self.pending_messages.push(value);
                return Err(ChannelFull);
            }
        }

        self.tx.push(value).map_err(|_| ChannelFull)
    }

    pub fn force_send(&mut self, value: T) {
        while let Some(value) = self.pending_messages.pop() {
            if let Err(PushError::Full(value)) = self.tx.push(value) {
                self.pending_messages.push(value);
                break;
            }
        }

        if let Err(PushError::Full(value)) = self.tx.push(value) {
            self.pending_messages.push(value);
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
