//! **A process composed from two capabilities, run in the space it was built in**, the claim of
//! §185 (what carries the claim that userspace composes a process from an authority you can count
//! on one hand). It holds a memory region (slot 0) and a report line (slot 1), and from those alone
//! composes and runs a child in the steps below. The kernel allocates nothing on the way: every
//! object is retyped from the region it was handed. Reading the archive is not a third capability:
//! the kernel maps it read-only at `user_mode_runtime::initial_ramdisk::INITIAL_RAMDISK_VA`, naming
//! no kernel object. The verdict is one word, a bit per step
//! (`capability_witness_contract::process_composition`).

//! # The steps
//!
//! 1. Read `least_authority_demo` out of the archive by name and parse its ELF.
//! 2. Mint a rendezvous out of its own memory, for the child to answer on.
//! 3. Build the child with `supervision_protocol::build_child_space`, every composer's loader: an
//!    address space, W^X segments, a stack, a thread, and `WRITE` on that rendezvous in slot 0.
//! 4. Map a frame of its own into that space twice, which must be refused (break-before-make holds
//!    in a space a process built).
//! 5. Configure the thread at the child's entry in that space and start it with an input.
//! 6. Receive the child's answer, the input squared, on the rendezvous it minted.

//! # BUGS
//!
//! - A child that never answers hangs this program, which has no deadline and does not supervise
//!   it (that would cost a third capability); the kernel's test waits with one.
//! - The child is loaded unmeasured (notes/trusted-init.md).
//!
//! Name: ratified 2026-10-05 (calef, §185): a fixture is named for what it proves, not what it does.
//! Refused `address_space_builder` (an action) and `builder` (a generic word that could name almost
//! anything in an operating system).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use capability_witness_contract::process_composition::{
    CHILD_ANSWERED, CHILD_FOUND, DOUBLE_MAP_REFUSED, FRAME_MAPPED, SPACE_BUILT, STARTED,
};
use supervision_protocol::{
    ChildEndowment, Retention, build_child_space, configure_child, start_child,
};
use user_mode_runtime::{
    address_space_map_into, exit, receive, retype_into_kernel_object, retype_into_page_frame, send,
};

const MEMORY_REGION: u64 = 0;
const REPORT: u64 = 1;

/// The child this composes: one capability in slot 0, squares the input in `x1`, sends it there.
const CHILD: &str = "least_authority_demo";
const INPUT: u64 = 9;

/// Where the witness maps a frame of its own into the child's space for milestone 19b's probe: in
/// the pair band, so nothing the loader lays down can be there already.
const VA: u64 = address_space_map::pair_page(0x0040_0000);

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, initial_ramdisk_len: u64, _arg2: u64) -> ! {
    send(REPORT, compose(initial_ramdisk_len), 0, 0);
    exit()
}

/// Every step, returning the verdict bits it reached. Stops at the first step that fails, so the
/// word says where.
fn compose(initial_ramdisk_len: u64) -> u64 {
    // SAFETY: forwarded from user_mode_runtime::initial_ramdisk::bytes's own contract: the kernel
    // mapped the archive at INITIAL_RAMDISK_VA and passed its length in x1.
    let archive = unsafe { user_mode_runtime::initial_ramdisk::bytes(initial_ramdisk_len) };
    let Some(elf) = nifefs::Fs::parse(archive)
        .ok()
        .and_then(|fs| fs.read(CHILD))
        .and_then(|bytes| elf::Elf::parse(bytes).ok())
    else {
        return 0;
    };
    let mut verdict = CHILD_FOUND;

    let answer = retype_into_kernel_object(MEMORY_REGION, abi::objtype::RENDEZVOUS);
    if answer < 0 {
        return verdict;
    }
    let answer = answer as u64;
    let caps = [(answer, abi::rights::WRITE)];
    let endow = ChildEndowment {
        caps: &caps,
        ..ChildEndowment::new(Retention::Nothing)
    };
    // One budget pays for both the child and our own scratch mappings: there is only the one, and
    // nothing here destroys it while the child lives.
    let Ok((child, aspace)) = build_child_space(MEMORY_REGION, MEMORY_REGION, &elf, &endow) else {
        return verdict;
    };
    verdict |= SPACE_BUILT;

    let frame = retype_into_page_frame(MEMORY_REGION);
    if frame >= 0 {
        let frame = frame as u64;
        if address_space_map_into(aspace, VA, frame, abi::address_space::MAP_RW) == 0 {
            verdict |= FRAME_MAPPED;
        }
        if address_space_map_into(aspace, VA, frame, abi::address_space::MAP_RW) < 0 {
            verdict |= DOUBLE_MAP_REFUSED;
        }
    }

    // CONFIGURE consumes the address space capability; START, with `Retention::Nothing`, our
    // thread control block capability. After this the child is reachable only through what it holds.
    if configure_child(child.tcb, aspace, elf.entry()).is_err() || !start_child(child, 0, INPUT, 0)
    {
        return verdict;
    }
    verdict |= STARTED;

    let (word, _, _) = receive(answer);
    if word == INPUT * INPUT {
        verdict |= CHILD_ANSWERED;
    }
    verdict
}

user_mode_runtime::panic_handler!();
