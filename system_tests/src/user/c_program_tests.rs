//! **Somebody else's C, unmodified, on nife's C library** (milestone 835 (a C library, stage 1:
//! files, clock and memory); `design/fatal-risks/1-only-software-written-for-nife.md`).
//!
//! Risk 1 is *"only software written for nife runs on nife"*. `ripgrep` (milestone 121 (`ripgrep`
//! on nife: enumeration as a capability, and what the walk costs)) answered it once, in Rust,
//! through `std`. These answer it in C, through a C library (§265 (a C library started from
//! relibc, whose Rust platform layer holds the capabilities)) that neither program has heard of:
//!
//! - **SQLite 3.50.4's `speedtest1`**: 266,378 lines of C (the amalgamation and the benchmark)
//!   written for POSIX. `helpers/build-speedtest1.sh` fetches both files, pinned by SHA-256.
//! - **ioping 1.3**: one file, a storage-latency tool, the one milestone 834 (ioping on nife and
//!   Linux) runs on silicon. `helpers/build-ioping.sh` fetches it, pinned the same way.
//!
//! Neither source is touched. What differs from a Linux build is on the command line: the target
//! flags, nife's headers, and for SQLite two compile-time options it documents.
//!
//! **Each test skips when its program is not in the archive**, which is every ordinary build and
//! all of CI, on `ripgrep_tests`' terms: fetching the source in a gate is §46 (thin primitives or
//! whole subsystems; we write everything in between)'s decision and calef's.
//!
//! **All three ISAs**, one body each, nothing architecture-specific asserted (§19 (architectural
//! parity is a tenet; the targets are aarch64, riscv64, and x86_64)).

use super::*;

/// Skip unless this archive has `name` and this boot can run it.
macro_rules! skip_without {
    ($name:literal) => {
        if program($name).is_none() {
            crate::testing::skip!(concat!(
                "no ",
                $name,
                " in this archive: build it with helpers/build-",
                $name,
                ".sh, which fetches its source (milestone 835)"
            ));
        }
        if fs_service::fs_server_image().is_none() {
            crate::testing::skip!(fs_service::NO_FS_SERVER);
        }
        if clock_service::machine_has_no_rtc() {
            crate::testing::skip!(clock_service::NO_RTC);
        }
    };
}

/// **Run the C program `name`, told `line`, holding `c-library` with every right and a clock, and
/// return what it printed.** It must exit on its own without trapping, and everything it held comes
/// back, the clock service included. `None` when this boot has no disk to run it against.
///
/// The clock is a grant because both programs time what they do with `gettimeofday`, which nife's
/// C library answers from `SystemTime`: a process holding no clock is stopped with a message
/// naming the missing grant rather than told it is 1970, as `std` does under §43 (reading the
/// clock is a page, setting it is a page you may write, proposing is an endpoint). It is a held
/// service, started for the run and ended after it, because the suite's frame ledger is at its
/// budget and a clock service otherwise never exits.
fn run(name: &str, line: &str, out: &mut [u8]) -> Option<usize> {
    use core::sync::atomic::Ordering;

    use crate::arch::exceptions::USER_FAULTS;

    let image = program(name)?;
    // The clock service built from one region, so one reclaim ends it and returns its page and
    // endpoints (`clock_service::start_in`'s reasoning).
    let region =
        crate::memory_region::create(clock_service::REGION_PAGES).expect("no region for a clock");
    let (clock, clock_thread) =
        clock_service::start_in(program("clock").expect("no clock in the archive"), region);
    let mut clock_held = crate::user::holding::Holding::new();
    clock_held.add_thread(clock_thread);
    clock_held.add_region(region);
    // Its startup verdict first, so the offset is published before the program reads the page.
    let _ = crate::sched::ipc_receive(clock.report);
    let faults_before = USER_FAULTS.load(Ordering::Relaxed);
    let Some(spawned) = fs_service::start_std_narrowed_clocked(
        program("block_driver").expect("no block_driver program in the initrd archive"),
        program("redoxfs_server").expect("no redoxfs_server program in the initrd archive"),
        program("fs_subtree_caretaker").expect("no fs_subtree_caretaker in the initrd archive"),
        image,
        c_program_fixture::ROOT,
        filesystem_protocol::dir::ALL,
        Some(line.as_bytes()),
        Some(clock.page_phys),
    ) else {
        // No disk: nothing ran, so the clock goes back before the caller skips.
        assert!(
            clock_held.release(),
            "a clock service nothing used outlived its holding"
        );
        return None;
    };
    let len = super::std_tests::drain_sink(spawned.report, out, name);
    assert!(
        super::wait_for(|| !crate::sched::is_thread_present(spawned.thread)),
        "{name} never left: it is neither exited nor faulted",
    );
    assert_eq!(
        USER_FAULTS.load(Ordering::Relaxed),
        faults_before,
        "{name} trapped instead of exiting",
    );
    // Heap, stack, argument page and caretaker, as `ripgrep_tests::run_confined` gives them back:
    // the program is present only when somebody built it, so a charge left on the frame ledger
    // would fail the suite for exactly that person.
    assert!(
        spawned.release(),
        "{name}'s caretaker outlived its holding: a service this test cannot give back",
    );
    assert!(
        clock_held.release(),
        "{name}'s clock service outlived its holding: a service this test cannot give back",
    );
    Some(len)
}

// ===========================================================================================
// SQLite's speedtest1
// ===========================================================================================

/// How many numbered tests `--testset main --size 1` prints: 32, counted in the same program's
/// output on macOS, 2026-10-10 (UTC). A run that stopped early prints fewer.
const SPEEDTEST1_TESTS: usize = 32;

/// The checks both `speedtest1` runs share: SQLite's banner, every numbered test, the total, and the
/// hash of every result matching the one the same program printed on macOS.
fn assert_speedtest1_complete(text: &str, how: &str) {
    assert!(
        text.contains("-- Speedtest1 for SQLite 3.50.4"),
        "{how}: speedtest1 did not print SQLite's own banner",
    );
    let numbered = text
        .lines()
        .filter(|l| {
            let b = l.trim_start().as_bytes();
            b.len() > 6 && b[..3].iter().all(u8::is_ascii_digit) && &b[3..6] == b" - "
        })
        .count();
    assert_eq!(
        numbered, SPEEDTEST1_TESTS,
        "{how}: speedtest1 ran {numbered} of its {SPEEDTEST1_TESTS} tests",
    );
    assert!(
        text.lines().any(|l| l.trim_start().starts_with("TOTAL")),
        "{how}: speedtest1 never printed its total, so it did not finish",
    );
    // The strong claim. `--verify` hashes every value SQLite handed back, so one wrong byte from
    // `strtod`, `printf`, `memcmp` or a short `pread` changes the hash.
    let hash = c_program_fixture::SPEEDTEST1_HASH;
    assert!(
        text.lines().any(|l| l.trim_end() == hash),
        "{how}: speedtest1's results differ from the same program's on macOS (want `{hash}`)",
    );
}

/// **SQLite's engine runs unmodified on nife, in memory**: the whole engine, the allocator
/// (`malloc` on `std`'s heap), `printf` and the clock, and no file. If this fails the fault is in
/// the C library's core rather than its file calls.
///
/// Falsification: attested 2026-10-10. On patagonia, aarch64 under HVF, before the run was granted
/// a clock: `speedtest1` printed its banner, trapped in `SystemTime::now()` on its first timed
/// test, and this test failed. A replay needs `speedtest1` in the archive, which no gate builds.
#[test_case]
fn unmodified_sqlite_speedtest1_runs_in_memory() {
    skip_without!("speedtest1");
    let mut got = [0u8; 16384];
    let line = c_program_fixture::SPEEDTEST1_MEMDB;
    let Some(len) = run("speedtest1", line, &mut got) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    let text = core::str::from_utf8(&got[..len]).unwrap_or("<not utf-8>");
    crate::println!("    speedtest1 --memdb printed {len} bytes:\n{text}");
    assert_speedtest1_complete(text, "--memdb");
}

/// **The same program, on a file, through the C library's file calls**: `open`, `pread`,
/// `pwrite`, `fsync`, `ftruncate` and `unlink`, which the platform layer turns into the file
/// contract `std::fs` speaks. SQLite creates a rollback journal beside the database and deletes it
/// at every commit, so this is also a create-and-remove loop.
///
/// Falsification: attested 2026-10-10. The same red as the in-memory test's, on the same run: the
/// program trapped before its first test with no clock granted. A replay needs `speedtest1` in the
/// archive, which no gate builds.
#[test_case]
fn unmodified_sqlite_speedtest1_runs_on_a_file() {
    skip_without!("speedtest1");
    let mut got = [0u8; 16384];
    let line = c_program_fixture::SPEEDTEST1_FILE;
    let Some(len) = run("speedtest1", line, &mut got) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    let text = core::str::from_utf8(&got[..len]).unwrap_or("<not utf-8>");
    crate::println!("    speedtest1 on a file printed {len} bytes:\n{text}");
    assert_speedtest1_complete(text, "on a file");
}

// ===========================================================================================
// ioping
// ===========================================================================================

/// **ioping runs unmodified on nife**: it parses its options with `getopt_long_only`, installs a
/// `SIGINT` handler, makes its working file with `mkstemp` in the granted directory, fills it,
/// times five reads with `gettimeofday`, prints its statistics with `sqrt` and `printf`, and removes
/// the file. The latencies are QEMU's and are not asserted; the shape of the run is.
///
/// Falsification: attested 2026-10-10. On patagonia, aarch64 under HVF, with `printf` still reading
/// a `double` vararg from the floating-point save area: every size printed as 0 (`0 KiB <<< .`),
/// and this test failed on its first request line. A replay needs `ioping` in the archive, which
/// no gate builds.
#[test_case]
fn unmodified_ioping_times_reads_in_a_granted_directory() {
    skip_without!("ioping");
    let mut got = [0u8; 8192];
    let Some(len) = run("ioping", c_program_fixture::IOPING, &mut got) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    let text = core::str::from_utf8(&got[..len]).unwrap_or("<not utf-8>");
    crate::println!("    ioping printed {len} bytes:\n{text}");
    // One line per request, numbered 1 to 5, the first marked as the warmup ioping does not count.
    for want in [
        "request=1 ",
        "request=2 ",
        "request=3 ",
        "request=4 ",
        "request=5 ",
    ] {
        assert!(
            text.lines()
                .any(|l| l.starts_with("4 KiB <<< . ") && l.contains(want)),
            "ioping printed no line for `{want}`",
        );
    }
    assert!(
        text.lines()
            .any(|l| l.contains("request=1 ") && l.ends_with("(warmup)")),
        "ioping's first request was not its warmup",
    );
    assert!(
        text.contains("ioping statistics ---"),
        "ioping never printed its statistics, so it did not finish",
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("4 requests completed in ")),
        "ioping did not count four requests after the warmup",
    );
    // Every size and time ioping prints goes through `printf("%.*f", ...)` with a `double`, and on
    // nife's soft-float ABI that vararg arrives in a general register. Before the C library read it
    // as one, each printed 0 ("0 KiB read"); four 4 KiB reads are 16 KiB, whatever QEMU's speed.
    assert!(
        text.lines()
            .any(|l| l.starts_with("4 requests completed in ") && l.contains(", 16 KiB read, ")),
        "ioping's statistics did not print 16 KiB read: `printf` lost its `double` argument",
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("generated 5 requests in ")),
        "ioping did not generate the five requests it was told to",
    );
    assert!(
        text.lines().any(|l| l.starts_with("min/avg/max/mdev = ")),
        "ioping printed no latency summary",
    );
}
