#![no_std]
// `Result<_, ()>` throughout the build path, for `supervision_protocol`'s reason: every failure here is
// a syscall that already returned its own error through the ABI, and a second, richer error would be
// inventing detail the kernel did not provide. See that crate's head comment.
#![allow(clippy::result_unit_err)]
//! **The interactive system, built once.** `components/src/progenitor.rs` is the one first process
//! on every architecture, and all it holds is the table of slot numbers its kernel granted
//! ([`BootEndowment`]); everything from parsing the archive to serving the shell's last `run` is
//! here. Nothing host-side can run this crate (it reaches `user_mode_runtime`'s EL0 `asm!`);
//! `script/swish-check`, which boots and types at the prompt, is what proves it.
//!
//! From the endowment alone [`boot`] builds, in its own budget and wired with endpoints and pages
//! it creates: the console (UART, plus the firmware screen's terminal when granted), the input
//! driver, the line discipline, the shell, the terminal's sink adapter when the archive has one,
//! and `job_undertaker`. GPU and virtio keyboard grants ride out to the shell untouched; graphics
//! is launched from the prompt, never at boot. Then it stays alive as the spawn service. It carves
//! [`INIT_OWN_PAGES`] and a renewable [`JOBS_BUDGET_PAGES`] off the construction root and deletes
//! the root, gives up the UART capabilities once the drivers hold them, and keeps the file service.
//! The negative control after the delete must see `NoSuchSlot`, not `NotPermitted`.
//!
//! **It grants nothing of its own to what it cannot vouch for**, per §219 (how the shell names an
//! installed program to the spawner). Every archive program is looked up in
//! [`measured_boot::PROGRAM_MEASUREMENTS`], which the kernel's trust root vouches for; a mismatch
//! or an unlisted name is a refusal, treated as a missing entry. A refused core component (console,
//! input, `line_editor`, swish, `job_undertaker`) traps, a refused sink adapter costs that feature,
//! and a refused spawnable program is left out of `spawn_service`'s table. Logging a mismatch and
//! loading anyway is refused: an advisory link is not a chain.
//!
//! # BUGS
//!
//! - A refused `console` or `line_editor`, or an absent core component, traps silently.
//! - Measurement is checked once, at load; nothing re-measures a running process.
//! - Pages return LIFO: a reclaimed job region below a live one is a hole until that job ends.
//! - A `graphical_terminal` session takes [`GRAPHICAL_TERMINAL_SESSION_PAGES`] of the job pool, so
//!   while one runs a `std` job or a second session gets `could not spawn`. `^C` cannot end a
//!   session, and one whose build fails after its first driver started may keep those drivers'
//!   pages until the boot ends.
//! - Scratch windows are unmapped through slot 28 once a child holds them; a kernel that
//!   grants no slot 28 leaves them all mapped.
//! - A boot with a NIC waits for DHCP before the prompt, with no console to say why.
//! - The capability table is finite and exhausting it inside `build_child` halts in silence.
//!   `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` records the peak and the headroom rule; the
//!   buy-backs for another permanent capability are in `notes/shared-page-audit.md`.
//!
//! Name: ratified 2026-08-04 (calef). `BootEndowment` ratified the same day.

mod activation;
mod announcement;
mod boot_sequence;
mod boot_steps;
mod children;
mod client_windows;
mod endowment;
mod fault;
mod image_request;
mod layout;
mod measurement;
mod spawn_loop;
mod std_layout;

use core::sync::atomic::AtomicU16;
use core::sync::atomic::Ordering::Relaxed;

use activation::{Activating, activate, copy_text, package_manager_parts, serve_installer};
use announcement::*;
pub use boot_sequence::boot;
use children::*;
use client_windows::*;
pub use endowment::{BootEndowment, SecondDirGrant};
use fault::*;
use filesystem_contract::{dir, fs as fs_operation};
use grant_plan::job_windows::{ReapsDue, Windows};
use grant_plan::{Program, spawnproto};
use image_request::*;
use layout::*;
pub use layout::{
    CHILD_STACK_PAGES, GRANT_MACHINE_PAGE, IMAGE_POOL_PAGES, INIT_OWN_PAGES, JOBS_BUDGET_PAGES,
};
use line_editor::proto;
use measurement::*;
use spawn_loop::*;
use std_layout::{StdGrants, StdLayout};
// The loader, and the tree's only one since milestone 96. Named here rather than qualified at every
// call, because the point of the crate is that there is one of these.
use supervision_protocol::{
    ChildEndowment, Retention, build_child, retype_obj_from as retype_obj,
    retype_page_frame_from as retype_page_frame, start_child,
};
use user_mode_runtime::{
    call, cap_delete, invoke, is_granted, receive, receive_badged, receive_cap_badged, send,
    send_capability,
};
