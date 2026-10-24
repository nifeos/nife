//! **Thread-local keys on nife, through the thread pointer** (milestone 812 (`std::thread::spawn`
//! runs real threads in one address space); §269 (how threads share a process) fork 4).
//!
//! Each thread's thread pointer (`TPIDR_EL0`, `tp`, the `FS` base) names its own [`Block`], set by
//! the kernel at `CONFIGURE` for a spawned thread and through `SET_THREAD_POINTER` on its own TCB
//! for the main thread. A `thread_local!` is a key, an index into the block's slots (xous's shape).
//! The block also holds the thread's current-CPU page (§269 fork 6), its sleep objects, and its
//! allocation cache (milestone 561 (a per-CPU allocator is what the current-CPU page was for);
//! [`allocation_cache`]). A spawned thread's end spills the cache before anything else of the
//! thread's is freed ([`destroy_tls`]), and frees the block last ([`free_own_block`]).

//! # The main thread's block
//!
//! Made on first use, before any second thread can exist, because `Thread::new` makes it first;
//! until then a process-wide flag says "not yet" without a race. On `x86_64` ring 3 cannot read the
//! `FS` base (`CR4.FSGSBASE` is off), only address through it, so a block's first word points at
//! the block itself and is read as `fs:[0]`, which before the base is set would read address zero.

//! # BUGS
//!
//! - **[`KEYS`] keys a process**, and a key is never reused: `std` makes one per `thread_local!`
//!   that is ever touched. More aborts the program, as xous's does.

use crate::alloc::{GlobalAlloc, Layout, System};
use crate::mem::ManuallyDrop;
use crate::num::NonZero;
use crate::ptr;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicBool, AtomicPtr, AtomicUsize};
use crate::sys::alloc::nife::allocation_cache::Cache;
use crate::sys::pal::nife::{abi, currentcpuproto, rt, runtimeproto};

pub type Key = usize;
pub type Dtor = unsafe extern "C" fn(*mut u8);

/// How many keys a process may make.
const KEYS: usize = 250;

/// **One thread's block.** `#[repr(C)]` and `this` first, because `x86_64` finds the block as
/// `fs:[0]`.
#[repr(C)]
pub struct Block {
    this: *mut Block,
    /// This thread's current-CPU page, where its core and its allowance are published.
    cpu_page: u64,
    /// This thread's sleep timer and notification, `0` until first made, `u64::MAX` if they
    /// cannot be.
    sleep_timer: u64,
    sleep_notification: u64,
    /// This thread's allocation cache. Zeroed is empty, so a fresh block needs no setup.
    cache: Cache,
    /// Set at the thread's end, after which its allocations go to the heap itself.
    cache_closed: bool,
    slots: [*mut u8; KEYS],
}

// A block's own free must not land in the cache it holds (`free_own_block`).
const _: () = assert!(size_of::<Block>() > crate::sys::alloc::nife::allocation_cache::LARGEST);

fn layout() -> Layout {
    Layout::new::<Block>()
}

/// **A fresh block for a thread about to be spawned**, null on no memory.
pub fn new_block() -> *mut Block {
    // SAFETY: a nonzero layout.
    let p = unsafe { System.alloc_zeroed(layout()) }.cast::<Block>();
    if !p.is_null() {
        // SAFETY: just allocated, zeroed, ours.
        unsafe { (*p).this = p };
    }
    p
}

/// Free a block that never ran a thread, or the calling thread's own at its end.
///
/// # Safety
/// `block` came from [`new_block`] and nothing will read it again.
pub unsafe fn free_block(block: *mut Block) {
    // SAFETY: the caller's contract.
    unsafe { System.dealloc(block.cast(), layout()) };
}

/// Record a not-yet-started thread's current-CPU page in its block.
///
/// # Safety
/// `block` is a live block no running thread reads.
pub unsafe fn set_current_cpu_page(block: *mut Block, page: u64) {
    // SAFETY: the caller's contract.
    unsafe { (*block).cpu_page = page };
}

/// The main thread's block, once made; and whether it has been.
static MAIN: Atomic<*mut Block> = AtomicPtr::new(ptr::null_mut());
static MAIN_READY: Atomic<bool> = AtomicBool::new(false);
/// Whether the main thread's pointer could be set, which a spawn needs.
static MAIN_POINTER_SET: Atomic<bool> = AtomicBool::new(false);

/// The thread pointer's value, read the one way all three architectures allow.
#[inline]
fn thread_pointer() -> *mut Block {
    let p: usize;
    #[cfg(target_arch = "aarch64")]
    // SAFETY: an EL0 read of EL0's own register.
    unsafe {
        core::arch::asm!("mrs {}, tpidr_el0", out(reg) p, options(nomem, nostack, preserves_flags));
    }
    #[cfg(target_arch = "riscv64")]
    // SAFETY: a register move.
    unsafe {
        core::arch::asm!("mv {}, tp", out(reg) p, options(nomem, nostack, preserves_flags));
    }
    #[cfg(target_arch = "x86_64")]
    // SAFETY: called only once a block is installed (see `block`), whose first word is itself.
    unsafe {
        core::arch::asm!("mov {}, fs:[0]", out(reg) p, options(readonly, nostack, preserves_flags));
    }
    p as *mut Block
}

/// **The calling thread's block.**
#[inline]
fn block() -> *mut Block {
    if !MAIN_READY.load(Acquire) {
        return main_block();
    }
    let p = thread_pointer();
    if p.is_null() { main_block() } else { p }
}

/// **Make the main thread's block and point its thread pointer at it.** Runs while the main thread
/// is the only thread, so the flags need no lock. Without a TCB capability for itself the main
/// thread keeps a zero thread pointer and finds its block through [`MAIN`], and the process cannot
/// spawn ([`main_is_ready`]).
#[cold]
fn main_block() -> *mut Block {
    let existing = MAIN.load(Acquire);
    if !existing.is_null() {
        return existing;
    }
    let block = new_block();
    if block.is_null() {
        rtabort!("no memory for the main thread's thread-local block");
    }
    // SAFETY: just made, ours.
    unsafe { (*block).cpu_page = currentcpuproto::PAGE_VA };
    // SAFETY: `invoke` traps to the kernel, which validates the capability.
    let set = unsafe {
        rt::invoke(
            runtimeproto::THREAD_SLOT,
            abi::thread_control_block::SET_THREAD_POINTER,
            block as u64,
            0,
            0,
        )
    };
    MAIN.store(block, Release);
    MAIN_POINTER_SET.store(set == 0, Release);
    MAIN_READY.store(set == 0, Release);
    block
}

/// **May this process spawn?** Makes the main thread's block if it is not made, and answers whether
/// its thread pointer is set: a second thread needs every thread's pointer to name its own block.
pub fn main_is_ready() -> bool {
    let _ = block();
    MAIN_POINTER_SET.load(Acquire)
}

/// **The calling thread's allocation cache**, or null when it must use the heap's lock: before the
/// main thread's block exists (whose making allocates, so this must not make it), in a process
/// whose thread pointer could not be set, and after the thread's end has closed it.
///
/// Only the calling thread touches what this answers, on whichever core it runs: that is the whole
/// correctness argument, and why it needs no atomic (`notes/std/allocation.md`).
#[inline]
pub fn allocation_cache() -> *mut Cache {
    // Relaxed: the main thread sets the flag before any other thread exists, and a spawn, which
    // follows `main_is_ready`'s acquire, is what makes another thread exist.
    if !MAIN_READY.load(Relaxed) {
        return ptr::null_mut();
    }
    let b = thread_pointer();
    // SAFETY: with the flag set, every thread's pointer names its own live block.
    unsafe { if (*b).cache_closed { ptr::null_mut() } else { &raw mut (*b).cache } }
}

/// This thread's allowance: how many cores its process may use (§269 fork 7).
pub fn allowance() -> Option<NonZero<usize>> {
    // SAFETY: a block's page is the one the kernel maps read-only for this thread.
    let page = unsafe { currentcpuproto::CurrentCpuPage::new((*block()).cpu_page) };
    page.allowance()
}

/// This thread's sleep objects, made from the heap's region on first use.
pub fn sleep_objects() -> Option<(u64, u64)> {
    let b = block();
    // SAFETY: the calling thread's own block; only it touches these fields.
    let (timer, notification) = unsafe { ((*b).sleep_timer, (*b).sleep_notification) };
    match timer {
        u64::MAX => return None,
        0 => {}
        t => return Some((t, notification)),
    }
    let retype = |objtype| {
        // SAFETY: `invoke` traps to the kernel, which validates the capability.
        unsafe {
            rt::invoke(
                runtimeproto::MEMORY_REGION_SLOT,
                abi::memory_region::RETYPE_OBJ,
                objtype,
                0,
                runtimeproto::SLOTS, // above every fixed slot (`RETYPE_OBJ`'s floor)
            )
        }
    };
    let n = retype(abi::objtype::NOTIFICATION);
    let t = if n >= 0 { retype(abi::objtype::TIMER) } else { n };
    // SAFETY: as above.
    unsafe {
        if n < 0 || t < 0 {
            (*b).sleep_timer = u64::MAX;
            return None;
        }
        (*b).sleep_timer = t as u64;
        (*b).sleep_notification = n as u64;
    }
    Some((t as u64, n as u64))
}

static NEXT_KEY: Atomic<usize> = AtomicUsize::new(1);
static DTORS: Atomic<*mut Node> = AtomicPtr::new(ptr::null_mut());

struct Node {
    dtor: Dtor,
    key: Key,
    next: *mut Node,
}

#[inline]
pub fn create(dtor: Option<Dtor>) -> Key {
    let key = NEXT_KEY.fetch_add(1, Relaxed);
    if key >= KEYS {
        rtabort!("out of thread-local keys");
    }
    if let Some(f) = dtor {
        let mut node = ManuallyDrop::new(Box::new_in(Node { key, dtor: f, next: ptr::null_mut() }, System));
        let mut head = DTORS.load(Acquire);
        loop {
            node.next = head;
            match DTORS.compare_exchange(head, &mut **node, Release, Acquire) {
                Ok(_) => break,
                Err(cur) => head = cur,
            }
        }
    }
    key
}

#[inline]
pub unsafe fn set(key: Key, value: *mut u8) {
    rtassert!(key >= 1 && key < KEYS);
    // SAFETY: the calling thread's own block, and the key is in range.
    unsafe { *(*block()).slots.get_unchecked_mut(key) = value };
}

#[inline]
pub unsafe fn get(key: Key) -> *mut u8 {
    rtassert!(key >= 1 && key < KEYS);
    // SAFETY: as `set`.
    unsafe { *(*block()).slots.get_unchecked(key) }
}

#[inline]
pub unsafe fn destroy(_key: Key) {
    // Keys are never reused, so there is nothing to give back (`BUGS`).
}

/// **A spawned thread's end**: run every thread-local's destructor, as many rounds as it takes for
/// destructors that set other thread-locals, then std's own cleanup, then close the thread's
/// allocation cache and give its blocks back to the heap. Without that spill each ended thread
/// would keep up to `MOST_CACHED_BYTES` out of the heap for good. The block stays, for
/// [`free_own_block`].
pub unsafe fn destroy_tls() {
    for _ in 0..5 {
        let mut any_run = false;
        let mut cur = DTORS.load(Acquire);
        while !cur.is_null() {
            // SAFETY: nodes are leaked and never freed; keys are in range.
            unsafe {
                let ptr = get((*cur).key);
                if !ptr.is_null() {
                    set((*cur).key, ptr::null_mut());
                    ((*cur).dtor)(ptr);
                    any_run = true;
                }
                cur = (*cur).next;
            }
        }
        if !any_run {
            break;
        }
    }
    crate::rt::thread_cleanup();
    let b = block();
    // SAFETY: this thread's own block. Closed first, so the spill's own lock and anything freed
    // after it go to the heap rather than refilling the cache.
    unsafe {
        (*b).cache_closed = true;
        crate::sys::alloc::nife::spill_thread_cache(&mut (*b).cache);
    }
}

/// **Free the calling spawned thread's block**, the last thing it does before `SYS_EXIT_THREAD`.
/// The thread pointer is left naming freed memory, which nothing reads: no allocation or free
/// follows (the block is too large for any cache class, so its own free goes to the heap).
///
/// # Safety
/// Called once, after [`destroy_tls`], by a spawned thread (never the main one) that will neither
/// allocate nor free again.
pub unsafe fn free_own_block() {
    // SAFETY: the caller's contract.
    unsafe { free_block(block()) };
}
