//! **The root supervisor: a first process that gives its authority away** (milestone 22 phase B.2).
//!
//! It holds full construction authority exactly as long as it takes to build two servers, a
//! `spawner` that can build only one program image and a `sub_server_supervisor` that holds no
//! memory, then **deletes it** and becomes a `RECEIVE` loop on its own supervision endpoint. After
//! that it cannot build a process, endow anyone with memory, or reach the sub-servers' internals: a
//! compromised `root_supervisor` can lie about deaths on one endpoint, and that is the entire blast
//! radius. The restart policy is `sub_server_supervisor`'s; the kernel never relaunches anything
//! (DECISIONS §26 (the fault endpoint)). notes/trusted-init/supervision-tree.md draws the tree.
//!
//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `rootsup`. Refused `rootsup` and
//! `root_sup` (`sup` was the abbreviation, so putting a separator in it would have relocated the
//! problem rather than fixed it).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

// Each binary in the tree compiles the shared module but uses a different slice of it (the sub-server
// builds nothing, the supervisor holds no memory), so the unused halves are expected, not dead.
use supervision_protocol::{
    ChildEndowment, REPORT_CONSTRUCTION_AUTHORITY_DROPPED, REPORT_FAILED, REPORT_SUP_SAW_DEATH,
    Retention,
};
use user_mode_runtime::{
    cap_delete, receive, retype_into_kernel_object, retype_into_page_frame, send,
};

/// What the kernel grants us, and nothing else.
const ROOT_UT: u64 = 0; // the construction budget: ours briefly, then deleted
const REPORT: u64 = 1; // WRITE|GRANT, so we can endow the servers with a narrowed view

/// The budget we carve off for the spawner. It builds every instance of the sub-server out of this,
/// each in its own reclaimable region, so this is the only memory the tree ever spends after boot.
const SPAWNER_BUDGET_PAGES: u64 = 384;

/// Where the spawner finds the one program image it is allowed to build. Must match spawner.rs.
const SPAWNER_IMAGE_VA: u64 = address_space_map::runtime_window(0x3000_0000);

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, initial_ramdisk_len: u64, _a2: u64) -> ! {
    // SAFETY: forwarded from user_mode_runtime::initial_ramdisk::bytes's own contract.
    let archive = unsafe { user_mode_runtime::initial_ramdisk::bytes(initial_ramdisk_len) };
    let Ok(fs) = nifefs::Fs::parse(archive) else {
        bail(1)
    };
    let Some(spawner) = fs.read("spawner").and_then(|b| elf::Elf::parse(b).ok()) else {
        bail(2)
    };
    let Some(sub_server_supervisor) = fs
        .read("sub_server_supervisor")
        .and_then(|b| elf::Elf::parse(b).ok())
    else {
        bail(3)
    };
    // The sub-server's image travels as *bytes*, not as an archive: the spawner gets exactly the one
    // program it may build. We hold the initial ramdisk; it never does.
    let Some(first_attempt_crasher) = fs.read("first_attempt_crasher") else {
        bail(4)
    };

    // The channels. The spawner serves `req` and answers on `rep`; `childfault` is where the
    // sub-server's deaths go (sub_server_supervisor receives them); `rootfault` is where OUR children's deaths go.
    let Ok(req) = supervision_protocol::retype_obj_from(ROOT_UT, abi::objtype::RENDEZVOUS) else {
        bail(5)
    };
    let Ok(rep) = supervision_protocol::retype_obj_from(ROOT_UT, abi::objtype::RENDEZVOUS) else {
        bail(6)
    };
    let Ok(childfault) = supervision_protocol::retype_obj_from(ROOT_UT, abi::objtype::RENDEZVOUS)
    else {
        bail(7)
    };
    let Ok(rootfault) = supervision_protocol::retype_obj_from(ROOT_UT, abi::objtype::RENDEZVOUS)
    else {
        bail(8)
    };
    let Ok(sp_ut) = supervision_protocol::memory_region_split(ROOT_UT, SPAWNER_BUDGET_PAGES) else {
        bail(9)
    };

    // 1. The construction sub-server. It holds a budget (`SPEND_ONLY`, every right but GRANT: it
    //    may *spend* memory, never lend it, nor since milestone 824 (a retype mints no right its
    //    budget lacks) anything it retypes from it), the request pair, a grantable report endpoint so it can endow the children it
    //    builds, a grantable view of the child fault endpoint so children are born supervised, and
    //    the one program image. It cannot build anything else, because it has nothing else to build.
    let Ok(child) = supervision_protocol::build_child(
        ROOT_UT,
        ROOT_UT,
        &spawner,
        &ChildEndowment {
            caps: &[
                (req, abi::rights::READ),
                (rep, abi::rights::WRITE),
                (sp_ut, abi::rights::SPEND_ONLY),
                (REPORT, abi::rights::WRITE | abi::rights::GRANT),
                (childfault, abi::rights::READ | abi::rights::GRANT),
            ],
            maps: &[],
            blobs: &[(SPAWNER_IMAGE_VA, first_attempt_crasher)],
            fault: Some(rootfault),
            ..ChildEndowment::new(Retention::Nothing)
        },
    ) else {
        bail(10)
    };
    if !supervision_protocol::start_child(child, 0, first_attempt_crasher.len() as u64, 0) {
        bail(11)
    }

    // 2. The supervisor. **It holds no memory at all**: a request channel, a death channel, and a way
    //    to report. Its restart policy is code, not authority, so a compromised sub_server_supervisor can ask for
    //    rebuilds of one program and nothing more.
    let Ok(child) = supervision_protocol::build_child(
        ROOT_UT,
        ROOT_UT,
        &sub_server_supervisor,
        &ChildEndowment {
            caps: &[
                (req, abi::rights::WRITE),
                (rep, abi::rights::READ),
                (childfault, abi::rights::READ),
                (REPORT, abi::rights::WRITE),
            ],
            maps: &[],
            blobs: &[],
            fault: Some(rootfault),
            ..ChildEndowment::new(Retention::Nothing)
        },
    ) else {
        bail(12)
    };
    if !supervision_protocol::start_child(child, 0, 0, 0) {
        bail(13)
    }

    // 3. Give it all away. The wiring caps first (the servers hold their own narrowed copies), then
    //    the spawner's budget, then the construction budget itself. From here we cannot make a page,
    //    an address space, a thread, or an endpoint.
    for c in [req, rep, childfault, sp_ut] {
        cap_delete(c);
    }
    cap_delete(ROOT_UT);

    // And prove it, from the inside, on the two primitives that build things: retype a page, and
    // retype a kernel object. Both must now fail with `NoSuchSlot` (-1), which is what
    // no-ambient-authority feels like: there is nothing there, and no way to name what used to be.
    let frame = retype_into_page_frame(ROOT_UT);
    let object = retype_into_kernel_object(ROOT_UT, abi::objtype::THREAD_CONTROL_BLOCK);
    send(
        REPORT,
        REPORT_CONSTRUCTION_AUTHORITY_DROPPED,
        (frame < 0 && object < 0) as u64,
        (-frame) as u64,
    );

    // 4. What is left: the irreducible root. Its whole policy is "hear it, say it, stop." It cannot
    //    restart a dead tier-one server, because reaping and rebuilding both need WRITE on a region,
    //    which is the construction authority we just gave up on purpose. That tradeoff is recorded in
    //    DECISIONS §26's phase B block: a root that can restart is a root that can build, and the
    //    fail-closed floor is the more valuable of the two.
    loop {
        let (event, tid, _pc) = receive(rootfault);
        send(REPORT, REPORT_SUP_SAW_DEATH, tid, event);
    }
}

/// Report which stage failed, then trap. A half-built tree is not worth limping along, and the stage
/// code turns "nothing happened" into a legible failure.
fn bail(stage: u64) -> ! {
    send(REPORT, REPORT_FAILED, stage, 0);
    supervision_protocol::fail()
}

user_mode_runtime::panic_handler!();
