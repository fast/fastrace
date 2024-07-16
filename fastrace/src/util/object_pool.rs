use std::cell::Cell;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ops::DerefMut;

use parking_lot::Mutex;

thread_local! {
    static REUSABLE: Cell<bool> = const { Cell::new(false) };
}

pub fn enable_reuse_in_current_thread() {
    REUSABLE.with(|r| r.set(true));
}

fn is_reusable() -> bool {
    REUSABLE.with(|r| r.get())
}

pub struct Pool<T> {
    // The objects in the pool ready to be reused.
    // The mutex should only be visited in the global collector, which is guaranteed by
    // `is_reusable`, so it should not have synchronization overhead.
    objects: Mutex<Vec<T>>,
    init: fn() -> T,
    reset: fn(&mut T),
}

impl<T> Pool<T> {
    #[inline]
    pub fn new(init: fn() -> T, reset: fn(&mut T)) -> Pool<T> {
        Pool {
            objects: Mutex::new(Vec::new()),
            init,
            reset,
        }
    }

    #[inline]
    fn batch_pull<'a>(&'a self, n: usize, buffer: &mut Vec<Reusable<'a, T>>) {
        let mut objects = self.objects.lock();
        let len = objects.len();
        buffer.extend(
            objects
                .drain(len.saturating_sub(n)..)
                .map(|obj| Reusable::new(self, obj)),
        );
        drop(objects);
        buffer.resize_with(n, || Reusable::new(self, (self.init)()));
    }

    pub fn puller(&self, buffer_size: usize) -> Puller<T> {
        assert!(buffer_size > 0);
        Puller {
            pool: self,
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
        }
    }

    #[inline]
    pub fn recycle(&self, mut obj: T) {
        if is_reusable() {
            (self.reset)(&mut obj);
            self.objects.lock().push(obj)
        }
    }
}

pub struct Puller<'a, T> {
    pool: &'a Pool<T>,
    buffer: Vec<Reusable<'a, T>>,
    buffer_size: usize,
}

impl<'a, T> Puller<'a, T> {
    #[inline]
    pub fn pull(&mut self) -> Reusable<'a, T> {
        self.buffer.pop().unwrap_or_else(|| {
            self.pool.batch_pull(self.buffer_size, &mut self.buffer);
            self.buffer.pop().unwrap()
        })
    }
}

pub struct Reusable<'a, T> {
    pool: &'a Pool<T>,
    obj: ManuallyDrop<T>,
}

impl<'a, T> Reusable<'a, T> {
    #[inline]
    pub fn new(pool: &'a Pool<T>, obj: T) -> Self {
        Self {
            pool,
            obj: ManuallyDrop::new(obj),
        }
    }

    #[inline]
    pub fn into_inner(mut self) -> T {
        unsafe {
            let obj = ManuallyDrop::take(&mut self.obj);
            std::mem::forget(self);
            obj
        }
    }
}

impl<'a, T> std::fmt::Debug for Reusable<'a, T>
where T: std::fmt::Debug
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.obj.fmt(f)
    }
}

impl<'a, T> std::cmp::PartialEq for Reusable<'a, T>
where T: std::cmp::PartialEq
{
    fn eq(&self, other: &Self) -> bool {
        T::eq(self, other)
    }
}

impl<'a, T> std::cmp::Eq for Reusable<'a, T> where T: std::cmp::Eq {}

impl<'a, T> Deref for Reusable<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl<'a, T> DerefMut for Reusable<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.obj
    }
}

impl<'a, T> Drop for Reusable<'a, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.pool.recycle(ManuallyDrop::take(&mut self.obj));
        }
    }
}
