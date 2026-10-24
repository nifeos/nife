//! **One page this process was handed at a fixed virtual address, read and written by volatile
//! access.**
//!
//! [`MappedWindow`] takes the trust that the page is mapped once, at construction, and
//! bounds-checks every access against it, so a wrong offset panics instead of reading or writing
//! past the page. [`MappedWindow::as_slice`] and [`as_mut_slice`](MappedWindow::as_mut_slice) hand
//! the whole page to a parser or serializer. [`doorbell_barrier`] orders device-visible stores. See
//! notes/user-mode-runtime.md.

//! # Examples
//! Bare-metal only, like the rest of this crate, so the first example is `no_run`.
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

//! An offset that would read or write past the window panics instead of touching memory outside
//! it:
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
/// trust this type asks for is the one made at construction. Accesses are native-endian, which is
/// little-endian on every architecture this tree targets.
#[derive(Clone, Copy)]
pub struct MappedWindow {
    base: u64,
    len: u64,
}

impl MappedWindow {
    /// A window of `len` bytes at `base`.
    ///
    /// # Safety
    /// `base .. base + len` must be a range the kernel has mapped read/write-accessible into this
    /// process for as long as `self` is used: the wiring maps it before `_start` runs (a DMA page),
    /// or the program's own `PageFrame::MAP` succeeded first (a shared IPC frame) and `self` is not
    /// read or written before that call returns. That is the trust every `invoke` already extends
    /// to the kernel, asserted once instead of at every access.
    pub const unsafe fn new(base: u64, len: u64) -> Self {
        Self { base, len }
    }

    /// Panics if `off .. off + size` would fall outside the window, naming the access.
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
    /// word, which is what makes a scroll of the terminal's own pixels cheap.
    ///
    /// Not volatile, and safe anyway: no reference is formed. What a peer or device reads afterwards
    /// is ordered by the fence and the call the caller makes to publish the bytes, as for its
    /// volatile writes; a peer writing the same bytes meanwhile makes the result a mix of both. See
    /// notes/user-mode-runtime.md.
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

    /// The whole window as an ordinary byte slice, for a parser or serializer. **Not volatile**:
    /// the compiler may assume nothing else touches the bytes while the reference is alive.
    ///
    /// # Safety
    /// As [`new`](Self::new)'s, for as long as the returned reference is used, plus one more:
    /// nothing else, no peer process and no other reference or thread in this process, reads or
    /// writes this range while the slice is alive. Callers get that from the IPC protocol's
    /// turn-taking (one side stages the page, then hands it off by a `SEND`/`receive`) rather than
    /// from a lock. A caller in a process with threads must say why none of them touches the range.
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
/// `mfence` on x86_64. Stronger than [`crate::virtio::virtio_ring_barrier`], whose drivers notify
/// through a syscall, and not to be merged with it. See notes/user-mode-runtime.md.
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
