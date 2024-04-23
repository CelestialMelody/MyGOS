use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub struct SpinLock<T: ?Sized> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T> Sync for SpinLock<T> {}
unsafe impl<T> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> SpinLock<T> {
        SpinLock {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> SpinLock<T> {
    pub fn lock(&self) -> SpinLockGuard<T> {
        let mut try_count = 0usize;
        const MAX_TRIES: usize = 0x10000000;
        while self
            .lock
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
            try_count += 1;
            if try_count >= MAX_TRIES {
                panic!("Mutex: deadlock detected! try_count > {:#x}\n", try_count);
            }
        }
        SpinLockGuard { _lock: self }
    }

    fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }
}

pub struct SpinLockGuard<'a, T: ?Sized> {
    _lock: &'a SpinLock<T>,
}

// impl<'a, T: ?Sized + 'a> !Send for SpinLockGuard<'a, T> {}
// impl<'a, T: ?Sized + 'a> !Sync for SpinLockGuard<'a, T> {}

impl<'a, T: ?Sized> Drop for SpinLockGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self._lock.unlock();
    }
}

impl<'a, T: ?Sized> Deref for SpinLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self._lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self._lock.data.get() }
    }
}
