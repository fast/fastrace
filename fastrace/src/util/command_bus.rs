// Copyright 2025 FastLabs Developers
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

use std::time::Duration;

use parking_lot::Mutex;

use crate::util::spsc;

pub(crate) struct CommandBus<T> {
    rxs: Mutex<Vec<spsc::Receiver<T>>>,
    notify: Notify,
}

impl<T> CommandBus<T> {
    pub fn new() -> Self {
        Self {
            rxs: Mutex::new(Vec::new()),
            notify: Notify::new(),
        }
    }

    pub fn sender(&self, capacity: usize) -> CommandSender<T> {
        let (tx, rx) = spsc::bounded(capacity);
        self.rxs.lock().push(rx);
        CommandSender {
            tx,
            notify: self.notify.sender(),
        }
    }

    pub fn drain(&self, mut f: impl FnMut(T)) {
        self.rxs.lock().retain_mut(|rx| {
            loop {
                match rx.try_recv() {
                    Ok(Some(msg)) => f(msg),
                    Ok(None) => return true,
                    Err(_) => return false,
                }
            }
        });
    }

    pub fn wait_timeout(&self, timeout: Duration) {
        self.notify.wait_timeout(timeout);
    }
}

pub(crate) struct CommandSender<T> {
    tx: spsc::Sender<T>,
    notify: NotifySender,
}

impl<T> CommandSender<T> {
    pub fn send(&mut self, value: T) {
        self.tx.send(value);
        #[cfg(not(target_family = "wasm"))]
        if self.tx.is_under_pressure() {
            self.notify.notify();
        }
    }
}

struct Notify {
    #[cfg(not(target_family = "wasm"))]
    notify_tx: std::sync::mpsc::SyncSender<()>,
    #[cfg(not(target_family = "wasm"))]
    notify_rx: Mutex<std::sync::mpsc::Receiver<()>>,
}

impl Notify {
    fn new() -> Self {
        #[cfg(not(target_family = "wasm"))]
        {
            let (notify_tx, notify_rx) = std::sync::mpsc::sync_channel(1);
            Self {
                notify_tx,
                notify_rx: Mutex::new(notify_rx),
            }
        }
        #[cfg(target_family = "wasm")]
        {
            Self {}
        }
    }

    fn sender(&self) -> NotifySender {
        #[cfg(not(target_family = "wasm"))]
        {
            NotifySender {
                notify_tx: self.notify_tx.clone(),
            }
        }
        #[cfg(target_family = "wasm")]
        {
            NotifySender {}
        }
    }

    fn wait_timeout(&self, timeout: Duration) {
        #[cfg(not(target_family = "wasm"))]
        self.notify_rx.lock().recv_timeout(timeout).ok();
    }
}

struct NotifySender {
    #[cfg(not(target_family = "wasm"))]
    notify_tx: std::sync::mpsc::SyncSender<()>,
}

impl NotifySender {
    fn notify(&self) {
        #[cfg(not(target_family = "wasm"))]
        self.notify_tx.try_send(()).ok();
    }
}
