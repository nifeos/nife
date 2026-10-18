//! **What this shell was given, and where**: its capability slots, the authorities it probes for
//! at `_start`, the clock slot it is told, its budget, the windows it maps and the proof that
//! they are disjoint, and the delegations that hand a capability on to the progenitor.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

// Capability slots.
pub(super) const TERM: u64 = 0; // CALL requests on the terminal

pub(super) const SPAWN: u64 = 1; // SEND a spawn request to the progenitor

pub(super) const RESULT: u64 = 2; // RECEIVE a spawned program's answer

pub(super) const BUDGET: u64 = 3; // our own untyped; SPLIT a grant off it for `--mem`

/// The budget the progenitor granted us at boot, [`spawnproto::SHELL_BUDGET_PAGES`], which it reads
/// too. We cannot query how much remains (there is no such syscall), so `caps` prints the initial
/// grant.
pub(super) const SH_BUDGET_PAGES: u64 = spawnproto::SHELL_BUDGET_PAGES;

/// **The slot holding the clock page**, or [`NO_CLOCK`] when this wiring granted none. Told to us in
/// `x2` at [`_start`](crate::entry::_start) rather than fixed, because the clock is granted after the filesystem pair and a
/// boot with no disk attached has one fewer capability under it.
///
/// Being *told* is the same shape as `arg1` carrying the directory's rights, and for the same reason
/// recorded there: nothing in this system reports what a process holds, so a shell that guessed would
/// be guessing about its own capability table. A wrong guess here is worse than the directory's, because
/// probing the wrong slot would find some *other* object and map a page that is not a clock.
pub(super) static CLOCK_SLOT: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(NO_CLOCK);

/// **Whether this session holds the run-unvouched capability** (DECISIONS §219 gate D2), at
/// [`spawnproto::RUN_UNVOUCHED_SLOT`], `WRITE` only.
///
/// Probed once, at [`_start`](crate::entry::_start), and that timing is the soundness argument rather than a detail: at
/// `_start` this shell has allocated nothing, so the slot holds exactly what its builder placed
/// there. Probed any later, a frame or a region this shell retyped could have landed in it. Probed
/// rather than told because `x0` to `x2` are all spoken for (role, directory rights, clock slot),
/// and a named slot is how this tree already places an authority a program looks for
/// (`grant_plan::NETWORK_SLOT`).
///
/// Holding it changes one thing: an image request claims it and sends one message on it
/// ([`run_image`]), so a digest miss runs with only what the line granted instead of being
/// refused. It is not something this shell can hand on: it carries no `GRANT`.
pub(super) static HOLDS_RUN_UNVOUCHED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

// `grant_plan` states the slot without depending on `abi`; the relation is held by each reader.
const _: () = assert!(spawnproto::RUN_UNVOUCHED_SLOT == abi::fault::FAULT_EP_SLOT - 1);

/// **Whether this shell holds a read-only view of the inert-configuration page**
/// ([`grant_plan::SHELL_CONFIG_SLOT`], mapped at [`grant_plan::SHELL_CONFIG_VA`]; milestone 47
/// (navigation and naming), DECISIONS §111 (inert configuration is a validated page)). Only the
/// progenitor places it, and it maps the page in the same build, so a capability in the slot at
/// `_start` means the page is there to read. Probed once at `_start` for [`HOLDS_RUN_UNVOUCHED`]'s
/// reason. `caps` reads it to print the values a child declaring `config` will read; nothing else
/// in this shell looks at it.
pub(super) static HOLDS_CONFIG: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

// The named slot sits under the run-unvouched one and so under the kernel's fault slot.
const _: () = assert!(grant_plan::SHELL_CONFIG_SLOT < spawnproto::RUN_UNVOUCHED_SLOT);

/// **Whether this session holds the machine statistics page** (milestone 126 (the `procps`
/// package), DECISIONS §225 (`free` sees the machine and your share)), at
/// [`spawnproto::MACHINE_PAGE_SLOT`], `READ | GRANT`. Probed once at [`_start`](crate::entry::_start), for
/// [`HOLDS_RUN_UNVOUCHED`]'s reason.
///
/// Holding it changes one thing: a program whose manifest declares `machine` is sent the page with
/// its spawn request ([`machine_wiring`]), so it can see how the machine is doing. A session the
/// owner withheld it from cannot pass on what it does not hold, and `free` then says so.
pub(super) static HOLDS_MACHINE_PAGE: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Whether a spawn of `e` sends the machine statistics page: the program declared it and this
/// session holds it.
pub(super) fn machine_wiring(e: &Endowment) -> bool {
    e.program.manifest().machine && HOLDS_MACHINE_PAGE.load(core::sync::atomic::Ordering::Relaxed)
}

/// **Send the machine statistics page, the last delegated capability of a request that set
/// `machine`** (`spawnproto::MACHINE_BIT`). We keep our own copy.
pub(super) fn delegate_machine_page(wired: bool) {
    if wired {
        delegate(
            spawnproto::MACHINE_PAGE_SLOT,
            abi::rights::READ | abi::rights::GRANT,
        );
    }
}

/// The `x2` value meaning "this shell was granted no clock". Zero rather than a sentinel, because
/// slot 0 is the terminal in every wiring, so no clock can ever legitimately be there.
pub(super) const NO_CLOCK: u64 = 0;

/// **What this shell holds, which is what decides whether a named file can be backed.**
///
/// A per-file grant is a directory capability narrowed to one name (milestone 31 phase 2,
/// `components/src/fs_file_caretaker.rs`), and a redirection is a file opened through one. Both need a
/// directory to narrow, so both answer to this: a shell whose the progenitor granted it a directory
/// capability (slot [`DIR_TERMINAL`]) can back `>` and `<`, and a shell whose the progenitor granted it none
/// gets the milestone's headline refusal, which is **true** rather than a placeholder.
///
/// The same binary is in both positions, which is the point: `date > report.txt` is refused in the
/// wiring with no filesystem and runs in the wiring with one, and nothing in this file branches on
/// which. See notes/pipes.md.
pub(super) fn holdings(nav: &Nav) -> grant_plan::Holdings {
    nav.holds
}

/// Carve `pages` off our own untyped budget (slot 3) into a delegatable child untyped. `None` when
/// the budget is exhausted.
pub(super) fn memory_region_split(pages: u64) -> Option<u64> {
    let r = split_memory_region(BUDGET, pages);
    if r < 0 { None } else { Some(r as u64) }
}

pub(super) const PAGE: u64 = 4096;

/// Where the wiring maps this shell's clock page read-only: `crates/system_initializer`'s
/// `SH_CLOCK_VA`, which must match. Nothing here reads through it by address; it is named so that
/// [`JOBFRAME_WINDOWS`] can be proven clear of it.
pub(super) const SH_CLOCK_VA: u64 = address_space_map::pair_page(0x0000_0000_00d0_0000);

// Every fixed window this shell maps, and the job frames' range, are disjoint. A window added to
// the shell belongs in this list.
const _: () = {
    let fixed: [(u64, u64); 6] = [
        (OUT_VA, PAGE),
        (LINE_VA, PAGE),
        (FS_VA, filesystem_contract::PAGE as u64),
        (SH_CLOCK_VA, PAGE),
        (grant_plan::SHELL_CONFIG_VA, PAGE),
        // The argv and name-set pages, and the image window above them (DECISIONS §219 option D).
        (IMAGE_WINDOW, IMAGE_WINDOW_END - IMAGE_WINDOW),
    ];
    let mut i = 0;
    while i < fixed.len() {
        let (va, len) = fixed[i];
        assert!(
            va + len <= JOBFRAME_WINDOWS.start || va >= JOBFRAME_WINDOWS.end,
            "a supervised job's frame window overlaps one of the shell's fixed windows"
        );
        i += 1;
    }
    assert!(address_space_map::PAIR_PAGES.holds(JOBFRAME_WINDOWS.start, JOBFRAME_WINDOWS.end));
    assert!(address_space_map::PAIR_PAGES.holds(IMAGE_WINDOW, IMAGE_WINDOW_END));
};

/// Delegate the capability in `slot` to the progenitor over the spawn rendezvous, narrowed to WRITE|GRANT (the progenitor
/// builds from an untyped or maps a frame, and narrows further from there). We keep our own copy.
pub(super) fn send_cap(slot: u64) {
    delegate(slot, abi::rights::WRITE | abi::rights::GRANT);
}

/// **Delegate one capability to the progenitor, with exactly the rights it should travel with.**
///
/// `GRANT` has to be in `rights` for the send itself to be legal, so what varies is the other half,
/// and that half is load-bearing for milestone 50: a pipe's write end goes over as `WRITE|GRANT` and
/// its read end as `READ|GRANT`, so the child at the far end of a `|` **cannot write back up its own
/// input**. A pipeline that granted both directions would be a two-way channel nobody asked for, and
/// the narrowing is what stops it rather than a convention the programs are trusted to keep.
pub(super) fn delegate(slot: u64, rights: u64) {
    user_mode_runtime::send_capability(SPAWN, slot, rights, spawnproto::CAP_TAG);
}
