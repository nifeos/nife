//! Rust `std` on the native ABI (milestone 27).
//!
//! std's Platform Abstraction Layer for nife lives in patches/std-nife (the Hermit shape:
//! a `sys` backend on the capability ABI, not a libc shim). `std-src` materializes a patched
//! rust-src into a farm linked as this worktree's own toolchain ([`toolchain_name`]);
//! `std-exerciser` builds the `std_exerciser` program for the
//! custom targets with -Zbuild-std against it. See notes/std.md.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::host::{capture, run, workspace_root};

/// The prefix of every worktree's linked toolchain name (`rustup toolchain link`); the rest of the
/// name is [`toolchain_name`]'s. Provisional, minted 2026-10-10 (UTC) by lane
/// maintainer/per-worktree-toolchain; calef has not ruled on it.
///
/// Each worktree links its own name, and none links the bare `nife-dev`: `rustup toolchain link` is
/// one symlink per user account, so a shared name is one that lanes take from each other. The
/// legacy `nife-dev` link is never written, read for a build, or deleted here; removing it is the
/// maintainer's (briefs/merge-and-cleanup.md). See notes/std.md, "Each worktree has its own
/// toolchain name".
const TOOLCHAIN_PREFIX: &str = "nife-dev-";

/// Bump to force every farm to rebuild after a change to the patch logic itself (not the inputs).
const STD_SRC_PATCH_VERSION: u32 = 15;

// The verbs and their helpers, one child each (§282 (when a file is split, the parent becomes an
// index)). Each child takes `use super::*`; the names other modules call keep their `crate::farm::`
// paths through the re-exports below. The constants above are the handful every child reads.
mod aborts;
mod exerciser;
mod generate;
mod patch;
mod programs;
mod source;
mod stamp;
mod toolchain;

pub(crate) use aborts::std_aborts;
use exerciser::STD_TARGETS;
pub(crate) use exerciser::{build_jig, build_mdr, std_exerciser};
use generate::*;
use patch::*;
pub(crate) use programs::*;
pub(crate) use source::std_src;
use source::*;
pub(crate) use stamp::std_inputs_stamp;
use stamp::*;
use toolchain::*;

#[cfg(test)]
mod tests;
