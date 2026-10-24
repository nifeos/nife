//! One-way pipe **throughput** on the host OS, and the host side of nife's
//! `sink_throughput` bench (milestone 50, notes/pipes.md).
//!
//! Two forked processes and one pipe: the producer writes until end of file and the reader times
//! it. The same pipe is measured twice. `pipe_16` writes sixteen bytes, the size nife's lockstep
//! endpoint moves, and is the honest head-to-head; `pipe_64k` writes 64 KiB, as a Unix program
//! does, and its gap to `pipe_16` is what buffering buys. notes/benchmarks/host-programs.md has why.
//!
//! Build and run natively (no Cargo, so it does not fight the aarch64 workspace target):
//!   rustc -O bench/host/pipe_throughput.rs -o /tmp/pipe_throughput && /tmp/pipe_throughput

use std::time::Instant;

unsafe extern "C" {
    fn pipe(fds: *mut i32) -> i32;
    fn fork() -> i32;
    fn read(fd: i32, buf: *mut u8, n: usize) -> isize;
    fn write(fd: i32, buf: *const u8, n: usize) -> isize;
    fn close(fd: i32) -> i32;
    fn _exit(code: i32) -> !;
    fn wait(status: *mut i32) -> i32;
}

/// Bytes moved per run. Deliberately larger than one 64 KiB pipe buffer, so the buffered arm has to
/// block at least once and the measurement is about flow control rather than about fitting.
const TOTAL: usize = 4 * 1024 * 1024;

fn main() {
    // The small size first, because it is the one the nife number is compared against.
    for chunk in [16usize, 64 * 1024] {
        let ns = run_one(chunk);
        let secs = ns as f64 / 1e9;
        let mib = TOTAL as f64 / (1024.0 * 1024.0);
        let name = if chunk == 16 { "pipe_16" } else { "pipe_64k" };
        println!(
            "bench: {name} {TOTAL} bytes in {:.3} ms = {:.1} MiB/s ({:.0} ns per {chunk}-byte write)",
            ns as f64 / 1e6,
            mib / secs,
            ns as f64 / (TOTAL / chunk) as f64,
        );
    }
}

/// Move [`TOTAL`] bytes through one pipe in `chunk`-sized writes, and return the nanoseconds the
/// **reader** spent. The reader times it for the reason our consumer does: it is the end of the
/// stream, so it sees the last byte arrive.
fn run_one(chunk: usize) -> u128 {
    let mut fds = [0i32; 2];
    unsafe {
        assert!(pipe(fds.as_mut_ptr()) == 0, "pipe");
    }
    let (r, w) = (fds[0], fds[1]);

    if unsafe { fork() } == 0 {
        // The producer. Writes are looped because a pipe may accept fewer bytes than asked for once
        // the buffer is nearly full, which is precisely the back pressure this is measuring.
        unsafe { close(r) };
        let buf = vec![b'x'; chunk];
        let mut left = TOTAL;
        while left > 0 {
            let want = chunk.min(left);
            let n = unsafe { write(w, buf.as_ptr(), want) };
            if n <= 0 {
                unsafe { _exit(1) };
            }
            left -= n as usize;
        }
        unsafe {
            close(w);
            _exit(0)
        };
    }

    unsafe { close(w) };
    let mut buf = vec![0u8; chunk];
    // The first read is outside the window: it pays the child's fork and exec-less startup, which is
    // the same thing our consumer's warmup messages pay for.
    let first = unsafe { read(r, buf.as_mut_ptr(), chunk) };
    assert!(first > 0, "the producer wrote nothing");
    let mut got = first as usize;

    let t0 = Instant::now();
    while got < TOTAL {
        let n = unsafe { read(r, buf.as_mut_ptr(), chunk) };
        if n <= 0 {
            break;
        }
        got += n as usize;
    }
    let ns = t0.elapsed().as_nanos();
    unsafe {
        close(r);
        let mut status = 0i32;
        wait(&mut status);
    }
    assert_eq!(got, TOTAL, "the pipe lost bytes");
    ns
}
