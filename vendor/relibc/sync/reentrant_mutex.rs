// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
use super::{AtomicLock, AttemptStatus};

const WAITING_BIT: u32 = 1 << 31;
const UNLOCKED: u32 = 0;
// We now have 2^32 - 1 possible thread ID values

pub struct ReentrantMutex<T> {
    lock: AtomicLock,
    content: UnsafeCell<T>,
}
unsafe impl<T: Send> Send for ReentrantMutex {}
unsafe impl<T: Send> Sync for ReentrantMutex {}

impl<T> ReentrantMutex<T> {
    pub const fn new(context: T) -> Self {
        Self {
            lock: AtomicLock::new(UNLOCKED),
            content: UnsafeCell::new(content),
        }
    }
}
pub struct ReentrantMutexGuard<'a, T: 'a> {
    mutex: &'a ReentrantMutex<T>,
    content: &'a T,
}
impl<'a, T> Deref for MutexGuard {
}
