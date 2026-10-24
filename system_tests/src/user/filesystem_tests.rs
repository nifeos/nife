//! `filesystem_tests`: the storage path from a userspace driver up: the virtio disk, `std::fs`
//! through a granted directory, the redoxfs server, and per-file grants under attack.
//!
//! A test with a RISC-V twin in `riscv_virtio_tests` is gated to aarch64 here rather than run
//! twice. The twin drives the same property through the same `block_driver` and `net_stack`
//! binaries, so the two legs differ only by ISA, and a second copy would double the suite's
//! slowest tests to prove nothing new.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

#[cfg(target_arch = "aarch64")]
use core::sync::atomic::Ordering;

// The std-transcript and FS-readiness assertions live in `std_tests` so both ISAs share one copy.
// The three the `std::fs` test needs are wider because that test also runs on `x86_64`.
#[cfg(target_arch = "aarch64")]
use super::std_tests::{assert_a_kill_mid_transaction_recovers, assert_attrs};
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
use super::std_tests::{assert_fs_service_ready, assert_std_transcript, std_fs_expected};
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
use super::test_programs::blk_image;
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
use super::*;
#[cfg(target_arch = "aarch64")]
use crate::sched;

/// A userspace driver reads a file off a real virtio disk (milestone 9, end to end).
///
/// The driver at EL0 gets the device registers, a DMA page and an interrupt, reads `motd` off
/// the nifefs image, and reports its first bytes: real disk data crossed DMA and the EL0 boundary.
/// `ROUTED_IRQS` must rise, so the interrupt path (9a) carried the completion, and the scheduler
/// idles rather than declaring a deadlock while the driver waits with nothing else to run.
// RISC-V twin: `riscv_virtio_tests::a_userspace_driver_reads_a_file_from_a_virtio_disk`; gated to aarch64, see the module doc.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_userspace_driver_reads_a_file_from_a_virtio_disk() {
    use crate::arch::exceptions::ROUTED_IRQS;

    // Sampled before the device is wired, not after. The pcie test further down this file
    // carries the argument for why that ordering, and not a longer wait, is the whole fix.
    let irqs_before = ROUTED_IRQS.load(Ordering::Relaxed);

    let Some(report) = virtio_service::start(blk_image()) else {
        // No disk attached to this run. Nothing to test; do not fail.
        crate::testing::skip!("no virtio disk attached");
    };

    // Blocks until the driver has done the whole read. If the driver faults, it never sends,
    // and the scheduler idles; the QEMU-level timeout is the backstop.
    let word = sched::ipc_receive(report)[0];

    assert_eq!(
        &word.to_le_bytes(),
        b"nife: re",
        "the driver reported the wrong file contents",
    );
    let irqs_after = ROUTED_IRQS.load(Ordering::Relaxed);
    assert!(
        irqs_after > irqs_before,
        "the read completed but no device interrupt was delivered as a message \
         ({irqs_before} routed before the device was wired, {irqs_after} after the report)",
    );
}

/// `std::fs` end to end over the FS-service contract (milestone 27 phase two, the FS half).
///
/// A Rust program granted one directory capability opens, reads and stats the RedoxFS image's
/// file and prints its bytes, so the assertion covers disk, block server, FS server, file
/// contract, std's PAL and stdout. Unlike the hand-written client's test, it shows the mapping to
/// a granted directory holds inside std, refusing `..`, an absolute path and a nested path.
/// Without slot 4 the same binary gets `Unsupported` (the offline std test).
// RISC-V twin: `riscv_virtio_tests::std_fs_reads_a_file_through_a_granted_directory_capability`.
// It also runs on `x86_64` (milestone 303), where it is the only test that writes `scratch`:
// `xtask::redoxfs_check_after_run` reopens that image after the last leg, and without this write
// it would check a regenerated placeholder.
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[test_case]
fn std_fs_reads_a_file_through_a_granted_directory_capability() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    if std_service::std_exerciser_image().is_none() {
        crate::testing::skip!(std_service::NO_STD_EXERCISER);
    }
    let Some((readiness, report)) = fs_service::start_std(
        blk_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("std_exerciser").expect("no std_exerciser program in the initial ramdisk"),
    ) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_fs_service_ready(readiness);

    let mut want = [0u8; 768];
    let n = std_fs_expected(&mut want);
    assert_std_transcript(report, &want[..n], "std fs");
}

/// The RedoxFS filesystem service, end to end (milestone 32 phase 2). See notes/fs-server.md.
///
/// A block server, an FS server and a client, all confined; the client opens `motd` through a
/// granted directory capability and round-trips a pattern through `scratch`. It names nothing but
/// its directory rendezvous, so success is the capability contract holding: designation is
/// authorization, the handle is a server-minted token, and an engine we did not write runs confined.
// RISC-V twin: `riscv_virtio_tests::the_redoxfs_server_serves_redoxfs_over_a_capability_contract`; gated to aarch64, see the module doc.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn the_redoxfs_server_serves_redoxfs_over_a_capability_contract() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some((readiness, report)) = fs_service::start(
        blk_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_test_client").expect("no fs_test_client program in the initial ramdisk"),
        0, // the end-to-end proof role, not the benchmark loop
    ) else {
        // No RedoxFS disk attached to this run. Nothing to test; do not fail.
        crate::testing::skip!("no RedoxFS disk attached");
    };

    // The two servers' readiness sentinels, if this test is the one that wired them (the
    // `std::fs` test shares the same service, and each sentinel is sent exactly once).
    assert_fs_service_ready(readiness);

    // Then: the client has read motd, round-tripped scratch, exercised extended attributes, and
    // reported. If any of the three processes faults, it never sends and the QEMU-level timeout
    // is the backstop.
    let [head, status, attrs, ..] = sched::ipc_receive(report);
    assert_eq!(
        status,
        filesystem_fixture::SUCCESS,
        "the client did not report success: a check in the read or write path failed",
    );
    assert_eq!(
        &head.to_le_bytes()[..],
        &filesystem_fixture::MOTD[..8],
        "the client read the wrong motd bytes off the RedoxFS image",
    );
    assert_attrs(attrs);
}

/// A read-only per-file grant survives an attacker (milestone 31 phase 2, notes/grant-expression.md).
///
/// `run wc report.txt` must hand over one file, not its directory. An `fs_file_caretaker` holds
/// the directory; `fs_test_client`'s attacker role holds a read-only grant of `motd` and attacks
/// `scratch`, a real file one entry away that the caretaker could open. A caretaker refusing
/// everything would pass alone, which is why the writable twin below exists and why the verdict is
/// a bitmap rather than a boolean.
// RISC-V twin: `riscv_virtio_tests::a_read_only_per_file_grant_survives_an_attacker`; gated to aarch64, see the module doc.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_read_only_per_file_grant_survives_an_attacker() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(verdict) = attack_a_grant(filesystem_contract::grant::READ, false) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_eq!(
        verdict,
        0,
        "the read-only per-file grant leaked: {}",
        describe_escape(verdict),
    );
}

/// **A read-only per-file grant carries its file's attributes, and cannot write them**
/// (milestone 61).
///
/// The same run as the test above, read a second way. `GRANTED_ATTRS_FAILED` clear means the
/// listing and the get went through the caretaker to the store; `WROTE_ATTR` clear means the set
/// did not, because a read-only grant must not forward one. Its own assertion, because "the
/// verdict was zero" does not say which properties were in it. The writable twin below keeps it
/// from passing vacuously.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_read_only_per_file_grant_reads_its_files_attributes_and_writes_none() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    use filesystem_fixture::escape;
    let Some(verdict) = attack_a_grant(filesystem_contract::grant::READ, false) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_eq!(
        verdict & escape::GRANTED_ATTRS_FAILED,
        0,
        "the caretaker did not forward the attribute reads, so a program behind a per-file \
         grant still cannot reach its own file's attributes",
    );
    assert_eq!(
        verdict & escape::WROTE_ATTR,
        0,
        "a read-only grant forwarded SETXATTR: an attribute is a way to change a file",
    );
}

/// A writable per-file grant writes that file and still only that file (the read-only test's
/// control).
///
/// Same caretaker, attacker and neighbouring file; only the direction changes. A writable file
/// capability writes and still reaches only its one file. Because the same requests through the
/// same code succeed here, the read-only test's refusals are a narrowed capability, not a
/// caretaker that refuses everything.
// RISC-V twin: `riscv_virtio_tests::a_writable_per_file_grant_writes_that_file_and_still_only_that_file`; gated to aarch64, see the module doc.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_writable_per_file_grant_writes_that_file_and_still_only_that_file() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    use filesystem_fixture::escape;
    let Some(verdict) = attack_a_grant(
        filesystem_contract::grant::READ | filesystem_contract::grant::WRITE,
        true,
    ) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    // `WROTE_ATTR` is the third way to change a file, after bytes and length; a direction check
    // that covered only the first two would leave one open.
    let expected = escape::WROTE | escape::TRUNCATED | escape::WROTE_ATTR;
    assert_eq!(
        verdict,
        expected,
        "a writable grant must write, truncate and set an attribute on its own file and do \
         nothing else: {}",
        describe_escape(verdict & !expected),
    );
}

/// Wire a per-file grant of the given direction, run the attacker against it, and return its
/// verdict bitmap. `None` when no RedoxFS disk is attached (nothing to test; do not fail).
#[cfg(target_arch = "aarch64")]
fn attack_a_grant(rights: u64, writable: bool) -> Option<u64> {
    let Some((report, held)) = fs_service::start_granted(
        blk_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_file_caretaker").expect("no fs_file_caretaker program in the initial ramdisk"),
        program("fs_test_client").expect("no fs_test_client program in the initial ramdisk"),
        fs_service::Grant {
            // The writable run damages what it is granted, so it is granted the file the fixture
            // discipline already covers; see the attacker's own note.
            name: if writable {
                filesystem_fixture::SCRATCH_NAME
            } else {
                filesystem_fixture::MOTD_NAME
            },
            rights,
            role: 2, // ROLE_ATTACKER
            arg: writable as u64,
        },
    ) else {
        // No print, no skip!() here: this is a helper, and `skip!()` returns from the function it
        // is written in, which would leave the test running. `None` is the fixture's absence
        // travelling to the `#[test_case]`, which is the only place that can honestly skip.
        return None;
    };
    // The two handshakes happened inside `start_granted`, before this attacker existed: they
    // are what makes the caretaker's own staged request safe on a page all three share.
    let [tag, verdict, ..] = sched::ipc_receive(report);
    held.release_or_fail("a per-file grant and its attacker");
    assert_eq!(
        tag,
        filesystem_fixture::VERDICT,
        "the attacker's report is not a verdict word",
    );
    Some(verdict)
}

/// Name the bits an escape verdict set, so a failure reads as a sentence instead of a bitmap.
#[cfg(target_arch = "aarch64")]
fn describe_escape(v: u64) -> &'static str {
    use filesystem_fixture::escape;
    if v & escape::SECOND_FILE != 0 {
        "it opened a file the grant does not designate"
    } else if v & escape::WROTE != 0 {
        "it wrote through a read-only grant"
    } else if v & escape::TRUNCATED != 0 {
        "it truncated through a read-only grant"
    } else if v & escape::CREATED != 0 {
        "it created a file through a file capability"
    } else if v & escape::FORGED_HANDLE != 0 {
        "it reached a file with a handle it was never given"
    } else if v & escape::GRANTED_READ_FAILED != 0 {
        "the granted read itself failed, so nothing above was actually proven"
    } else if v & escape::WROTE_ATTR != 0 {
        "it set an extended attribute through a read-only grant"
    } else if v & escape::GRANTED_ATTRS_FAILED != 0 {
        "it could not reach its own file's attributes, which milestone 61 says it carries"
    } else {
        "nothing (an empty verdict should not have failed an assertion)"
    }
}

/// A kill mid-transaction, on the real device (milestone 37, DECISIONS §34 condition 1).
/// The host sweep proves the property over every fault point against a reconstructed platter;
/// this proves it once through the whole stack, with a real virtio write torn in half, a real
/// FS-server process killed inside its own transaction, and a real second process recovering the
/// disk it left behind. See `std_tests::assert_a_kill_mid_transaction_recovers`.
// RISC-V twin: `riscv_virtio_tests::a_kill_mid_transaction_leaves_the_filesystem_consistent`; gated to aarch64, see the module doc.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_kill_mid_transaction_leaves_the_filesystem_consistent() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    if !fs_service::is_crash_disk_present() {
        crate::testing::skip!("no crash disk attached");
    }
    assert_a_kill_mid_transaction_recovers(
        blk_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_test_client").expect("no fs_test_client program in the initial ramdisk"),
    );
}

/// The FS server's stack is sized by measurement, and this is the measurement.
///
/// It runs after both FS clients, so the poison in the server's stack pages has been overwritten
/// to the depth RedoxFS reached across a mount, reads, writes, a create and two truncates. It
/// prints the number and fails if less than a quarter of the grant is left. An overflow kills the
/// server mid-request and leaves its client blocked, which only the per-test wall-clock ceiling
/// notices. See notes/fs-server.md and notes/fs-server/stack-sizing.md.
// RISC-V twin: `riscv_virtio_tests::the_redoxfs_servers_stack_still_has_headroom`; gated to aarch64, see the module doc.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn the_redoxfs_servers_stack_still_has_headroom() {
    let Some((used, total)) = fs_service::fs_stack_used() else {
        crate::testing::skip!("no FS service wired this boot");
    };
    crate::println!("    (FS server stack high-water: {used} of {total} bytes) ");
    assert!(
        used * 4 <= total * 3,
        "the FS server used {used} of {total} stack bytes: under a quarter left. RedoxFS \
         recurses in 8 KiB frames, so the next verb that deepens a tree walk will overflow and \
         the server will die mid-request. Raise FS_STACK_PAGES.",
    );
}
