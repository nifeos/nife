//! **`std_rayon`: unmodified `rayon` on nife** (milestone 812 (`std::thread::spawn` runs real
//! threads in one address space)'s exit test, its second check).
//!
//! The global pool sizes itself from `available_parallelism`. A parallel count over four million
//! items must come to four million, and `rayon::broadcast` reports how many distinct workers really
//! ran; `system_tests/src/user/std_threads_tests.rs` checks that against the cores online. `rayon`
//! is a dependency of this package only (calef's ruling on pull request #1892, 2026-10-10 UTC,
//! under §46 (thin primitives or whole subsystems)); `deny.toml` refuses it anywhere else.

use rayon::prelude::*;

const ITEMS: u64 = 4_000_000;

fn main() {
    let threads = rayon::current_num_threads();
    let sum: u64 = (0..ITEMS).into_par_iter().map(|_| 1u64).sum();
    let mut ran = rayon::broadcast(|ctx| ctx.index());
    ran.sort_unstable();
    ran.dedup();
    println!("rayon threads {threads}");
    println!("rayon sum {sum}");
    println!("rayon broadcast reached {}", ran.len());
}
