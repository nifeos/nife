//! **The system log, as logic** (§242 (a system log)): one service that programs append to and
//! readers read from. This crate is all of it except the syscalls, which
//! `components/src/system_log.rs` wraps around [`Log`]. The service stamps each line from the
//! writer's badge ([`registry`]), joins each writer's chunks into lines ([`assembler`]), renders
//! them as JSONL ([`json`]) and keeps them in a 64 KiB ring read by cursor ([`ring`]). See
//! notes/system-log.md.

//! # BUGS
//!
//! - A dropped-count line counts every user's records, leaking how busy the machine was.
//! - A line is stamped when it completes, not when its first byte arrived.
//! - A ninth writer mid-line cuts the least recent writer's line into two records, `"cut":true`.
//! - No writer has a quota, and a writer picks its own severity, as `syslog(3)` allows.
//! - The intake endpoint must receive nothing the kernel writes, because a kernel delivery reads
//!   as the spawner's badge 0; notes/system-log.md, "The intake hears only programs".
//!
//! Name: ratified 2026-10-06 (calef): §242's noun, shared with the program. `starlog` was deferred,
//! not refused, for a reader command that does not exist yet.

#![no_std]

pub mod assembler;
pub mod json;
pub mod registry;
pub mod ring;

use assembler::{Assembler, Line};
use byte_sink_contract::{INLINE_MAX, Msg, unpack};
use registry::{Name, Registry};
use ring::Ring;
pub use system_log_contract::record;
use system_log_contract::record::Header;
use system_log_contract::{control, read, severity};

#[cfg(test)]
mod tests;

mod log;
mod store;

pub use log::{Handled, Log};
use store::*;
