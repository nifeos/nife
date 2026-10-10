//! **Where the unmodified C programs run in the test image, and what they are told**
//! (milestone 835 (a C library, stage 1: files, clock and memory)).
//!
//! A directory a C program may write in: empty in the image, a sibling of everything else at its
//! root, and granted alone with every right, so SQLite's `speedtest1` can create its database and
//! journal there and remove the journal, and what it writes cannot be confused with another test's
//! files. The image builder (`xtask`'s `disk.rs`) makes it and `system_tests`' `c_program_tests`
//! grants it, so its name is a crate rather than two string literals (rule 7). The command lines
//! and the expected hash live beside it because they describe the same runs.
//!
//! It is its own crate because `filesystem_protocol`, where the image's other fixtures live, is at
//! its §266 (a Rust source file stays under 2,000 lines) ceiling and is copied whole into
//! `std` by the farm, so it can neither grow nor split.
//!
//! Name: provisional, milestone 835's lane, 2026-10-10 (UTC). calef ruled on #1896 that this lane's
//! names stay provisional.

#![no_std]

/// The directory, one component under the image root.
pub const ROOT: &str = "c-library";
/// `speedtest1` against an in-memory database: SQLite's engine, the allocator, `printf`
/// and the clock, with no file opened. `--testset main` is the default set without the
/// R-Tree one, which needs an optional SQLite module; `--size 1` is a hundredth of the
/// default, which keeps the TCG legs to seconds and leaves the test list unchanged; and
/// `--verify` makes it hash every result it reads back and print the hash.
pub const SPEEDTEST1_MEMDB: &str = "speedtest1 --memdb --testset main --size 1 --verify";
/// The same run on a file in [`ROOT`], through the C library's `open`, `pread`, `pwrite`,
/// `fsync`, `ftruncate` and `unlink`. `unix-none` is the VFS SQLite ships for a platform with
/// no file locks, which nife's file contract does not have (`c_library/README.md`).
pub const SPEEDTEST1_FILE: &str =
    "speedtest1 --vfs unix-none --testset main --size 1 --verify speedtest1.db";
/// What both runs must print as their last line: the hash of every result, as SQLite
/// 3.50.4's `speedtest1` printed it on macOS (aarch64, hardware float) for both lines
/// above, read 2026-10-10 (UTC). The same hash on nife is the same answers to every query.
pub const SPEEDTEST1_HASH: &str =
    "Verification Hash: 111130 1e792c9db61996c477b8ab5ce2d690052e8dae74824a430a";
/// ioping against [`ROOT`]: five 4 KiB reads (the first a warmup it does not count), no
/// pause between them (`-i 0`), from a 64 KiB working file it creates in the directory
/// with `mkstemp` and removes when it is done.
pub const IOPING: &str = "ioping -c 5 -i 0 -s 4k -S 64k .";
