//! Page-map (fault-in) latency on the host OS, lmbench's `lat_mmap`, and the host side of nife's
//! EL0 `map_el0` bench (notes/benchmarks.md, milestone 25).
//!
//! It `mmap`s a large anonymous region and writes one byte per page. Each first touch faults and
//! the kernel installs the PTE, the user-kernel-user round trip nearest our `MAP_INTO`; per page is
//! the touch loop's time over the page count. The trigger differs (a fault, not a syscall) and the
//! host also allocates and zeroes each page; notes/benchmarks/host-programs.md has both asymmetries.
//!
//! Build and run natively (no Cargo, so it does not fight the aarch64 workspace target):
//!   rustc -O bench/host/mmap.rs -o /tmp/mmap && /tmp/mmap

use std::time::Instant;

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
    fn munmap(addr: *mut u8, len: usize) -> i32;
}

const PROT_READ: i32 = 0x1;
const PROT_WRITE: i32 = 0x2;
const MAP_PRIVATE: i32 = 0x0002;
// MAP_ANON's value differs by OS; everything else above is shared.
#[cfg(target_os = "macos")]
const MAP_ANON: i32 = 0x1000;
#[cfg(not(target_os = "macos"))]
const MAP_ANON: i32 = 0x20; // Linux MAP_ANONYMOUS

const PAGE: usize = 4096;

/// Fault in `pages` fresh anonymous pages, one write each, and return ns per page. `mmap` is lazy, so
/// the first write to each page traps into the kernel's fault handler, which installs the PTE (and,
/// unlike our aliasing bench, allocates and zeroes a page). `write_volatile` cannot be optimized away.
fn map_fault_ns(pages: usize) -> f64 {
    let len = pages * PAGE;
    let p = unsafe {
        mmap(
            std::ptr::null_mut(),
            len,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANON,
            -1,
            0,
        )
    };
    assert!(p as isize != -1, "mmap failed");
    let t = Instant::now();
    for i in 0..pages {
        // SAFETY: `p` covers `pages` pages; this writes the first byte of page i, faulting it in.
        unsafe { p.add(i * PAGE).write_volatile(1) };
    }
    let ns = t.elapsed().as_nanos() as f64 / pages as f64;
    unsafe { munmap(p, len) };
    ns
}

fn main() {
    // Warm the mmap and fault paths on a throwaway region (allocator, loop, branch predictor). The
    // timed run uses a fresh region: a first touch cannot be warmed on the same pages, since once
    // faulted they stay resident and a second touch would measure a hit, not a map.
    let _ = map_fault_ns(2_000);
    let ns = map_fault_ns(50_000);
    let os = if cfg!(target_os = "macos") { "macos" } else { "linux" };
    println!("{os} map (first-touch fault-in): {ns:.1} ns/page");
}
