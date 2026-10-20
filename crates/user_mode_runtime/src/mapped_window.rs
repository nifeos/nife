//! **One page this process was handed at a fixed virtual address, read and written by volatile
//! access** (milestone 139).
//!
//! Seven programs each hand-rolled the same four to six functions: `r8`/`w8`/`r16`/`w16`/`r32`,
//! sometimes `r16le`/`w16le` or `a_r8`/`a_w8`, every one of them `unsafe { core::ptr::read_volatile
//! ... }` or its `write_volatile` twin, over either a DMA page a driver's wiring maps before
//! `_start` runs or a shared IPC frame the program `PageFrame::MAP`s itself. `entropy.rs`,
//! `keyboard_driver.rs`, `virtio_net_transport.rs`, `multicast_dns_responder.rs`,
//! `socket_test_client.rs`, `smb_server.rs` and `ntp.rs`
//! all carried a copy; `ntp.rs`'s own comment named the duplication out loud ("the same shape
//! `net_stack` and `socket_test_client` use") without anyone lifting it out.
//!
//! **The invariant every copy asserted by hand was the same one**: the offset passed in is inside
//! the one page the kernel mapped at a known base VA. Copying that assertion N times is the §94
//! shape (DECISIONS' `design/decisions/0094-what-may-live-in-a-library.md`), and nothing checked that
//! any of the N were actually kept inside the page: a wrong offset constant was a silent
//! out-of-bounds volatile access, not a caught bug.
//!
//! [`MappedWindow`] holds the one hand-written assertion, at construction, and turns the other end
//! of it (staying inside the page) into a checked `assert!` instead of a repeated comment. A caller
//! that gets an offset wrong now panics at the access instead of reading or writing past the page
//! silently: this is a real reduction in what can go wrong, not a relocation of the same risk.
//!
//! Round 6 added [`MappedWindow::as_slice`]/[`as_mut_slice`](MappedWindow::as_mut_slice), for a
//! different call shape: a caller that hands the whole page to a parsing or serialization routine
//! rather than reading or writing named fields. Six files (`credentialer.rs`,
//! `credentialer_test_client.rs`, `identity_provisioner.rs`, `login_test_client.rs`,
//! `session_reviver.rs`, `smb_server.rs`) each hand-rolled `core::slice::from_raw_parts[_mut]` over
//! their own shared page, the exact same duplicated-invariant shape as the `r8`/`w8` cluster above,
//! just returning a slice instead of one field.
//!

//! # Examples
//!
//! Bare-metal only, like the rest of this crate (see the crate-level doc for why the examples below
//! are `no_run`).
//!
//! ```no_run
//! use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
//!
//! // Constructed once, at the top of the file, the same VA and length every copy hard-coded.
//! const DMA_VA: u64 = 0x0000_0000_0090_0000;
//! // SAFETY: the wiring maps one page read/write at DMA_VA before this program runs.
//! const WINDOW: MappedWindow = unsafe { MappedWindow::new(DMA_VA, PAGE) };
//!
//! // Every access below is safe: bounds-checked against `PAGE`, no unsafe at the call site.
//! WINDOW.w16(0x080, 7);
//! assert_eq!(WINDOW.r16(0x080), 7);
//! ```
//!
//! An offset that would read or write past the window panics instead of touching memory outside
//! it, which is the property no hand-written copy had:
//!
//! ```should_panic
//! # use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
//! # const WINDOW: MappedWindow = unsafe { MappedWindow::new(0x0090_0000, PAGE) };
//! WINDOW.r32(PAGE); // one past the last valid u32, deliberately
//! ```

/// The page size every migrated caller was already assuming (each one's own comment said "one
/// page" or "single-page DMA region"; none of them checked it).
pub const PAGE: u64 = 4096;

/// A caller-owned window onto one page this process was handed at a fixed virtual address: a DMA
/// page a driver's wiring mapped before `_start` ran, or a shared IPC frame the program mapped
/// itself with `PageFrame::MAP`. Every read and write is bounds-checked against `len`, so the only
/// trust this type asks for is the one made at construction.
#[derive(Clone, Copy)]
pub struct MappedWindow {
    base: u64,
    len: u64,
}

impl MappedWindow {
    /// # Safety
    /// `base .. base + len` must be a range the kernel has mapped read/write-accessible into this
    /// process for as long as `self` is used. Every caller in this tree gets that from one of two
    /// places: the wiring maps it before `_start` runs (a DMA page), or the program's own
    /// `PageFrame::MAP` succeeded first (a shared IPC frame) and `self` is not read or written before
    /// that call returns. Both are the same trust every `invoke` already extends to the kernel;
    /// this type does not add a new one, it just asserts it once instead of at every access.
    pub const unsafe fn new(base: u64, len: u64) -> Self {
        Self { base, len }
    }

    /// Panics if `off .. off + size` would fall outside the window. The check every hand-written
    /// copy skipped: a wrong offset constant used to be a silent out-of-bounds volatile access,
    /// and is now a panic naming the access that caused it.
    fn check(&self, off: u64, size: u64) {
        assert!(
            off.checked_add(size).is_some_and(|end| end <= self.len),
            "MappedWindow: offset {off:#x}, size {size}, is outside the {}-byte window",
            self.len
        );
    }

    /// Read a plain integer `T` at byte offset `off`, bounds-checked against the window.
    pub fn read<T: Copy>(&self, off: u64) -> T {
        self.check(off, core::mem::size_of::<T>() as u64);
        // SAFETY: `self.base + off .. + size_of::<T>()` is inside the range `new`'s caller
        // promised was mapped, and the bounds check above keeps it inside `self.len`. Volatile:
        // a peer process or a device may write the same memory out from under an ordinary read.
        unsafe { core::ptr::read_volatile((self.base + off) as *const T) }
    }

    /// Write a plain integer `T` at byte offset `off`, bounds-checked against the window.
    pub fn write<T>(&self, off: u64, val: T) {
        self.check(off, core::mem::size_of::<T>() as u64);
        // SAFETY: as `read`'s.
        unsafe { core::ptr::write_volatile((self.base + off) as *mut T, val) }
    }

    /// Named accessors matching the vocabulary every migrated call site already used
    /// (`r8`/`w8`/`r16`/`w16`/`r32`), so migrating a file changes only where these functions are
    /// *defined*, never where they are called. All native-endian, which is little-endian on both
    /// architectures this tree targets; the `le` some copies wrote in their own name documented an
    /// assumption this makes true rather than a different behaviour.
    ///
    /// Read one byte at `off`.
    pub fn r8(&self, off: u64) -> u8 {
        self.read(off)
    }
    /// Write one byte at `off`.
    pub fn w8(&self, off: u64, v: u8) {
        self.write(off, v);
    }
    /// Read a `u16` at `off`.
    pub fn r16(&self, off: u64) -> u16 {
        self.read(off)
    }
    /// Write a `u16` at `off`.
    pub fn w16(&self, off: u64, v: u16) {
        self.write(off, v);
    }
    /// Read a `u32` at `off`.
    pub fn r32(&self, off: u64) -> u32 {
        self.read(off)
    }
    /// Write a `u32` at `off`.
    pub fn w32(&self, off: u64, v: u32) {
        self.write(off, v);
    }
    /// Read a `u64` at `off`. The caller keeps the offset 8-aligned: this is a volatile read of a
    /// pointer whose alignment the type does not check, and the bulk-copy caller (the framebuffer's
    /// flush) arranges alignment by construction instead.
    pub fn r64(&self, off: u64) -> u64 {
        self.read(off)
    }
    /// Write a `u64` at `off`. See [`r64`](Self::r64) for the alignment the caller owes.
    pub fn w64(&self, off: u64, v: u64) {
        self.write(off, v);
    }

    /// **Move `len` bytes from offset `src` to offset `dst` inside the window**, the ranges allowed
    /// to overlap, as [`slice::copy_within`] does. Both ranges are bounds-checked once, not per
    /// word.
    ///
    /// For a caller moving a block of its own pixels (the terminal's scroll), where a word-at-a-time
    /// loop of [`r64`](Self::r64)/[`w64`](Self::w64) at opt-level 0 costs a call, a bounds check and
    /// a volatile access per word: 349,440 of them for one scroll of the 1120x624 scanout. **Not
    /// volatile**, and safe anyway: no reference is formed, so the compiler assumes nothing about
    /// who else sees the bytes, and what a peer or device reads afterwards is ordered by the fence
    /// and the call the caller makes to publish them, exactly as for its volatile writes. A peer
    /// writing the same bytes meanwhile would make the result a mix of both, as it would for any
    /// copy.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by lane/2451-cpu-matrix-rv64-budget, for
    /// [`slice::copy_within`], whose argument order it keeps.
    pub fn copy_within(&self, src: u64, dst: u64, len: u64) {
        self.check(src, len);
        self.check(dst, len);
        // SAFETY: both ranges are inside the window (checked above), which `new`'s caller promised
        // is mapped read/write; `ptr::copy` is the overlap-safe move.
        unsafe {
            core::ptr::copy(
                (self.base + src) as *const u8,
                (self.base + dst) as *mut u8,
                len as usize,
            );
        }
    }

    /// The whole window as an ordinary byte slice, for a caller that hands it wholesale to a
    /// parsing or serialization routine rather than reading or writing individual fields (round 6:
    /// `credentialer.rs`, `credentialer_test_client.rs`, `identity_provisioner.rs`,
    /// `login_test_client.rs`, `session_reviver.rs` and `smb_server.rs` each hand-rolled
    /// `core::slice::from_raw_parts(VA as *const u8, proto::PAGE)` over their own shared
    /// credential/filesystem/login page, one hand-written `// SAFETY:` comment per site asserting
    /// the same invariant this type already exists to hold once). **Not volatile**: this returns
    /// an ordinary Rust reference the compiler may assume nothing else touches while it is alive,
    /// unlike [`read`](Self::read)/[`write`](Self::write)'s volatile access. Every migrated call
    /// site already used an ordinary (non-volatile) slice this way, so this changes nothing about
    /// that risk; it only collapses where the assertion is written.
    ///
    /// # Safety
    /// As [`new`](Self::new)'s (the window is mapped into this process for as long as the
    /// returned reference is used), plus one more: nothing else, no peer process and no other
    /// reference in this process, reads or writes this range while the returned slice is alive.
    /// Every migrated call site gets that from the IPC protocol's own turn-taking (the shared page
    /// is staged by one side, then handed off by a `SEND`/`receive`, one side touching it at a time)
    /// rather than from a lock. **The claim is the call site's, and it includes this process's own
    /// threads** since milestone 812 (`std::thread::spawn` runs real threads in one address space):
    /// every caller today is a native program built without its own process, so it has one thread
    /// and nothing can give it a second. A caller in a process with threads must say why none of
    /// them touches the range.
    ///
    /// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round
    /// 9's question 2), with [`as_mut_slice`](Self::as_mut_slice). His words: "`as_slice` /
    /// `as_mut_slice`". Refused `bytes`/`bytes_mut` (std spells this pair `as_slice`/`as_mut_slice`).
    pub unsafe fn as_slice(self) -> &'static [u8] {
        // SAFETY: forwarded from this function's own contract, verbatim.
        unsafe { core::slice::from_raw_parts(self.base as *const u8, self.len as usize) }
    }

    /// The mutable twin of [`as_slice`](Self::as_slice); same contract, exclusive access besides.
    ///
    /// # Safety
    /// As [`as_slice`](Self::as_slice)'s.
    ///
    /// Name: ratified 2026-10-08 (calef, #1842), with [`as_slice`](Self::as_slice).
    pub unsafe fn as_mut_slice(self) -> &'static mut [u8] {
        // SAFETY: forwarded from this function's own contract, verbatim.
        unsafe { core::slice::from_raw_parts_mut(self.base as *mut u8, self.len as usize) }
    }
}

/// **Order this process's stores to device-visible memory before a store to a device register
/// that announces them**, and a status read before the payload reads it gates.
///
/// For a driver whose doorbell is a direct store to a device-typed page in its own address space,
/// with no syscall between the ring stores and it: `dmb sy` on aarch64, `fence` on riscv64,
/// `mfence` on x86_64. Lifted on 2026-10-04 by milestone 494 (a driver for the network card a PC
/// actually has) from `components/src/non_volatile_memory_express.rs`, whose own comment explains
/// why it is stronger than [`crate::virtio::virtio_ring_barrier`] (those drivers notify through a
/// syscall) and must not be merged with it. The `e1000e` data plane was its second caller.
///
/// Name: provisional (milestone 494's lane, 2026-10-04).
pub fn doorbell_barrier() {
    // SAFETY: a barrier has no operands and cannot be unsound; it only constrains ordering.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dmb sy", options(nostack, nomem, preserves_flags));
    }
    // SAFETY: as above.
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!("fence", options(nostack, preserves_flags));
    }
    // SAFETY: as above.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!("mfence", options(nostack, nomem, preserves_flags));
    }
    // A fourth architecture fails to build rather than silently ordering nothing.
    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "x86_64"
    )))]
    compile_error!(
        "doorbell_barrier(): this architecture has no ordering named here. A ring publish must be \
         visible before the doorbell that announces it; name the instruction that does that."
    );
}
