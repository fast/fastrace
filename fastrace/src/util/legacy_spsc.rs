// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::Arc;

use parking_lot::Mutex;

pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let page = Arc::new(Mutex::new(Vec::with_capacity(capacity)));
    (
        Sender {
            page: page.clone(),
            capacity,
        },
        Receiver {
            page,
            received: Vec::with_capacity(capacity),
        },
    )
}

pub struct Sender<T> {
    page: Arc<Mutex<Vec<T>>>,
    capacity: usize,
}

pub struct Receiver<T> {
    page: Arc<Mutex<Vec<T>>>,
    received: Vec<T>,
}

#[derive(Debug)]
pub struct ChannelFull;

#[derive(Debug)]
pub struct ChannelClosed;

impl<T> Sender<T> {
    pub fn send(&self, value: T) -> Result<(), ChannelFull> {
        let mut page = self.page.lock();
        if page.len() < self.capacity {
            page.push(value);
            Ok(())
        } else {
            Err(ChannelFull)
        }
    }

    pub fn force_send(&self, value: T) {
        let mut page = self.page.lock();
        page.push(value);
    }
}

impl<T> Receiver<T> {
    pub fn try_recv(&mut self) -> Result<Option<T>, ChannelClosed> {
        match self.received.pop() {
            Some(val) => Ok(Some(val)),
            None => {
                let mut page = self.page.lock();
                std::mem::swap(&mut *page, &mut self.received);
                match self.received.pop() {
                    Some(val) => Ok(Some(val)),
                    None => {
                        let is_disconnected = Arc::strong_count(&self.page) < 2;
                        if is_disconnected {
                            Err(ChannelClosed)
                        } else {
                            Ok(None)
                        }
                    }
                }
            }
        }
    }
}
