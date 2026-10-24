//! The confiner of a C component (milestone 36 (a foreign-language component, seam first),
//! DECISIONS §31 (the foreign-language seam)): three roles, fused on purpose; see notes/c-seam.md.
//!
//! 1. Builder: splits a region off its budget per instance and lays `c_shim` out in it, so a single
//!    `MemoryRegion::DESTROY` reaps a whole instance (§16 (object revocation)).
//! 2. Supervisor: holds the supervision endpoint (§26), reaps each death through it (§32 (a
//!    supervisor may collect a corpse)) and restarts; the kernel relaunches nothing.
//! 3. Checker: holds the witness pages in its own address space, so "nothing outside the grant
//!    changed" is asserted from the other side of an MMU boundary.
//!

//! Name: ratified 2026-08-01 (calef, milestone 61), replacing `cwarden`. Refused `cwarden` and the
//! whole `warden` family, a synonym this project invented for a pattern that already has a name:
//! DECISIONS §50 settled that using the literature's word claims "this is that", where a synonym
//! asserts novelty there is none. Refused the caretaker noun for this one specifically: it holds a
//! region and confines foreign code rather than attenuating a directory capability to a narrower
//! one, so it is deliberately outside that family.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

// What this binary shares with `c_shim` and the supervision tree comes from the `c_seam` and
// `supervision_protocol` crates (rule 7), so this file needs no dead-code allow under §38 (a
// suppression is scoped to an item and carries a reason).

// The loader, shared with the milestone-22 supervision tree. `ChildEndowment.maps` and `ChildEndowment.fault` are the
// two parts this milestone leans on: a child born with shared pages and born supervised.
use c_seam::checks;
use supervision_protocol::{ChildEndowment, Retention};
use user_mode_runtime::{cap_delete, map_page_frame, receive_fault, send};

/// What the kernel grants us, and nothing else.
const ROOT_UT: u64 = 0; // the construction budget: what we build each instance out of
const REPORT: u64 = 1; // WRITE|GRANT, so each instance gets its own narrowed view

/// Pages per instance region. A debug-build `c_shim` is a couple of dozen pages of segments plus
/// its stack, its page tables, its TCB, its address space, and the eight-page heap `malloc` grows
/// into. Each instance is reaped before the next is built, so this is a peak, not a total.
const INSTANCE_PAGES: u64 = 96;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, initial_ramdisk_len: u64, _a2: u64) -> ! {
    // SAFETY: forwarded from user_mode_runtime::initial_ramdisk::bytes's own contract.
    let archive = unsafe { user_mode_runtime::initial_ramdisk::bytes(initial_ramdisk_len) };
    let Ok(fs) = nifefs::Fs::parse(archive) else {
        bail(1)
    };
    let Some(shim) = fs.read("c_shim").and_then(|b| elf::Elf::parse(b).ok()) else {
        bail(2)
    };

    // The three pages the whole proof rests on. All from our own budget, all ours to start with; the
    // C component's process will see two of them and never the third.
    let Ok(grant) = supervision_protocol::retype_page_frame_from(ROOT_UT) else {
        bail(3)
    };
    let Ok(wit_ro) = supervision_protocol::retype_page_frame_from(ROOT_UT) else {
        bail(4)
    };
    let Ok(wit_far) = supervision_protocol::retype_page_frame_from(ROOT_UT) else {
        bail(5)
    };

    // Map all three read/write **into ourselves**, at the same virtual addresses the C component will
    // use. Same numbers on both sides is what lets the wild-store address be compared directly
    // against the address the kernel reports, with no translation step to get wrong.
    for (frame, va) in [
        (grant, c_seam::GRANT_VA),
        (wit_ro, c_seam::WITNESS_RO_VA),
        (wit_far, c_seam::WITNESS_FAR_VA),
    ] {
        if !map_page_frame(frame, va, true, ROOT_UT) {
            bail(6)
        }
    }
    for i in 0..c_seam::PAGE as usize {
        witness_ro()[i] = c_seam::pattern_ro(i);
        witness_far()[i] = c_seam::pattern_far(i);
    }

    // Our children's deaths arrive here. We are the only holder with READ, and the kernel is the only
    // sender (§26.5), so both the tid and the fault address are trustworthy without a badge.
    let Ok(faultep) = supervision_protocol::retype_obj_from(ROOT_UT, abi::objtype::RENDEZVOUS)
    else {
        bail(7)
    };

    let mut attempt = 0u64;
    while attempt < c_seam::ATTEMPTS {
        // Reset the grant before every attempt. Necessary, not hygiene: the misbehaving attempts
        // scribble a marker over the input's first byte, so without this the honest run would be
        // transforming the wreckage of the previous one.
        let g = grant_page();
        g.fill(0);
        g[c_seam::IN_OFF..c_seam::IN_OFF + c_seam::INPUT.len()].copy_from_slice(c_seam::INPUT);

        let Ok(region) = supervision_protocol::memory_region_split(ROOT_UT, INSTANCE_PAGES) else {
            bail(10)
        };
        let Ok(child) = supervision_protocol::build_child(
            ROOT_UT,
            region,
            &shim,
            &ChildEndowment {
                // The shell's whole authority: say what happened, and spend the region it lives in
                // (which is what pays for `malloc`). No GRANT on either, so it can neither lend the
                // report endpoint on nor hand its budget to anyone.
                caps: &[
                    (REPORT, abi::rights::WRITE),
                    (region, abi::rights::SPEND_ONLY),
                ],
                // The grant is read/write, which is the authority the C code abuses. The witness page
                // right behind it is read-only, which is what turns an off-by-one into a fault
                // instead of a corruption. Nothing maps WITNESS_FAR here, deliberately.
                maps: &[
                    (c_seam::GRANT_VA, grant, abi::address_space::MAP_RW),
                    (c_seam::WITNESS_RO_VA, wit_ro, abi::address_space::MAP_RO),
                ],
                blobs: &[],
                fault: Some(faultep),
                ..ChildEndowment::new(Retention::Nothing)
            },
        ) else {
            bail(11)
        };
        // Neither capability is the thing itself, and neither is needed any more. The TCB capability
        // is not the thread (dropping it leaves the thread running, which is why the endowment
        // declares `Retention::Nothing` and `start_child` drops it for us), and since §32 the region
        // capability is not the reap: the corpse is collected through the supervision endpoint. So we
        // hold nothing that reaches a live instance's memory, and the child keeps the narrowed copy of
        // the region it was endowed with, which is what `malloc` spends.
        if !supervision_protocol::start_child(child, 0, attempt, 0) {
            bail(12)
        }
        cap_delete(region);

        // Block until the child dies, one way or the other. All five words, because the fourth is the
        // faulting address and this is the program that cares where the C code pointed.
        let (event, tid, pc, addr, ..) = receive_fault(faultep);
        send(REPORT, c_seam::RPT_DEATH, tid, event);
        send(REPORT, c_seam::RPT_SITE, pc, addr);
        send(
            REPORT,
            c_seam::RPT_VERDICT,
            attempt,
            verdict(attempt, event, pc, addr),
        );

        // Reap, through the supervision endpoint the death arrived on, naming the tid the kernel
        // stamped on it (§32). The corpse is dead-until-reaped (§26.4), so its region stayed pinned
        // until now and the verdict above was computed while it was still there to inspect. We hold no
        // capability to that region: the authority for this is the supervision relationship, and the
        // pages go back to ROOT_UT, which is where they came from.
        if user_mode_runtime::reap(faultep, tid) != 0 {
            bail(13)
        }

        // The honest attempt's clean exit is "finished", not "crashed", and that distinction is
        // why §26 (the fault endpoint: thread death becomes a message a supervisor holds)
        // delivers both events. The underrun runs after it, so the loop's own bound ends the run.
        attempt += 1;
    }

    // Park rather than exit, so we do not become a death of our own for somebody else to handle.
    loop {
        let (event, tid, pc, addr, ..) = receive_fault(faultep);
        send(REPORT, c_seam::RPT_DEATH, tid, event);
        send(REPORT, c_seam::RPT_SITE, pc, addr);
    }
}

/// **The verdict on one attempt**: a bitmap of the confinement claims that held.
///
/// Every bit is a separate question asked of memory, not of the component. The component is not
/// consulted at all: it is dead by the time this runs.
fn verdict(attempt: u64, event: u64, pc: u64, addr: u64) -> u64 {
    let mut bits = 0u64;
    let g = grant_page();

    // Claim 1: the component's stores *inside* its grant landed. Without this every other bit could
    // be satisfied by a process whose stores never worked at all, which would prove nothing.
    let in_grant_landed = if attempt == c_seam::ATTEMPT_HONEST {
        // The honest path writes a checksum, so a non-zero one is the evidence its stores worked.
        g[c_seam::OUT_OFF..c_seam::OUT_OFF + 4]
            .iter()
            .any(|&b| b != 0)
    } else {
        g[0] == c_seam::MARK
    };
    if in_grant_landed {
        bits |= checks::IN_GRANT_WRITE_LANDED;
    }

    // Claim 2 and 3: the witnesses. Read through OUR mappings, in OUR address space, after the
    // component is dead. Every byte, because a partial overwrite is still an escape.
    if (0..c_seam::PAGE as usize).all(|i| witness_ro()[i] == c_seam::pattern_ro(i)) {
        bits |= checks::WITNESS_RO_INTACT;
    }
    if (0..c_seam::PAGE as usize).all(|i| witness_far()[i] == c_seam::pattern_far(i)) {
        bits |= checks::WITNESS_FAR_INTACT;
    }

    // Claim 4: the kernel's fault site is the site the C code computed. This is what rules out "it
    // crashed for some other reason on the way", which would make the witness checks vacuous.
    let expect_addr = match attempt {
        c_seam::ATTEMPT_OVERRUN => Some(c_seam::WITNESS_RO_VA),
        c_seam::ATTEMPT_WILD => Some(c_seam::WITNESS_FAR_VA),
        // The underrun aims one page below the grant, an address nothing in the component maps:
        // a translation fault at a site no other attempt reaches.
        c_seam::ATTEMPT_UNDERRUN => Some(c_seam::GRANT_VA - c_seam::PAGE),
        _ => None,
    };
    let site_ok = match expect_addr {
        Some(want) => event == abi::fault::EVENT_FAULT && addr == want && pc != 0,
        // A clean exit reports no site at all, and §26 says both words are zero. Asserting that is
        // asserting the kernel does not leak a stale register into an EXIT message.
        None => event == abi::fault::EVENT_EXIT && addr == 0 && pc == 0,
    };
    if site_ok {
        bits |= checks::FAULT_ADDR_AS_EXPECTED;
    }

    // Claim 5, the honest attempt only: the C actually computed the right answer, checked against an
    // independent Rust implementation of the same definition. A restart that produced a corpse that
    // "ran" but computed nothing would otherwise pass.
    if attempt == c_seam::ATTEMPT_HONEST && is_output_correct(g) {
        bits |= checks::OUTPUT_CORRECT;
    }

    bits
}

/// Is the honest attempt's output in the grant, and right? The checksum against a Rust
/// recomputation, and the transformed string byte for byte including its terminator.
fn is_output_correct(g: &mut [u8]) -> bool {
    let text = &c_seam::INPUT[..c_seam::INPUT.len() - 1]; // without the NUL the C stops at
    let want = c_seam::expected_checksum(text);
    let got = u32::from_le_bytes([
        g[c_seam::OUT_OFF],
        g[c_seam::OUT_OFF + 1],
        g[c_seam::OUT_OFF + 2],
        g[c_seam::OUT_OFF + 3],
    ]);
    if got != want {
        return false;
    }
    let out = &g[c_seam::OUT_OFF + 4..];
    c_seam::INPUT.iter().enumerate().all(|(i, &b)| {
        let up = if b.is_ascii_lowercase() { b - 32 } else { b };
        out[i] == up
    })
}

/// The three pages, as slices. Separate functions rather than one static, because each is a distinct
/// claim and mixing them up in the checker would be the most embarrassing possible bug.
fn grant_page() -> &'static mut [u8] {
    // SAFETY: a page we mapped read/write into our own address space at `_start`, and never unmap.
    unsafe { core::slice::from_raw_parts_mut(c_seam::GRANT_VA as *mut u8, c_seam::PAGE as usize) }
}
fn witness_ro() -> &'static mut [u8] {
    // SAFETY: ours, read/write, mapped at `_start`. The C component holds a read-only view of the
    // same physical frame, which is the point.
    unsafe {
        core::slice::from_raw_parts_mut(c_seam::WITNESS_RO_VA as *mut u8, c_seam::PAGE as usize)
    }
}
fn witness_far() -> &'static mut [u8] {
    // SAFETY: ours, read/write, mapped at `_start`. No other address space maps this frame at all.
    unsafe {
        core::slice::from_raw_parts_mut(c_seam::WITNESS_FAR_VA as *mut u8, c_seam::PAGE as usize)
    }
}

/// Report which stage failed, then trap. A half-built harness is not worth limping along, and the
/// stage code turns "nothing happened" into a legible failure.
fn bail(stage: u64) -> ! {
    send(REPORT, c_seam::RPT_FAILED, stage, 0);
    supervision_protocol::fail()
}

user_mode_runtime::panic_handler!();
