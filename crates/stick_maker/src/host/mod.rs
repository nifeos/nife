//! **The half that runs the host's own tools**: `diskutil` on macOS, `sfdisk`, `mkfs.vfat` and
//! `mount` on Linux, four `kernel32` calls on Windows; and the real terminal.
//!
//! Every decision is elsewhere and tested (`crate::disk`, `crate::macos`, `crate::linux`,
//! `crate::windows`, and `crate::cli` against a fake host); this only runs a program or makes a
//! system call and reports what came back. It is outside `script/coverage`'s floor, recorded there
//! too: the unprivileged Linux coverage run has no `diskutil` or `kernel32` and cannot erase a
//! block device as root. Each arm runs on its own host instead: `helpers/stick-maker-proof.sh`
//! (macOS, hdiutil-attached files) and `.github/workflows/stick-maker-hosts.yml` (a Linux loop
//! device through both paths; Windows discovery).

use std::io::{self, BufRead as _, Write as _};
use std::path::{Path, PathBuf};

use crate::cli::{Console, Host};
use crate::disk::Disk;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

mod machine;
mod terminal;

pub use machine::*;
pub use terminal::*;
