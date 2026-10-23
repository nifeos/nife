#![no_std]
// `Result<_, ()>` throughout the build path, for `supervision_protocol`'s reason: every failure here is
// a syscall that already returned its own error through the ABI, and a second, richer error would be
// inventing detail the kernel did not provide. See that crate's head comment.
#![allow(clippy::result_unit_err)]
//! **The interactive system, built once**: everything the progenitor does, from the table of slot
//! numbers its kernel granted ([`BootEndowment`]) to serving the shell's last `run`.
//! `components/src/progenitor.rs`, the first process on every architecture, calls [`boot`]. No
//! host test can run this crate (it reaches `user_mode_runtime`'s EL0 `asm!`); `script/swish-check`
//! proves it by booting and typing at the prompt.
//!
//! It grants nothing of its own to what it cannot vouch for (§219 (how the shell names an installed
//! program to the spawner)). What [`boot`] builds, the measured load, the spawn service and the
//! full account of each BUGS entry are in notes/system-initializer.md.

//! # BUGS
//! - A refused `console` or `line_editor`, or an absent core component, traps silently.
//! - Measurement is checked once, at load; nothing re-measures a running process.
//! - Pages return LIFO: a reclaimed job region below a live one is a hole until that job ends.
//! - A `graphical_terminal` session crowds the job pool, `^C` cannot end it, and it may strand pages.
//! - A kernel that grants no slot 28 leaves every scratch window mapped.
//! - A boot with a NIC waits for DHCP before the prompt, with no console to say why.
//! - Exhausting the capability table inside `build_child` halts in silence.
//! - A screen-narrowed child is outside `deaths`'s domain, so a concurrent `ps` does not show it.
//! - Native slot 1 is the source or the `--mem` budget, safe only while no manifest has both.
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
pub use endowment::{BootEndowment, MountedTreeGrant};
use fault::*;
use filesystem_contract::{dir, fs as fs_operation};
use grant_plan::job_windows::{ReapsDue, Windows};
use grant_plan::{Program, spawnproto};
use image_request::*;
use layout::*;
pub use layout::{
    CHILD_STACK_PAGES, GRANT_MACHINE_PAGE, IMAGE_POOL_PAGES, JOBS_BUDGET_PAGES,
    PROGENITOR_RESERVED_PAGES,
};
use line_editor::proto;
use measurement::*;
use spawn_loop::*;
use std_layout::{NO_RESOLVERS, Resolvers, StdGrants, StdLayout};
// The tree's only loader, named here rather than qualified at every call.
use supervision_protocol::{
    ChildEndowment, Retention, build_child, forget_thread_control_block, hand_to_requester,
    retype_obj_from as retype_obj, retype_page_frame_from as retype_page_frame, start_child,
};
use user_mode_runtime::{
    address_space_map_into, call, cap_delete, is_granted, map_page_frame, receive, receive_badged,
    receive_cap_badged, retype_into_kernel_object, retype_into_page_frame, send, send_capability,
    slice_page_frame,
};
