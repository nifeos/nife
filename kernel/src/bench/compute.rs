//! **The compute workload** (milestone 19e (run a real workload)): CoreMark, which measures the
//! core rather than nife.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The compute row: `crates/coremark`, which touches no OS primitive, so its cost is the core's.
///
/// It records nife's compute number on the same instruments as every other row; compute is
/// privilege-independent, so it equals the EL0 workload's. `SINK` keeps the loop live.
pub(super) fn coremark_compute() {
    static SINK: AtomicU64 = AtomicU64::new(0);
    timed("coremark", COREMARK_ITERS, || {
        let crc = coremark::run(COREMARK_ITERS as u32);
        SINK.store(crc as u64, Ordering::Relaxed);
    });
}
