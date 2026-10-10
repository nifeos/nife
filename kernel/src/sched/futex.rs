//! **Futex wait and wake, the scheduler half** (milestone 812 (`std::thread::spawn` runs real
//! threads in one address space), §269 (how threads share a process) fork 2). Split out of
//! `sched.rs` along this seam by §266 (a Rust source file stays under 2,000 lines); the queues are
//! `inter_process_communication::futex`, the wire contract `abi::address_space::WAIT`, and the
//! argument `notes/futex.md`.

use core::sync::atomic::Ordering;

use super::{
    IPC_TABLES, Thread, Wait, current_thread_id, hold_token, schedule, take_token, trace, wake,
};

/// **The key a parked futex waiter is under**, read from its own handshake: what the futex table
/// asks of each waiter it passes while looking for a key.
///
/// # Safety
/// `waiter` is a thread on the futex table, so a live TCB page, and `IPC_TABLES` is held. Only the
/// handshake's wait record is read, through the raw pointer, never a reference to the `Thread`.
unsafe fn futex_key_of(
    waiter: core::ptr::NonNull<Thread>,
) -> inter_process_communication::futex::Key {
    // SAFETY: the function's contract; `wait_on` is a `Copy` field read in place.
    match unsafe { (*waiter.as_ptr()).handshake.wait_on } {
        Some(Wait::Futex(key)) => key,
        // A waiter on this table that is not parked on a futex is a bookkeeping defect; a key no
        // wake can name keeps it from being woken as somebody else's.
        _ => inter_process_communication::futex::Key {
            space: u64::MAX,
            address: u64::MAX,
        },
    }
}

/// **Is `space` the calling thread's own address space?** The futex methods' authority check: the
/// key is a word in memory, and a thread may only wait on or wake words of its own space (§269 fork
/// 2, "a virtual address in the caller's own space").
pub fn is_current_space(space: u64) -> bool {
    let guard = IPC_TABLES.lock();
    guard
        .as_ref()
        .and_then(|sched| sched.threads.get(current_thread_id()))
        .and_then(|t| t.space.as_ref())
        .is_some_and(|s| s.name() == space)
}

/// **`AddressSpace::WAIT`, the kernel half** (milestone 812 (`std::thread::spawn` runs real threads
/// in one address space), §269 (how threads share a process) fork 2): park the calling thread on
/// the 32-bit word at `va` in its own space `space`, unless the word no longer holds `expected`.
/// `Ok(0)` after a wake, `Ok(1)` if the word differed, `BadPointer` if `va` is not mapped readable
/// to user mode, `Gone` if the wait ended without a wake.
///
/// The syscall layer has already checked the flags, the alignment, and that `space` is the caller's
/// own, so the live user page tables are this space's.
///
/// **Why reading the word and parking are one hold of `IPC_TABLES`**, which is the whole of a
/// futex's correctness. A waker stores the new value, then calls `WAKE`, which takes this lock. If
/// the store comes before our hold, the lock's release-acquire pair orders it before our read and
/// we see the new value and do not sleep. If it comes after, we are already parked when `WAKE`
/// looks, and it finds us. There is no third order. The load is `Acquire` for the same reason the
/// lock is: what the waker wrote before its store is visible to us once we return.
pub fn futex_wait(space: u64, va: u64, expected: u32) -> Result<u64, abi::Error> {
    let key = inter_process_communication::futex::Key { space, address: va };
    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let Some((phys, flags)) = crate::arch::mmu::translate_user(va) else {
            return Err(abi::Error::BadPointer);
        };
        if !flags.is_user_accessible() {
            return Err(abi::Error::BadPointer);
        }
        // SAFETY: `phys` is the frame this space mapped at `va`, user-accessible, and every RAM
        // frame is in the direct map, so the load reads RAM; `va` is 4-aligned, so the word is
        // aligned and inside the page. An atomic load, because user threads write the word
        // concurrently. Whether the frame can be revoked between the walk and the load has not been
        // argued (the walk is not under the space's own lock), and it is recorded in
        // notes/futex.md's BUGS: the worst case is reading one stale word of a frame being freed,
        // which makes this compare wrong and nothing else, since nothing read is returned.
        let word = unsafe {
            (*(crate::arch::mmu::phys_to_virt(phys) as *const core::sync::atomic::AtomicU32))
                .load(Ordering::Acquire)
        };
        if word != expected {
            return Ok(1);
        }
        let current = current_thread_id();
        let token = take_token(sched, current);
        sched.futexes.park(key, token);
        let t = sched.threads.get_mut(current).expect("running thread");
        t.handshake.park(Wait::Futex(key)); // only a WAKE (or an abort) may wake us
        trace::record(trace::Event::BlockSelf, current, 12);
    }
    schedule(); // blocks; a WAKE serves us and makes us Ready
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let t = sched
        .threads
        .get_mut(current_thread_id())
        .expect("running thread");
    if t.handshake.take_aborted() {
        return Err(abi::Error::Gone);
    }
    Ok(0)
}

/// **`AddressSpace::WAKE`, the kernel half** (milestone 812, §269 fork 2): wake up to `count`
/// threads parked on the word at `va` in `space`, oldest first, and return how many. Reads no
/// memory, so an unmapped `va` simply has no waiters. The woken are placed on this core, as a
/// notification's are: the waker is usually about to block or yield itself.
pub fn futex_wake(space: u64, va: u64, count: u64) -> u64 {
    let key = inter_process_communication::futex::Key { space, address: va };
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let max = usize::try_from(count).unwrap_or(usize::MAX);
    // SAFETY: every waiter on the table is a live TCB, and `IPC_TABLES` is held.
    let mut woken = sched.futexes.take(key, max, |w| unsafe { futex_key_of(w) });
    let mut n = 0;
    while let Some(token) = woken.pop_front() {
        let tid = hold_token(token);
        let t = sched
            .threads
            .get_mut(tid)
            .expect("a futex waiter vanished under IPC_TABLES");
        t.mailbox = [0; 5];
        t.handshake.serve();
        trace::record(trace::Event::Served, tid, 12);
        wake(sched, tid);
        n += 1;
    }
    n
}

/// How many threads are linked on the futex table at all: for the tests, which must see a TCB
/// left linked after its thread was torn down, which no longer has a key to be counted under.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn futex_queued() -> usize {
    let guard = IPC_TABLES.lock();
    guard.as_ref().map_or(0, |sched| sched.futexes.queued())
}

/// How many threads are parked on the futex at `va` in `space`: for the tests, which need to know
/// a thread is asleep on exactly this word before they wake it.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn futex_waiters(space: u64, va: u64) -> usize {
    let key = inter_process_communication::futex::Key { space, address: va };
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    // SAFETY: as `futex_wake`'s.
    sched.futexes.waiters(key, |w| unsafe { futex_key_of(w) })
}
