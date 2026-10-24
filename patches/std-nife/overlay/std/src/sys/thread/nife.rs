//! **`std::thread` on nife** (milestone 812 (`std::thread::spawn` runs real threads in one address
//! space); §269 (how threads share a process), as calef ruled it on pull request #1892).
//!
//! A spawned thread is a TCB retyped from the heap's region, joined to this program's process by
//! `ThreadControlBlock::CONFIGURE` with the capability in `runtimeproto::PROCESS_SLOT` (which
//! carries `BIND`). It runs on a heap stack, shares the capability table, the space and every lock,
//! and has its own thread pointer naming its own block (`sys::thread_local::key::nife`). It ends
//! with `SYS_EXIT_THREAD`, which clears the word its joiner waits on only once it has left its
//! stack for good. A program given no process capability cannot spawn: `Thread::new` answers
//! `Unsupported`.

//! # BUGS
//!
//! - **A spawned thread's stack has no guard page.** It is heap memory, so an overflow writes into
//!   the heap rather than faulting. `notes/std/threads.md` has the plan.
//! - **A detached thread's stack is never freed**: nothing is left to free it once the thread has
//!   gone, so a program that spawns and drops threads without joining leaks one stack each.
//! - **Each spawn spends one TCB page of the heap's region for good** (`notes/processes.md`).
//! - **A process has at most `current_cpu_contract::THREAD_PAGE_SLOTS` (16) threads**, its main
//!   thread included; one more spawn answers `OutOfMemory` as an `io::Error`.

use crate::ffi::CStr;
use crate::io;
use crate::num::NonZero;
use crate::sync::atomic::{Atomic, AtomicU32, Ordering};
use crate::sys::pal::nife::{abi, rt, runtimeproto};
use crate::sys::thread_local::key::nife as tls;
use crate::thread::ThreadInit;
use crate::time::Duration;

/// The packet a spawner and its thread share: one word, the futex the joiner waits on.
///
/// `RUNNING` until the thread leaves. The thread's `SYS_EXIT_THREAD` names this word, and the
/// kernel stores `DONE` in it and wakes the joiner once the thread will never touch its stack
/// again, which is what makes freeing the stack after a join sound. A handle dropped without a
/// join marks it `DETACHED`, and the thread then frees the packet itself and names no word.
struct Packet {
    state: Atomic<u32>,
}

const DONE: u32 = 0;
const RUNNING: u32 = 1;
const DETACHED: u32 = 2;
const EXITING: u32 = 3;

pub struct Thread {
    packet: *mut Packet,
    stack: *mut u8,
    stack_size: usize,
    tcb: u64,
}

// SAFETY: a `Thread` is a handle to a thread of this process; nothing in it is tied to the thread
// that made it.
unsafe impl Send for Thread {}
// SAFETY: as above; `&Thread` offers nothing.
unsafe impl Sync for Thread {}

pub const DEFAULT_MIN_STACK_SIZE: usize = 64 * 1024;

const STACK_ALIGN: usize = 16;

fn stack_layout(size: usize) -> crate::alloc::Layout {
    // SAFETY-free: the size is rounded and nonzero, and 16 is a power of two.
    crate::alloc::Layout::from_size_align(size, STACK_ALIGN).expect("a thread stack's layout")
}

impl Thread {
    pub unsafe fn new(stack: usize, init: Box<ThreadInit>) -> io::Result<Thread> {
        use crate::alloc::{GlobalAlloc, System};

        // The calling thread's own block first: on the main thread this is the moment its thread
        // pointer is set, and it must be before a second thread exists (`tls::block`).
        if !tls::main_is_ready() {
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }
        let stack_size = stack.max(DEFAULT_MIN_STACK_SIZE).next_multiple_of(STACK_ALIGN);
        // SAFETY: a nonzero, aligned layout.
        let stack_base = unsafe { System.alloc(stack_layout(stack_size)) };
        if stack_base.is_null() {
            return Err(io::Error::new(io::ErrorKind::OutOfMemory, "no memory for a thread stack"));
        }
        let block = tls::new_block();
        if block.is_null() {
            // SAFETY: allocated just above with this layout.
            unsafe { System.dealloc(stack_base, stack_layout(stack_size)) };
            return Err(io::Error::new(io::ErrorKind::OutOfMemory, "no memory for a thread block"));
        }
        let packet = Box::into_raw(Box::new(Packet { state: AtomicU32::new(RUNNING) }));
        let data = Box::into_raw(init);
        let undo = |packet: *mut Packet, data: *mut ThreadInit| {
            // SAFETY: each was made just above and handed to nobody.
            unsafe {
                drop(Box::from_raw(packet));
                drop(Box::from_raw(data));
                tls::free_block(block);
                System.dealloc(stack_base, stack_layout(stack_size));
            }
        };

        // SAFETY: `invoke` traps to the kernel, which validates each capability and method.
        let tcb = unsafe {
            rt::invoke(
                runtimeproto::MEMORY_REGION_SLOT,
                abi::memory_region::RETYPE_OBJ,
                abi::objtype::THREAD_CONTROL_BLOCK,
                0,
                runtimeproto::SLOTS, // above every fixed slot (`RETYPE_OBJ`'s floor)
            )
        };
        if tcb < 0 {
            undo(packet, data);
            return Err(error(tcb, "no thread control block"));
        }
        let tcb = tcb as u64;
        let stack_top = stack_base as u64 + stack_size as u64;
        // CONFIGURE joins our own process and gives the thread its block as its thread pointer
        // (the sixth register); its answer is the thread's own current-CPU page.
        let page = rt::configure_joining(
            tcb,
            entry_address(),
            stack_top,
            runtimeproto::PROCESS_SLOT,
            block as u64,
        );
        if page < 0 {
            rt::cap_delete(tcb);
            undo(packet, data);
            return Err(error(page, "the thread could not join this process"));
        }
        // SAFETY: `block` is the new thread's, not yet running, so this write is the only access;
        // `START` below orders it before the thread's first instruction.
        unsafe { tls::set_current_cpu_page(block, page as u64) };
        // SAFETY: as `RETYPE_OBJ` above. The three words are the thread's first argument registers.
        let started = unsafe {
            rt::invoke(
                tcb,
                abi::thread_control_block::START,
                data as u64,
                packet as u64,
                0,
            )
        };
        if started < 0 {
            rt::cap_delete(tcb);
            undo(packet, data);
            return Err(error(started, "the thread could not start"));
        }
        Ok(Thread { packet, stack: stack_base, stack_size, tcb })
    }

    pub fn join(self) {
        let this = crate::mem::ManuallyDrop::new(self);
        // SAFETY: the packet lives until whichever of joiner and thread frees it, and the joiner
        // frees it only after the kernel has cleared the word on the thread's way out.
        let state = unsafe { &(*this.packet).state };
        while state.load(Ordering::Acquire) != DONE {
            let seen = state.load(Ordering::Acquire);
            if seen == DONE {
                break;
            }
            crate::sys::sync::futex::futex_wait(state, seen, None);
        }
        // SAFETY: the thread has left user mode for good (the word is cleared only then), so
        // nothing uses its stack or the packet any more.
        unsafe { this.release() };
    }

    /// Free what the joiner owns: the stack, the packet, the TCB capability.
    unsafe fn release(&self) {
        use crate::alloc::{GlobalAlloc, System};
        rt::cap_delete(self.tcb);
        // SAFETY: the caller's contract: the thread is gone from user mode.
        unsafe {
            drop(Box::from_raw(self.packet));
            System.dealloc(self.stack, stack_layout(self.stack_size));
        }
    }
}

impl Drop for Thread {
    /// **Detach.** If the thread is still running, it frees the packet itself on its way out, and
    /// its stack is left (`BUGS`). If it is already leaving, wait for it to be gone and free all.
    fn drop(&mut self) {
        // SAFETY: as `join`'s.
        let state = unsafe { &(*self.packet).state };
        match state.compare_exchange(RUNNING, DETACHED, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => rt::cap_delete(self.tcb),
            Err(_) => {
                while state.load(Ordering::Acquire) != DONE {
                    crate::sys::sync::futex::futex_wait(state, EXITING, None);
                }
                // SAFETY: the thread has left user mode for good.
                unsafe { self.release() };
            }
        }
    }
}

fn error(code: i64, what: &'static str) -> io::Error {
    if code == abi::Error::OutOfMemory as i64 {
        io::Error::new(io::ErrorKind::OutOfMemory, what)
    } else if code == abi::Error::NoSuchSlot as i64 || code == abi::Error::NotPermitted as i64 {
        io::Error::UNSUPPORTED_PLATFORM
    } else {
        io::Error::other(what)
    }
}

#[inline(never)]
fn run(init: Box<ThreadInit>) {
    let rust_start = init.init();
    rust_start();
}

/// The address a spawned thread starts at: [`thread_start`], through a `call` on `x86_64`. The kernel
/// enters user mode with the stack 16-aligned, and the C ABI expects the alignment a `call` leaves,
/// which is why the main thread's `_start` goes through one too (`sys::pal::nife`).
fn entry_address() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        unsafe extern "C" {
            fn nife_thread_entry();
        }
        nife_thread_entry as usize as u64
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        thread_start as usize as u64
    }
}

#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(
    ".pushsection .text.nife_thread_entry, \"ax\", @progbits",
    ".globl nife_thread_entry",
    "nife_thread_entry:",
    "call {entry}",
    "ud2",
    ".popsection",
    entry = sym thread_start,
);

/// **Where a spawned thread begins.** Its thread pointer already names its block (set at
/// `CONFIGURE`), so thread-locals work from the first instruction. Runs the closure, runs the
/// thread-locals' destructors, and ends the thread, and only the thread.
extern "C" fn thread_start(data: u64, packet: u64) -> ! {
    // SAFETY: `data` is the `Box<ThreadInit>` `Thread::new` leaked for exactly this thread.
    let init = unsafe { Box::from_raw(data as *mut ThreadInit) };
    run(init);
    // SAFETY: the closure has returned; nothing of this thread's will use a thread-local again.
    unsafe { tls::destroy_tls() };
    let packet = packet as *mut Packet;
    // SAFETY: the packet lives until the joiner frees it, which it does only after the word is
    // cleared, or until this thread frees it, when it has been detached.
    let state = unsafe { &(*packet).state };
    let word = match state.compare_exchange(RUNNING, EXITING, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => state.as_ptr() as u64,
        Err(_) => {
            // Detached: nobody will wait on the word, so free it here and name none.
            // SAFETY: the detacher has let go of it.
            unsafe { drop(Box::from_raw(packet)) };
            0
        }
    };
    // SAFETY: `destroy_tls` ran, and the packet above was this thread's last free.
    unsafe { tls::free_own_block() };
    rt::exit_thread(word)
}

/// **How many cores this process may use**: the allowance on this thread's current-CPU page
/// (§269 fork 7), the online count today. One where the page carries none.
pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    Ok(tls::allowance().unwrap_or(NonZero::<usize>::MIN))
}

pub fn current_os_id() -> Option<u64> {
    None
}

pub fn yield_now() {
    rt::yield_now();
}

pub fn set_name(_name: &CStr) {
    // nife threads have no names the kernel keeps.
}

/// **Sleep**, on a timer and a notification of this thread's own (`tls::sleep_objects`): two
/// threads sleeping at once must not share one notification, or one would take the other's wake.
/// Where they cannot be made, a yield loop to the deadline, as before.
pub fn sleep(dur: Duration) {
    let ticks = abi::timer::counter_ticks_for(dur.as_secs(), dur.subsec_nanos(), rt::cntfrq());
    let deadline = rt::now().saturating_add(ticks);
    if let Some((timer, notification)) = tls::sleep_objects() {
        // SAFETY: `invoke` traps to the kernel, which validates both capabilities.
        let armed = unsafe { rt::invoke(timer, abi::timer::ARM, deadline, notification, 1) };
        if armed == 0 {
            // SAFETY: as above.
            unsafe { rt::invoke(notification, abi::notification::WAIT, 0, 0, 0) };
        }
    }
    while rt::now() < deadline {
        rt::yield_now();
    }
}
