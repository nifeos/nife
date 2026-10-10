//! **Configuring an embryo**: binding its address space and entry, its thread pointer, and its
//! cycle-counter grant, everything fixed on a thread before `START` runs it, plus a thread changing
//! its own thread pointer later. Cut out of `sched.rs` along this seam by milestone 812
//! (`std::thread::spawn` runs real threads in one address space), whose thread pointer (§269 (how
//! threads share a process) fork 4) is the new part, because that file is at its §266 (a Rust
//! source file stays under 2,000 lines) ceiling. Nothing moved here changed. The register halves
//! are `arch::thread_pointer` and the scope note is `notes/thread-pointer.md`.
//!
//! Name: provisional (milestone 812's lane, 2026-10-10 UTC).

use super::{IPC_TABLES, State, ThreadId, current_thread_id};

/// **Configure an embryo** (milestone 19c (run a real workload), step 19c.3; §249 (a running address
/// space stays nameable)): bind
/// the address space named by `aspace_name` and set the EL0 entry and user stack. Refuses anything
/// but an `Embryo`, so a running thread cannot be reconfigured under itself. `Ok(())` or a reason.
///
/// **The space stays in the registry and keeps its name** (§249's option A). Until 2026-10-05 this
/// moved the space out of the registry into the thread, which retired the name, so no capability
/// could name a running space and `UNMAP` could not reach the window milestone 95 (an unmap
/// primitive) exists to close. Now the thread keeps a copy of what the context switch reads, and the
/// registry records which thread the space is bound to, which is also what refuses a second bind
/// (§249's amendment (b), `WrongObject`; §105 (`std::thread::spawn` stays declined) stands on it).
///
/// The embryo check runs first, alone, so a TCB that is not an embryo answers `WrongObject` before
/// a stale space name answers `NoSuchSlot`, the order this function has always refused in. It runs
/// again inside the bind, under both locks, because the first answer can be stale by then.
pub fn configure_thread_control_block(
    tid: ThreadId,
    entry: u64,
    user_sp: u64,
    aspace_name: u64,
) -> Result<(), abi::Error> {
    configure_thread_control_block_with_thread_pointer(tid, entry, user_sp, aspace_name, 0)
}

/// **Grant an embryo the cycle counter** (milestone 229, DECISIONS 139 option 4): the thread this
/// TCB names may read `PMCCNTR_EL0` (aarch64) or the `cycle` CSR (riscv64) from user mode once it
/// runs. `x86_64` already lets every thread read the TSC and this changes nothing there, which is
/// DECISIONS 139 part 3 and a stated exception to §19 rather than a gap.
///
/// **Refuses a non-embryo**, exactly as [`configure_thread_control_block`] and
/// [`thread_control_block_insert_cap`] do, and that refusal is the security property rather than
/// housekeeping: it is what makes this a field in the thread's spawn manifest instead of something
/// a running program can ask for. A timing instrument acquired at will is a timing instrument
/// nobody declared.
///
/// One-way: there is no ungrant, because an embryo starts closed and nothing but this opens it.
///
/// **Nothing calls this today, and that is milestone 229's decision rather than an oversight.**
/// The syscall method that would let a loader call it was deliberately not minted: see
/// `abi::thread_control_block`'s standing note, whose short form is that a method number is
/// irreversible and `seL4_TCB_SetAffinity` is the worked example of one that had to be retired.
/// This is the kernel half of the mechanism, complete and tested, waiting for whoever mints the
/// surface with a requirement in hand.
///
/// `#[inline(never)]` for the reason milestone 156 gives `memory_region_map` and the other
/// spawn-path bodies: this is administration a loader runs once per child, never a step of the IPC
/// round trip, so it does not belong in the bytes `script/fastpath-footprint` bounds. It is not a
/// style choice here, it is a measurement: without it the riscv64 `syscall_entry` set grew 12%
/// against a 5% bound, because the callee folded into `invoke`.
#[inline(never)]
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub fn grant_cycle_counter(tid: ThreadId) -> Result<(), abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
    let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
    if t.handshake.state != State::Embryo {
        return Err(abi::Error::WrongObject);
    }
    t.cycle_counter_grant = true;
    Ok(())
}

/// **Grant the *running* thread the cycle counter, for tests only** (milestone 229).
///
/// This deliberately breaks the rule [`grant_cycle_counter`] enforces, which is why it is
/// `#[cfg(test)]` and cannot exist in a shipped kernel. It is here because milestone 229 shipped
/// the mechanism without the syscall method that would set it, so there is no honest userspace
/// route to a granted thread and the alternative was to leave the EL0 half of the mechanism
/// unexercised. Same spirit as the `soak` and `fastpath_pad` affordances: a door that exists only
/// in a build nobody runs.
///
/// It writes the register itself as well as the field, because the calling thread is already
/// running and will not pass through `schedule`'s switch again before it drops to EL0. Every later
/// switch back into this thread re-applies the same value from the field, which is the ordinary
/// path.
#[cfg(feature = "system_tests")]
pub fn grant_cycle_counter_to_current() {
    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();
        sched
            .threads
            .get_mut(current)
            .expect("no current thread")
            .cycle_counter_grant = true;
    }
    crate::arch::timer::set_cycle_counter_grant(true);
}

/// [`configure_thread_control_block`], and the thread's first thread pointer with it (milestone
/// 812, §269 (how threads share a process) fork 4; Linux's `CLONE_SETTLS`). `CONFIGURE` reads it
/// from the sixth argument register, which every caller built before 812 sends as zero, and zero is
/// what a thread had before there was a field to hold it.
///
/// Refused with `BadPointer` before anything is bound if it is neither zero nor a user address, so
/// a refusal leaves the embryo exactly as it was. Set in the same critical section as the space and
/// the entry, so a `START` racing this `CONFIGURE` sees all three or none.
///
/// Name: provisional (milestone 812 (`std::thread::spawn` runs real threads in one address space)'s
/// lane, 2026-10-10 UTC).
pub fn configure_thread_control_block_with_thread_pointer(
    tid: ThreadId,
    entry: u64,
    user_sp: u64,
    aspace_name: u64,
    thread_pointer: u64,
) -> Result<(), abi::Error> {
    if !is_thread_pointer(thread_pointer) {
        return Err(abi::Error::BadPointer);
    }
    {
        let guard = IPC_TABLES.lock();
        let sched = guard.as_ref().ok_or(abi::Error::NoSuchSlot)?;
        let t = sched.threads.get(tid).ok_or(abi::Error::NoSuchSlot)?;
        if t.handshake.state != State::Embryo {
            return Err(abi::Error::WrongObject); // only an unstarted TCB may be configured
        }
    }

    // **This is the moment a bare address space becomes a thread's**, so it is the moment the
    // current-CPU page belongs in it (calef's 2026-09-21 ruling on a thread observing itself), and
    // `bind_user_address_space` attaches it before handing over the copy. The closure runs under the
    // registry's lock (`ADDRESS_SPACES`, 61) and takes `IPC_TABLES` (60) beneath it, so the
    // registry's bound mark and the thread's copy are written in one critical section.
    crate::user::bind_user_address_space(aspace_name, tid, |bound| {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
        let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
        if t.handshake.state != State::Embryo {
            return Err(abi::Error::WrongObject);
        }
        t.space = Some(bound);
        t.entry = (entry, user_sp);
        t.thread_pointer = thread_pointer;
        Ok(())
    })
}

/// **Is this a value a thread pointer may hold**: zero (none), or an address in the user half.
///
/// Not alignment, and not whether anything is mapped there: the register is a pointer the program
/// dereferences itself, and a bad one faults the program that chose it. The half matters because
/// `x86_64`'s `wrmsr` to `IA32_FS_BASE` raises `#GP` in the kernel on a non-canonical value, and a
/// kernel-half value is a pointer no user access through it could use. The same rule on all three
/// architectures, so a program refused on one is refused on every one (§19 (architectural parity is
/// a tenet)).
fn is_thread_pointer(value: u64) -> bool {
    use paging::PageFormat;
    value == 0 || crate::arch::mmu::Format::is_in_half(paging::Half::Low, value)
}

/// **The calling thread's thread pointer**, for the first entry to user mode: riscv64's
/// `TrapFrame::for_user_entry` builds the frame with it, since `tp` is a frame register there. Zero
/// for a thread that has none, and for a kernel thread calling from outside any thread table.
/// riscv64 only: the other two install the register at the switch and never ask.
#[cfg(target_arch = "riscv64")]
pub fn current_thread_pointer() -> u64 {
    let guard = IPC_TABLES.lock();
    guard
        .as_ref()
        .and_then(|sched| sched.threads.get(current_thread_id()))
        .map_or(0, |t| t.thread_pointer)
}

/// **`ThreadControlBlock::SET_THREAD_POINTER`**: change the thread pointer of the thread `tid`
/// names (milestone 812, §269 fork 4; seL4's `SetTLSBase`).
///
/// Two targets are allowed, and the refusal of the third is what keeps this simple. An **embryo**
/// (not started) takes the value at its first switch in. **The caller itself** takes it now: the
/// field and the register (`arch::thread_pointer::set_live`, given `frame`, the frame the caller
/// returns through) change under one hold of `IPC_TABLES`. One hold, because on aarch64 a switch
/// between the two writes would save the old register over the new field. **Any other started thread is refused with
/// `WrongObject`**: its register lives on whatever core it runs on, or in a saved context or trap
/// frame whose location differs per architecture, and nothing `std` does needs to reach it. seL4
/// allows it; the refusal can be lifted additively if a debugger ever wants it.
///
/// `BadPointer` for a value [`is_thread_pointer`] refuses, before anything changes.
///
/// Name: provisional (milestone 812's lane, 2026-10-10 UTC).
pub fn set_thread_pointer(
    tid: ThreadId,
    value: u64,
    frame: &mut crate::arch::exceptions::TrapFrame,
) -> Result<(), abi::Error> {
    if !is_thread_pointer(value) {
        return Err(abi::Error::BadPointer);
    }
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
    let caller = current_thread_id();
    let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
    if t.handshake.state == State::Embryo {
        t.thread_pointer = value;
    } else if tid == caller {
        t.thread_pointer = value;
        crate::arch::thread_pointer::set_live(frame, value);
    } else {
        return Err(abi::Error::WrongObject);
    }
    Ok(())
}
