//! `memory_grant_depleter`: a program that spends exactly the memory it was granted, and reports
//! how much (milestone 31 (a capability shell: designation is authorization), phase 1).
//!
//! It makes `memory_grant_depleter --mem N` real. The shell splits `N` pages off its own untyped
//! budget and delegates them (init inserts the untyped at slot 1); this program maps pages out of
//! it one at a time until it is exhausted and reports the count on slot 0. With no grant slot 1 is
//! empty, the first `MAP` returns `NoSuchSlot`, and it reports zero: there is no ambient pool. The
//! count is `N` minus the page-table pages, which come from the same untyped (the kernel allocates
//! nothing on a process's behalf, DECISIONS §10 (process model: capability-based, microkernel)).
//! See notes/grant-expression/memory-grant.md.

//! # What this program holds
//!
//! - slot 0: the result endpoint (SEND: report the page count).
//! - slot 1: an untyped budget (the delegated `--mem` grant). Absent when `--mem` was 0.
//!
//! Name: ratified 2026-09-13 (calef, a naming-ratification session), replacing a provisional name
//! that had stood since 2026-07-28. Refused `budgeter` (the incumbent: it reads as a program that
//! *manages* a budget, where this one exists to exhaust one, and the bare stem names neither which
//! budget nor that depletion is the point), `budget_spender` and `grant_spender` (both name the
//! object, but spending is what any program does with a grant; depletion is the specific claim,
//! that this one runs the grant to zero on purpose as the proof the budget is enforced rather than
//! advisory), `memory_grant_depleater` (calef's own spelling in the ratifying session, corrected
//! in it: "depleater" is not a word, and the agent noun from "deplete" is `depleter`).
//! The correction is recorded rather than quietly applied because the transcript survives the
//! session: a reader who later meets his `depleater` should be able to see why the tree spells it
//! otherwise, which is what AGENTS.md means by correcting yourself loudly.
//! **What the rename costs, stated because a reader should meet it.** Milestone 63 cites `budgeter`
//! as a member of the agent-noun family it settles (`broker`, `spawner`, `painter`, `budgeter`,
//! `compositor`, `credentialer`), using this stem as evidence the family exists. The block is BUILT
//! and keeps the word it was written with; `memory_grant_depleter` is still an agent noun, so the
//! family survives, but this member leaves that list and 63 carries a line saying so.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, map_memory_region_page, send};

/// The result endpoint init grants every spawned program (slot 0).
const RESULT: u64 = 0;
/// The delegated `--mem` untyped (slot 1). Absent (empty slot) when no budget was granted.
const BUDGET: u64 = 1;

const PAGE: u64 = 4096;
/// Where we start mapping: the bottom of the address-space map's runtime windows, a window that
/// grows upward with nothing else of this program's in it.
const SPEND_BASE: u64 = address_space_map::runtime_window(0x1000_0000);

#[unsafe(no_mangle)]
pub extern "C" fn _start(_x0: u64, _x1: u64, _x2: u64) -> ! {
    let mut mapped: u64 = 0;
    let mut va = SPEND_BASE;
    loop {
        // Map one more page out of the granted untyped at `va`, writable. The page and any page
        // table it needs both come from the budget; when the budget is spent the kernel returns a
        // negative error (OutOfMemory), and a program with no budget at all gets NoSuchSlot on the
        // first call. Either way, we stop and report what we got.
        let r = map_memory_region_page(BUDGET, va);
        if r < 0 {
            break;
        }
        // Touch the page so the mapping is not just nominal: a byte written proves it is real,
        // writable memory the process now owns.
        // SAFETY: we just mapped `va` read/write in our own address space.
        unsafe { core::ptr::write_volatile(va as *mut u8, 0xA5) };
        mapped += 1;
        va += PAGE;
    }
    send(RESULT, mapped, 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
