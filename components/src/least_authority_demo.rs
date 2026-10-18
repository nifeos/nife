//! The least-authority demo: a whole program in one job.
//!
//! Milestone 19f.2, the first program that is its **own binary** rather than a role of `hello`.
//! The progenitor loads it out of the initial ramdisk archive by the name `"least_authority_demo"`
//! (nifefs), builds a child address space and TCB at this ELF's own entry, and `START`s it with the
//! input `n` in `x1`. It squares `n`, `SEND`s the answer on the one endpoint the progenitor granted
//! it (slot 0), and exits. That is the entire program: no role byte to dispatch on, no capabilities
//! beyond the single one it needs. Least authority made real, because the program *is* its
//! authority, and the squaring is arbitrary.
//!
//! It shares `components`' linker script (`crates/user_mode_runtime/link.ld`, linked at `address_space_map::IMAGE_BASE`, in its
//! own address space, so the shared load address is not a conflict) but not one line of hello's
//! code: a distinct ELF with its own `_start` and panic handler. The syscall runtime
//! (`send`/`exit`) comes from the shared `user_mode_runtime` crate, lifted out at 19f.6 once all the split
//! binaries existed.
//!
//! Name: ratified 2026-09-13 (calef, working the unratified worklist). Refused `worker` (a generic
//! word of the kind AGENTS.md flags beside `compose` and `measure`: in a kernel tree it could name
//! a thread-pool member, a scheduler entity or a job, and this tree spends the English word on all
//! three), `demo_square` (the header above says the squaring is arbitrary and the authority is the
//! point, so naming it after the arithmetic drops the point; `demo` is also a generic word with no
//! precedent here, and `square` is a verb where the noun rule wants a thing), `squarer` (a noun and
//! honest, but it is the fixture reading this ruling rejected), `least_authority` (the same name one
//! word shorter; calef took the longer form, which says it is a demonstration rather than a claim
//! about the program's own authority). The ruling also settled a classification three records had
//! made by repetition rather than by ruling (`design/naming.md`, milestones 39 and 175 all listed it
//! as a fixture): it is the canonical minimal program, so milestone 175 moved it into `components/`
//! rather than `fixtures/`, and performed the rename in the same change.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, send};

/// The endpoint the progenitor grants the `least_authority_demo` as its only capability (slot 0). Its one `SEND` goes here,
/// straight to whoever is waiting (the kernel test, or the shell behind the progenitor's spawn service).
const RESULT: u64 = 0;

// **Its manifest, carried in its own bytes** (milestone 597 (a program carries its manifest in an
// ELF note), provisional). Run by name, the shell and the progenitor read
// `Prog::LeastAuthorityDemo`'s compiled-in manifest and never this note. Run as a file
// (`script/swish-check` stages a copy nobody vouched for), the note is the only manifest there is,
// and it asks for an argument: which unvouched bytes may not be given, so that line is refused.
capability_manifest_note::carry!(grant_plan::Prog::LeastAuthorityDemo.manifest());

/// The `least_authority_demo`'s entry. `START` (milestone 19e) hands it three registers: `x0` is unused here (a
/// standalone binary needs no role selector), the input `n` arrives in `x1`, `x2` is unused.
#[unsafe(no_mangle)]
pub extern "C" fn _start(_x0: u64, n: u64, _x2: u64) -> ! {
    let result = n.wrapping_mul(n);
    send(RESULT, result, 0, 0);
    // Exit rather than spin: this is a whole process lifecycle (spawn, run, report, exit). The
    // kernel reaps the thread and frees the address space.
    exit();
}

user_mode_runtime::panic_handler!();
