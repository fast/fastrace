// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use std::fmt;
use std::mem::ManuallyDrop;
use std::mem::{self};
use std::ops;
use std::slice;
use std::sync::Mutex;

#[must_use]
pub struct GlobalVecPool<T>
where T: 'static
{
    storage: Mutex<Vec<Vec<T>>>,
}

impl<T> Default for GlobalVecPool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> GlobalVecPool<T> {
    pub const fn new() -> Self {
        Self {
            storage: Mutex::new(Vec::new()),
        }
    }

    pub const fn new_local(&'static self, capacity: usize) -> LocalVecPool<T> {
        debug_assert!(capacity > 0, "storage capacity cannot be zero");

        LocalVecPool {
            global_pool: self,
            storage: Vec::new(),
            // lazy allocation - only if thread uses pool
            capacity,
        }
    }

    fn fill_empty_local(&'static self, local_storage: &mut Vec<Vec<T>>) {
        debug_assert!(local_storage.is_empty(), "local storage must be empty");
        debug_assert!(
            local_storage.capacity() != 0,
            "local storage must have capacity"
        );

        let needs = local_storage.capacity();

        let mut storage = self.storage.lock().expect("not poisoned");

        let available = storage.len();

        // try to take `1..needs` from storage. All with non-zero capacity.
        if available != 0 {
            let range = available.saturating_sub(needs)..;

            let vecs = storage.drain(range);

            local_storage.extend(vecs);

            return;
        }

        drop(storage);

        // init with zero-capacity vectors
        local_storage.resize_with(needs, || Vec::new());
    }

    fn consume_local(&'static self, local_storage: &mut Vec<Vec<T>>) {
        let Some(first) = local_storage.first() else {
            return;
        };

        // local storage contains vectors from global pool with non-zero
        // capacity or zero-capacity vectors (see fill_empty_local).

        if first.capacity() == 0 {
            // all elements with zero capacity
            return;
        }

        self.storage
            .lock()
            .expect("not poisoned")
            .extend(local_storage.drain(..));
    }

    fn recycle(&'static self, mut data: Vec<T>) {
        debug_assert!(data.capacity() != 0, "vec must have capacity");

        data.clear();

        self.storage.lock().expect("not poisoned").push(data);
    }

    pub fn take(&'static self) -> ReusableVec<T> {
        let mut storage = self.storage.lock().expect("not poisoned");

        if let Some(data) = storage.pop() {
            return ReusableVec::new(self, data);
        }

        drop(storage);

        ReusableVec::new(self, Vec::new())
    }

    /// Create a new `ReusableVec` as a stub.
    ///
    /// Capacity must be 0 when object is ready to be dropped, otherwise it
    /// will be recycled what may cause memory leaks.
    pub const fn stub(&'static self) -> ReusableVec<T> {
        ReusableVec::stub(self)
    }
}

#[must_use]
pub struct LocalVecPool<T: 'static> {
    global_pool: &'static GlobalVecPool<T>,
    storage: Vec<Vec<T>>,
    capacity: usize,
}

impl<T> LocalVecPool<T> {
    pub fn take(&mut self) -> ReusableVec<T> {
        if self.storage.is_empty() {
            if self.storage.capacity() == 0 {
                self.storage.reserve_exact(self.capacity);
            }

            self.global_pool.fill_empty_local(&mut self.storage);
        }

        ReusableVec::new(self.global_pool, self.storage.pop().expect("not empty"))
    }
}

impl<T> Drop for LocalVecPool<T> {
    fn drop(&mut self) {
        self.global_pool.consume_local(&mut self.storage);
    }
}

#[must_use]
pub struct ReusableVec<T: 'static> {
    global_pool: &'static GlobalVecPool<T>,
    data: ManuallyDrop<Vec<T>>,
    #[cfg(debug_assertions)]
    is_stub: bool,
}

impl<T> PartialEq for ReusableVec<T>
where T: PartialEq
{
    fn eq(&self, other: &Self) -> bool {
        self.data.eq(&other.data)
    }
}

impl<T> fmt::Debug for ReusableVec<T>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.fmt(f)
    }
}

impl<T> ReusableVec<T> {
    pub fn new(global_pool: &'static GlobalVecPool<T>, data: Vec<T>) -> Self {
        debug_assert!(data.is_empty(), "vec must be empty");

        Self {
            global_pool,
            data: ManuallyDrop::new(data),
            #[cfg(debug_assertions)]
            is_stub: false,
        }
    }

    const fn stub(global_pool: &'static GlobalVecPool<T>) -> Self {
        Self {
            global_pool,
            data: ManuallyDrop::new(Vec::new()),
            #[cfg(debug_assertions)]
            is_stub: true,
        }
    }

    #[inline]
    pub fn into_inner(mut self) -> Vec<T> {
        unsafe {
            let obj = ManuallyDrop::take(&mut self.data);

            mem::forget(self);

            obj
        }
    }
}

impl<T> Drop for ReusableVec<T> {
    fn drop(&mut self) {
        // SAFETY: first call
        let data = unsafe { ManuallyDrop::take(&mut self.data) };

        if data.capacity() != 0 {
            #[cfg(debug_assertions)]
            debug_assert!(!self.is_stub, "stubs cannot recycles");

            self.global_pool.recycle(data);
        }
    }
}

impl<'a, T> IntoIterator for &'a ReusableVec<T> {
    type IntoIter = slice::Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> ops::Deref for ReusableVec<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> ops::DerefMut for ReusableVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
