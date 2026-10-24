//! **`std_heap_contention`: how the `std` heap's spinlock holds up under four threads** (milestone 812
//! (`std::thread::spawn` runs real threads in one address space), the block's item 12: "measure it
//! under four threads before replacing it").
//!
//! It times `ALLOCATIONS` allocations and frees on one thread, then split across four, and prints
//! the counter ticks for `system_tests::user::std_threads_tests`; `notes/std/threads.md` records
//! them. Then [`ENDED`] threads in turn each end with a full cache (milestone 561 (a per-CPU
//! allocator is what the current-CPU page was for)), which exhausts the one-mebibyte heap budget
//! unless a thread's end returns its cache. See notes/std/the-proof.md.

use std::alloc::{Layout, alloc, dealloc};
use std::time::Instant;

const ALLOCATIONS: usize = 200_000;

/// How many threads in turn fill their cache and end: 64 full caches are 2.2 MB, twice the budget.
const ENDED: usize = 64;

/// Allocate sixteen blocks of every size a cache holds, 16 to 256 bytes, then free them all, which
/// leaves this thread's cache full.
fn fill_my_cache() {
    let layouts: Vec<Layout> = (1..=16)
        .map(|k| Layout::from_size_align(k * 16, 8).expect("a valid layout"))
        .collect();
    let mut blocks = Vec::with_capacity(16 * layouts.len());
    for l in &layouts {
        for _ in 0..16 {
            // SAFETY: a nonzero layout.
            let p = unsafe { alloc(*l) };
            assert!(!p.is_null(), "out of memory filling a cache");
            blocks.push((p, *l));
        }
    }
    for (p, l) in blocks {
        // SAFETY: allocated just above with this layout, and freed once.
        unsafe { dealloc(p, l) };
    }
}

fn churn(n: usize) {
    for i in 0..n {
        let b = Box::new([i as u8; 48]);
        std::hint::black_box(&b);
    }
}

fn timed(threads: usize) -> u128 {
    let start = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|_| std::thread::spawn(move || churn(ALLOCATIONS / threads)))
        .collect();
    for h in handles {
        h.join().expect("a churning thread panicked");
    }
    start.elapsed().as_micros()
}

fn main() {
    let one = timed(1);
    let four = timed(4);
    println!("heap allocations {ALLOCATIONS}");
    println!("one thread us {one}");
    println!("four threads us {four}");
    for _ in 0..ENDED {
        std::thread::spawn(fill_my_cache)
            .join()
            .expect("a cache-filling thread panicked");
    }
    println!("threads ended with full caches {ENDED}");
}
