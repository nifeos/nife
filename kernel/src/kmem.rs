//! **The kernel's own object budget**: one untyped region, carved once at first use, that every
//! kernel-owned object draws from (kernel endpoints, kernel thread stacks, kernel TCB pages).
//!
//! After the carve the kernel cannot spend beyond it, so the frame allocator's count does not move
//! when kernel threads come and go. Dead object pages go on a fixed free-stack and are reused page
//! for page: a budget with reuse, not an allocator. See notes/kernel-budget.md.

use page_frames::FRAME_SIZE;

use crate::sync::{IrqSafeMutex, rank};

/// The carve: 2048 pages (8 MiB). A thread costs 7 pages here (six of stack, see thread.rs
/// `STACK_PAGES`, and one of TCB), so `sched::MAX_THREADS` (256) takes 1792, endpoints about 60,
/// and 196 are slack. Exhaustion fails the spawn or create cleanly; raising this is the fix when the
/// image legitimately needs more. See notes/kernel-budget.md.
const KERNEL_OBJ_PAGES: u64 = 2048;

struct Pool {
    /// The region id, once carved. Lazy: the first object need triggers the carve, which is
    /// early (the idle thread's stack) and cannot fragment-fail at that point.
    region: Option<u64>,
    /// Dead object pages, ready for reuse. Bounded by the carve itself.
    free: [u64; KERNEL_OBJ_PAGES as usize],
    free_len: usize,
}

/// Rank note: taken alone or under `IPC_TABLES` (`create_rendezvous` holds `IPC_TABLES`, then this, then the
/// region lock: 60 -> 59 -> 58, a legal descent). Shares 59 with INBOX and MAPPINGS, with which
/// it is never nested: inboxes are scheduling traffic, the mapping registry is user-space
/// bookkeeping, and this is the kernel shopping for its own pages.
static POOL: IrqSafeMutex<Pool> = IrqSafeMutex::new(
    rank::KMEM,
    Pool {
        region: None,
        free: [0; KERNEL_OBJ_PAGES as usize],
        free_len: 0,
    },
);

/// One page of the kernel's budget, zeroed, recycled if a dead object's page is waiting, carved
/// from the region otherwise. `None` when the carve is exhausted (raise `KERNEL_OBJ_PAGES`).
pub fn page() -> Option<u64> {
    let phys = {
        let mut pool = POOL.lock();
        if pool.free_len > 0 {
            pool.free_len -= 1;
            let phys = pool.free[pool.free_len];
            drop(pool);
            // Recycled pages carry the dead object's bytes; zero for parity with retype, so a
            // recycled page is indistinguishable from a fresh one to whatever it becomes next.
            // SAFETY: the page is the pool's own, off every live structure, direct-mapped.
            unsafe {
                core::ptr::write_bytes(
                    crate::arch::mmu::phys_to_virt(phys) as *mut u8,
                    0,
                    FRAME_SIZE as usize,
                );
            }
            return Some(phys);
        }
        let region = match pool.region {
            Some(r) => r,
            None => {
                let Some(r) = crate::memory_region::create(KERNEL_OBJ_PAGES) else {
                    crate::println!("kmem: the {KERNEL_OBJ_PAGES}-page carve itself was refused");
                    return None;
                };
                pool.region = Some(r);
                r
            }
        };
        drop(pool);
        // Pin-and-carve, like every object page since 19a: this region hosts kernel objects for
        // the machine's lifetime, and nothing may ever destroy it.
        let Some(phys) = crate::memory_region::retype_object_page(
            region,
            crate::memory_region::ObjectKind::KernelPool,
        ) else {
            crate::println!(
                "kmem: carve exhausted ({KERNEL_OBJ_PAGES} pages spent, none recycled); raise \
                 KERNEL_OBJ_PAGES"
            );
            return None;
        };
        phys
    };
    Some(phys)
}

/// A dead object's page comes home. The caller guarantees nothing references it any more (the
/// object was dropped in place, its mappings unmapped); the next `page()` may hand it out.
pub(crate) fn recycle(phys: u64) {
    let mut pool = POOL.lock();
    debug_assert!(
        pool.free_len < pool.free.len(),
        "more recycled pages than the carve holds"
    );
    if pool.free_len < pool.free.len() {
        let n = pool.free_len;
        pool.free[n] = phys;
        pool.free_len += 1;
    } // else: leak rather than corrupt; unreachable per the bound above
}
