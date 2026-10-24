//! The binaries the service runs, looked up in the boot archive, and the reasons a test
//! gives when one is missing.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The FS server's binary, packed as `redoxfs_server`, or `None` when nothing built it before the
/// archive was packed.
///
/// A test that needs a filesystem skips with [`NO_FS_SERVER`] rather than panicking on the lookup.
/// A wrong archive name would read as `None` everywhere and skip every such test silently. On
/// `x86_64` the server builds with `--cfg aes_force_soft` (`.cargo/config.toml`), since that target
/// has no vector register for `aes` to use.
pub fn fs_server_image() -> Option<&'static [u8]> {
    program("redoxfs_server")
}

/// The reason a test gives when there is no FS service to talk to. One string, because a dozen
/// files share one cause and a reader comparing two runs should not have to decide whether two
/// wordings mean the same thing.
///
/// It names two causes on purpose: most callers reach it from [`fs_server_image`] being `None`,
/// and `rmle_tests` from `rmle_service::start` returning `None` on a machine with no RedoxFS
/// disk.
pub const NO_FS_SERVER: &str = "no RedoxFS service on this boot: either nothing packed a \
                                redoxfs_server into this archive, or this machine has no RedoxFS \
                                disk for one to serve";

/// `mkfs`'s binary, or `None`. Same package as [`fs_server_image`], same
/// vendored engine underneath it, so it is absent for exactly the same reason and on exactly the
/// same targets; a separate accessor only so a skipped test names the program it actually wanted.
pub fn mkfs_image() -> Option<&'static [u8]> {
    program("mkfs")
}

/// The reason a test gives when [`mkfs_image`] is `None`. [`NO_FS_SERVER`]'s cause, one binary over.
pub const NO_MKFS: &str = "no mkfs in this archive (nothing built one for this target before \
                           the archive was packed)";

/// The binary carrying the block server's role, `block_driver` on every architecture.
///
/// It panics rather than returning `None` because a boot archive without it is a build that did
/// not finish, not a machine without a disk; the disk's absence is [`root_directory`]'s `None`.
pub fn blk_server_image() -> &'static [u8] {
    program("block_driver").expect("no block_driver program in the initial ramdisk")
}
