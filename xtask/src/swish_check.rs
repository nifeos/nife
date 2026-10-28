//! The `cargo xtask swish-check` command: boot the shell, type at it, and check what it
//! answered.
//!
//! The script is a table of (command, expected substrings), so adding a line to the check is
//! adding a row rather than writing a test.

mod boot;
mod gauge;
mod launch;
mod run;
mod script;
mod transcript;
mod verdict;
use std::process::Command;

use boot::*;
use gauge::*;
use launch::*;
use run::*;
/// The verb `main.rs` dispatches to, from [`run`].
pub(crate) use run::{probe_program_feature, swish_check};
use script::*;
use transcript::*;
use verdict::*;

use crate::archive::{has_initial_ramdisk_riscv, initial_ramdisk_path, riscv_initial_ramdisk_path};
use crate::disk::{SWISH_CHECK_DISK_MIB, disk_path, mkdisk, mkredoxfs_of, redoxfs_server_build};
use crate::host::{flag_value, workspace_root};
use crate::scanout::{gpu_mon_socket, screendump, sendkey};
use crate::suite::ArchLegs;
use crate::uefi::{esp_dir, uefi_image, uefi_image_with};
use crate::{RISCV_TARGET, RUNNER, TARGET, X86_TARGET, profile_dir, user};
