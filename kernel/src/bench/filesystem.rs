//! **The filesystem benchmarks**: a read through the confined stack, throughput, directory
//! walks, and a search.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// A warm file read through the confined stack: a client reads a file through a granted directory
/// capability, over the FS server and the block server, and reports `[ticks, iters]`.
///
/// After the first read the FS server serves it from memory, so the row is the confined IPC path
/// plus a copy; the device path is `fs_seq_read`'s. `--real --smp` only, and never on the icount
/// baseline, because the path is interrupt-driven. It is not a throughput number. See
/// notes/benchmarks/kernel-bench-harness.md.
pub(super) fn fs_read() {
    // Only the `--real --smp` boot attaches the RedoxFS disk.
    if crate::smp::online_count() <= 1 {
        return;
    }
    // Absent any of the three binaries, or the RedoxFS disk, skip.
    let (Some(blk_image), Some(redoxfs_server), Some(fs_test_client)) = (
        crate::trust::require_program("block_driver"),
        crate::trust::require_program("redoxfs_server"),
        crate::trust::require_program("fs_test_client"),
    ) else {
        return;
    };
    // Spawn the block server, the FS server, and the client in its ROLE_BENCH (timed) role.
    let Some((readiness, report)) =
        crate::user::fs_service::start(blk_image, redoxfs_server, fs_test_client, 1)
    else {
        return; // no RedoxFS disk on this run
    };
    // Device up, then mount, then the timed loop, as the test sequences it.
    if let Some((blk_ready, ready)) = readiness {
        // `None` only when the block service was already running; the first bench wires it.
        if let Some(blk_ready) = blk_ready {
            let _ = sched::ipc_receive(blk_ready);
        }
        let _ = sched::ipc_receive(ready);
    }
    let [ticks, iters, ..] = sched::ipc_receive(report);
    println!("bench: fs_read {ticks} {iters}");
}

/// Filesystem throughput through the confined FS server, §34 (RedoxFS is the primary filesystem,
/// on three conditions) condition 2.
///
/// Six phases over one file: sequential write, sequential read, random read, a record-aligned
/// read, random write, and the cost of producing a page of payload, which the write phases pay.
/// Each prints a row in per-request ticks and a MiB/s probe. One transfer is 4096 bytes, the one
/// shared page. `--real --smp` only. See notes/benchmarks/filesystem-throughput.md and
/// notes/benchmarks/kernel-bench-harness.md.
pub(super) fn fs_throughput() {
    // Only the `--real --smp` boot attaches the RedoxFS disk.
    if crate::smp::online_count() <= 1 {
        return;
    }
    let (Some(blk_image), Some(redoxfs_server), Some(fs_test_client)) = (
        crate::trust::require_program("block_driver"),
        crate::trust::require_program("redoxfs_server"),
        crate::trust::require_program("fs_test_client"),
    ) else {
        return;
    };
    // Reuses the service `fs_read` wired, so `readiness` is `None`.
    let Some((readiness, report)) = crate::user::fs_service::start(
        blk_image,
        redoxfs_server,
        fs_test_client,
        filesystem_fixture::throughput::ROLE,
    ) else {
        return; // no RedoxFS disk on this run
    };
    if let Some((blk_ready, ready)) = readiness {
        // `None` only when the block service was already running; the first bench wires it.
        if let Some(blk_ready) = blk_ready {
            let _ = sched::ipc_receive(blk_ready);
        }
        let _ = sched::ipc_receive(ready);
    }
    let hz = crate::arch::timer::frequency();
    for _ in 0..filesystem_fixture::throughput::PHASES {
        let [ticks, transfers, phase, ..] = sched::ipc_receive(report);
        let Some(name) = filesystem_fixture::throughput::name(phase) else {
            println!("bench-probe: fs_throughput unknown phase {phase}");
            continue;
        };
        println!("bench: {name} {ticks} {transfers}");
        // Hundredths of MiB/s. Zero ticks is a broken run, reported as zero rather than divided.
        let bytes = transfers * filesystem_fixture::throughput::UNIT as u64;
        let hundredths = bytes
            .checked_mul(hz)
            .and_then(|v| v.checked_mul(100))
            .and_then(|v| v.checked_div(ticks))
            .map(|v| v >> 20)
            .unwrap_or(0);
        println!(
            "bench-probe: {name}_mib_per_s {}.{:02}",
            hundredths / 100,
            hundredths % 100
        );
    }
}

/// A directory walk through a confined grant: `std_exerciser` walks, with `std::fs`, the tree of
/// milestone 121 (`ripgrep` on nife: enumeration as a capability) behind a caretaker.
///
/// The row is the median warm walk; probes split it by call. `bench/host/run_linux_walk.sh` prices
/// Linux alike. It is the release-kernel figure the debug kernel suite cannot give. `--real --smp`
/// only; skips without the RedoxFS disk or `std_exerciser`.
///
/// # BUGS
/// Warm only: the first walk fills the FS server's memo, as a repeated `rg` or `find` finds it. A
/// cold walk is a different number, with Linux's `drop_caches` as its control.
pub(super) fn fs_walk() {
    if crate::smp::online_count() <= 1 {
        return;
    }
    let (Some(blk), Some(server), Some(caretaker), Some(walker)) = (
        crate::trust::require_program("block_driver"),
        crate::trust::require_program("redoxfs_server"),
        crate::trust::require_program("fs_subtree_caretaker"),
        crate::trust::require_program("std_exerciser"),
    ) else {
        println!("bench-probe: fs_walk skipped (a program is missing from the archive)");
        return;
    };
    use filesystem_contract::dir;
    let Some(spawned) = crate::user::fs_service::start_std_narrowed(
        blk,
        server,
        caretaker,
        walker,
        filesystem_fixture::walk::ROOT,
        dir::ENUMERATE | dir::READ | dir::DESCEND,
        None,
    ) else {
        return; // no RedoxFS disk on this run
    };
    walk_rows(spawned.report, "fs_walk");
}

/// The same walk through a grant the FS server enforces itself, a badge bound to the subtree, so
/// the difference from `fs_walk` is the caretaker's hop. Takes the grant back afterwards.
pub(super) fn fs_walk_bound() {
    if crate::smp::online_count() <= 1 {
        return;
    }
    let (Some(blk), Some(server), Some(walker)) = (
        crate::trust::require_program("block_driver"),
        crate::trust::require_program("redoxfs_server"),
        crate::trust::require_program("std_exerciser"),
    ) else {
        return;
    };
    use filesystem_contract::dir;
    let Some(spawned) = crate::user::fs_service::start_std_bound(
        blk,
        server,
        walker,
        filesystem_fixture::walk::ROOT,
        dir::ENUMERATE | dir::READ | dir::DESCEND,
    ) else {
        return;
    };
    walk_rows(spawned.report, "fs_walk_bound");
    while crate::sched::is_thread_present(spawned.thread) {
        sched::yield_now();
    }
    if !spawned.release() {
        println!("bench-probe: fs_walk_bound could not take its grant back");
    }
}

/// Warm `rg` runs [`rg_search`] times, after one untimed run that fills the FS server's memo. Odd,
/// so the median is a run.
const RG_SEARCH_RUNS: usize = 5;

/// Unmodified `ripgrep` searching the tree [`fs_walk`] prices, with the same grant.
///
/// The row is ripgrep's own `--stats` time, the median of [`RG_SEARCH_RUNS`] warm runs, which
/// leaves out process creation here and in `bench/host/run_linux_rg.sh`; spawn-to-exit is a probe.
/// Skips unless `cargo xtask basalt-fetch` put `rg` in the image (calef, 2026-10-07 UTC).
///
/// # BUGS
/// One process per run: each `rg`'s heap, allocator and regex compilation are cold, and only the
/// FS server is warm, which is what typing `rg` twice gets on both sides.
pub(super) fn rg_search() {
    if crate::smp::online_count() <= 1 {
        return;
    }
    let (Some(blk), Some(server), Some(caretaker), Some(rg)) = (
        crate::trust::require_program("block_driver"),
        crate::trust::require_program("redoxfs_server"),
        crate::trust::require_program("fs_subtree_caretaker"),
        crate::trust::require_program("rg"),
    ) else {
        println!("bench-probe: rg_search skipped (no rg in the archive; cargo xtask basalt-fetch)");
        return;
    };
    use filesystem_contract::dir;
    use filesystem_fixture::walk as tree;
    let hz = crate::arch::timer::frequency();
    let mut searched = [0u64; RG_SEARCH_RUNS];
    let mut whole = [0u64; RG_SEARCH_RUNS];
    for run in 0..=RG_SEARCH_RUNS {
        let started = crate::arch::timer::now();
        let Some(spawned) = crate::user::fs_service::start_std_narrowed(
            blk,
            server,
            caretaker,
            rg,
            tree::ROOT,
            dir::ENUMERATE | dir::READ | dir::DESCEND,
            Some(tree::RG_SEARCH.as_bytes()),
        ) else {
            return; // no RedoxFS disk on this run
        };
        let mut text = [0u8; 8192];
        let len = drain_report(spawned.report, &mut text);
        while crate::sched::is_thread_present(spawned.thread) {
            sched::yield_now();
        }
        let ended = crate::arch::timer::now();
        if !spawned.release() {
            println!("bench-probe: rg_search could not take its grant back");
            return;
        }
        let text = core::str::from_utf8(&text[..len]).unwrap_or("");
        let Some(ns) = rg_seconds(text) else {
            println!("bench-probe: rg_search printed no stats ({len} bytes)");
            return;
        };
        if run == 0 {
            // The counts, once, so a reader can see the search found what the kernel test asserts.
            for line in text
                .lines()
                .filter(|l| l.contains("files searched") || l.contains("matches"))
            {
                println!("bench-probe: rg_search {line}");
            }
            continue;
        }
        searched[run - 1] = ns;
        whole[run - 1] = (ended - started) * 1_000_000_000 / hz;
    }
    searched.sort_unstable();
    whole.sort_unstable();
    let mid = RG_SEARCH_RUNS / 2;
    println!("bench: rg_search {} 1", searched[mid] * hz / 1_000_000_000);
    println!(
        "bench-probe: rg_search {} ns by rg's own clock, {} to {}",
        searched[mid],
        searched[0],
        searched[RG_SEARCH_RUNS - 1]
    );
    println!(
        "bench-probe: rg_search spawn to exit {} ns, {} to {}",
        whole[mid],
        whole[0],
        whole[RG_SEARCH_RUNS - 1]
    );
}

/// `rg --stats`'s last line, `<seconds> seconds` with six decimals, in nanoseconds.
fn rg_seconds(text: &str) -> Option<u64> {
    let line = text.lines().rev().find(|l| l.ends_with(" seconds"))?;
    let (whole, frac) = line.strip_suffix(" seconds")?.split_once('.')?;
    let mut ns = whole.parse::<u64>().ok()? * 1_000_000_000;
    let mut scale = 100_000_000;
    for b in frac.bytes() {
        ns += u64::from(b.checked_sub(b'0')?) * scale;
        scale /= 10;
    }
    Some(ns)
}

/// A program's whole transcript from its stdout sink, as far as `out` holds it.
fn drain_report(report: crate::sched::RendezvousId, out: &mut [u8]) -> usize {
    let mut len = 0usize;
    loop {
        let words = sched::ipc_receive(report);
        let mut chunk = [0u8; byte_sink_contract::INLINE_MAX];
        match byte_sink_contract::unpack(words[0], words[1], words[2], &mut chunk) {
            byte_sink_contract::Msg::Bytes(n) => {
                let take = n.min(out.len() - len);
                out[len..len + take].copy_from_slice(&chunk[..take]);
                len += take;
            }
            byte_sink_contract::Msg::Eof | byte_sink_contract::Msg::Malformed => return len,
        }
    }
}

/// Drain a priced walk's transcript from `report` and print it as the `row` and its probes.
fn walk_rows(report: crate::sched::RendezvousId, row: &str) {
    // The transcript, decoded from the program's stdout sink and kept only as far as this buffer.
    let mut text = [0u8; 2048];
    let mut len = 0usize;
    loop {
        let words = sched::ipc_receive(report);
        let mut chunk = [0u8; byte_sink_contract::INLINE_MAX];
        match byte_sink_contract::unpack(words[0], words[1], words[2], &mut chunk) {
            byte_sink_contract::Msg::Bytes(n) => {
                let take = n.min(text.len() - len);
                text[len..len + take].copy_from_slice(&chunk[..take]);
                len += take;
            }
            byte_sink_contract::Msg::Eof => break,
            byte_sink_contract::Msg::Malformed => {
                println!(
                    "bench-probe: fs_walk malformed sink message {:#x}",
                    words[0]
                );
                return;
            }
        }
    }
    let text = core::str::from_utf8(&text[..len]).unwrap_or("");
    let hz = crate::arch::timer::frequency();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("walk whole ") {
            let ns: u64 = rest
                .split(' ')
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            // The program timed in nanoseconds; the row is in counter ticks like every other.
            println!("bench: {row} {} 1", ns * hz / 1_000_000_000);
        }
        if let Some(rest) = line.strip_prefix("walk ") {
            println!("bench-probe: {row} {rest}");
        }
    }
}
