//! **The FS-server read benchmark** (`ROLE_BENCH`).
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use filesystem_fixture as fixture;
use user_mode_runtime::{exit, now, send};

use crate::channel::{REPORT, open, read};

/// Iterations for the timed read loop (`ROLE_BENCH`), self-timed and reported home like
/// `os_primitives_benchmarker`'s primitives. Warmup pays the `OPEN`; the timed loop rereads one
/// block. `motd` is 69 bytes inline in its node, so each read rides the tree walk that
/// `redoxfs_server::CachedDisk` answers from memory: the report is the FS-server contract's cost
/// above a raw endpoint call, warm. notes/fs-server/throughput.md has the measurement.
const BENCH_ITERS: u64 = 2000;
const BENCH_WARMUP: u64 = 64;

/// **The FS-server read benchmark** (the userspace-server tax, DECISIONS §32). Open `motd` once, then
/// time a loop of reads of its first block and report `[ticks, iters]` on the report endpoint, the
/// `os_primitives_benchmarker` shape. The kernel's bench boot prints it as `fs_read`; against the bare `ipc_rtt_el0`
/// round trip, the difference is what the FS-server contract costs above a raw endpoint call. It is a
/// `--real`-only magnitude (the mount is device-driven, not deterministic under icount); see
/// notes/benchmarks.md and kernel/src/bench.rs.
pub(crate) fn bench() -> ! {
    let handle = open(fixture::MOTD_NAME);
    let block = fixture::MOTD.len();
    for _ in 0..BENCH_WARMUP {
        let _ = read(handle, 0, block);
    }
    let start = now();
    for _ in 0..BENCH_ITERS {
        let _ = read(handle, 0, block);
    }
    let ticks = now().wrapping_sub(start);
    // slot 1 is REPORT; carry ticks and the iteration count, the bench line's two fields.
    send(REPORT, ticks, BENCH_ITERS, 0);
    exit();
}
