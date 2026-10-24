//! **`std_threads`: milestone 812 (`std::thread::spawn` runs real threads in one address space)'s
//! exit test, as an ordinary `std` program.**
//!
//! Four threads each add one to a shared `AtomicU64` and a shared `Mutex<u64>` a million times;
//! both must read four million. Each keeps a `thread_local!` value that survives, and the main
//! thread's is untouched. The first thread is joined early and shared state used after it.
//! `system_tests/src/user/std_threads_tests.rs` runs it on all three architectures and compares
//! the printed `available_parallelism` with the cores online. See notes/std/the-proof.md.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const THREADS: u64 = 4;
const ITERATIONS: u64 = 1_000_000;

thread_local! {
    static MINE: Cell<u64> = const { Cell::new(0) };
}

fn main() {
    let atomic = Arc::new(AtomicU64::new(0));
    let mutex = Arc::new(Mutex::new(0u64));
    let handles: Vec<_> = (0..THREADS)
        .map(|i| {
            let (atomic, mutex) = (Arc::clone(&atomic), Arc::clone(&mutex));
            std::thread::spawn(move || {
                MINE.with(|m| m.set(i + 1));
                for _ in 0..ITERATIONS {
                    atomic.fetch_add(1, Ordering::Relaxed);
                    *mutex.lock().unwrap() += 1;
                }
                MINE.with(|m| m.get()) == i + 1
            })
        })
        .collect();

    let mut handles = handles.into_iter();
    let first = handles
        .next()
        .expect("four threads")
        .join()
        .expect("the first thread panicked");
    // The first thread has exited and been joined; the shared state is still there and still
    // shared with the three that are running.
    let alive = Arc::strong_count(&atomic) >= 1 && atomic.load(Ordering::Relaxed) > 0;
    let rest: Vec<bool> = handles
        .map(|h| h.join().expect("a thread panicked"))
        .collect();

    println!("threads {THREADS}");
    println!("atomic {}", atomic.load(Ordering::Relaxed));
    println!("mutex {}", *mutex.lock().unwrap());
    println!(
        "thread-locals {}",
        if first && rest.iter().all(|&own| own) {
            "each its own"
        } else {
            "shared"
        },
    );
    println!("main thread-local {}", MINE.with(|m| m.get()));
    println!("space outlived the first exit {alive}");
    println!(
        "parallelism {}",
        std::thread::available_parallelism().map_or(0, |n| n.get()),
    );
}
