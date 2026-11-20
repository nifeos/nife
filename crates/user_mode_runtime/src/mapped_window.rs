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
//! use user_mode_runtime::mapped_window::{MappedWindow, PAGE_BYTES};
//!
//! // Constructed once, at the top of the file, the same VA and length every copy hard-coded.
//! const DMA_VA: u64 = 0x0000_0000_0090_0000;
//! // SAFETY: the wiring maps one page read/write at DMA_VA before this program runs.
//! const WINDOW: MappedWindow = unsafe { MappedWindow::new(DMA_VA, PAGE_BYTES) };
//!
//! // Every access below is safe: bounds-checked against `PAGE_BYTES`, no unsafe at the call site.
//! WINDOW.write_volatile::<u16>(0x080, 7);
//! assert_eq!(WINDOW.read_volatile::<u16>(0x080), 7);
//! ```

//! An offset that would read or write past the window panics instead of touching memory outside
//! it:
//!
//! ```should_panic
//! # use user_mode_runtime::mapped_window::{MappedWindow, PAGE_BYTES};
//! # const WINDOW: MappedWindow = unsafe { MappedWindow::new(0x0090_0000, PAGE_BYTES) };
//! WINDOW.read_volatile::<u32>(PAGE_BYTES); // one past the last valid u32, deliberately
//! ```

/// The page size every migrated caller was already assuming (each one's own comment said "one
/// page" or "single-page direct memory access (DMA) region"; none of them checked it).
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-15 (UTC) by lane/1112-outside-review-fixes.
pub const PAGE_BYTES: u64 = 4096;

mod sealed {
    /// The seal: only this crate names it, so only this crate implements `WindowInteger`.
    ///
    /// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-14 (UTC) by milestone/1101-1113-outside-review-fixes.
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl<T: Sealed, const N: usize> Sealed for [T; N] {}
}

/// **A plain integer a window can carry**: every bit pattern is a valid value, so a read of bytes a
/// device or peer wrote cannot produce an invalid one. Sealed: only the unsigned integers up to
/// 64 bits and arrays of them implement it, and nothing outside this crate can.
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-14 (UTC) by milestone/1101-1113-outside-review-fixes; `Word`
/// and `Integer` were considered.
pub trait WindowInteger: sealed::Sealed + Copy {}
impl WindowInteger for u8 {}
impl WindowInteger for u16 {}
impl WindowInteger for u32 {}
impl WindowInteger for u64 {}
/// An array of integers is as plain as one integer, and a device command is 64 bytes of them.
impl<T: WindowInteger, const N: usize> WindowInteger for [T; N] {}

/// A caller-owned window onto one page this process was handed at a fixed virtual address: a DMA
/// page a driver's wiring mapped before `_start` ran, or a shared inter-process communication (IPC) frame the program mapped
/// itself with `PageFrame::MAP`. Every read and write is bounds-checked against `length`, so the only
/// trust this type asks for is the one made at construction. Accesses are native-endian, which is
/// little-endian on every architecture this tree targets.
#[derive(Clone, Copy)]
pub struct MappedWindow {
    base: u64,
    length: u64,
}

impl MappedWindow {
    /// A window of `length` bytes at `base`.
    ///
    /// # Safety
    /// `base .. base + length` must be a range the kernel has mapped read/write-accessible into this
    /// process for as long as `self` is used: the wiring maps it before `_start` runs (a DMA page),
    /// or the program's own `PageFrame::MAP` succeeded first (a shared IPC frame) and `self` is not
    /// read or written before that call returns. That is the trust every `invoke` already extends
    /// to the kernel, asserted once instead of at every access.
    pub const unsafe fn new(base: u64, length: u64) -> Self {
        Self { base, length }
    }

    /// **The `length` bytes at `offset`**, a window inside this one. Panics if they would run past it.
    ///
    /// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-10 (UTC) by lane/drivers-read-the-startup-page.
    pub fn window(&self, offset: u64, length: u64) -> MappedWindow {
        self.check(offset, length);
        Self {
            base: self.base + offset,
            length,
        }
    }

    /// Panics if `offset .. offset + size` would fall outside the window, naming the access.
    fn check(&self, offset: u64, size: u64) {
        assert!(
            offset
                .checked_add(size)
                .is_some_and(|end| end <= self.length),
            "MappedWindow: offset {offset:#x}, size {size}, is outside the {}-byte window",
            self.length
        );
    }

    /// Panics if the address `offset` bytes into the window is not a multiple of `align`.
    fn check_aligned(&self, offset: u64, align: u64) {
        assert!(
            self.base.wrapping_add(offset).is_multiple_of(align),
            "MappedWindow: offset {offset:#x} is not {align}-byte aligned"
        );
    }

    /// Read a plain integer `T` at byte offset `offset`, bounds-checked against the window.
    ///
    /// # Panics
    /// If the access runs past the window, or `offset` is not a multiple of `T`'s alignment.
    ///
    /// Name: ratified 2026-11-20 (UTC), replacing `read`, which stays as a `doc(alias)`: the
    /// access is volatile, as `core::ptr::read_volatile` is, and the name says so.
    #[doc(alias = "read")]
    pub fn read_volatile<T: WindowInteger>(&self, offset: u64) -> T {
        self.check(offset, core::mem::size_of::<T>() as u64);
        self.check_aligned(offset, core::mem::align_of::<T>() as u64);
        // SAFETY: `self.base + offset .. + size_of::<T>()` is inside the range `new`'s caller
        // promised was mapped, and the bounds check above keeps it inside `self.length`. Volatile:
        // a peer process or a device may write the same memory out from under an ordinary read.
        unsafe { core::ptr::read_volatile((self.base + offset) as *const T) }
    }

    /// Write a plain integer `T` at byte offset `offset`, bounds-checked against the window.
    ///
    /// # Panics
    /// As [`read_volatile`](Self::read_volatile).
    ///
    /// Name: ratified 2026-11-20 (UTC), replacing `write`, which stays as a `doc(alias)`, for the
    /// same reason as `read_volatile`.
    #[doc(alias = "write")]
    pub fn write_volatile<T: WindowInteger>(&self, offset: u64, value: T) {
        self.check(offset, core::mem::size_of::<T>() as u64);
        self.check_aligned(offset, core::mem::align_of::<T>() as u64);
        // SAFETY: as `read_volatile`'s.
        unsafe { core::ptr::write_volatile((self.base + offset) as *mut T, value) }
    }

    /// **Move `bytes` bytes from offset `source` to offset `destination` inside the window**, the ranges allowed
    /// to overlap, as [`slice::copy_within`] does. Both ranges are bounds-checked once, not per
    /// word, which is what makes a scroll of the terminal's own pixels cheap.
    ///
    /// Not volatile, and safe anyway: no reference is formed. What a peer or device reads afterwards
    /// is ordered by the fence and the call the caller makes to publish the bytes, as for its
    /// volatile writes; a peer writing the same bytes meanwhile makes the result a mix of both. See
    /// notes/user-mode-runtime.md.
    ///
    /// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-10-20 (UTC) by lane/2451-cpu-matrix-rv64-budget, for
    /// [`slice::copy_within`], whose argument order it keeps.
    pub fn copy_within(&self, source: u64, destination: u64, bytes: u64) {
        self.check(source, bytes);
        self.check(destination, bytes);
        // SAFETY: both ranges are inside the window (checked above), which `new`'s caller promised
        // is mapped read/write; `ptr::copy` is the overlap-safe move.
        unsafe {
            core::ptr::copy(
                (self.base + source) as *const u8,
                (self.base + destination) as *mut u8,
                bytes as usize,
            );
        }
    }

    /// **Copy `output.len()` bytes starting at offset `offset` into `output`**, with one bounds check for
    /// the whole range rather than one per byte.
    ///
    /// Not volatile, as [`copy_within`](Self::copy_within) is not. A compiler fence on each side
    /// keeps the copy where the caller put it among its volatile accesses and barriers, so a
    /// driver that reads a descriptor's ownership bit, then [`doorbell_barrier`], then this,
    /// reads the buffer after the bit. A peer or device writing the range during the copy makes
    /// the result a mix of old and new bytes; the caller's protocol rules that out. See
    /// notes/user-mode-runtime.md.
    ///
    /// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-10-25 (UTC) by lane milestone/53-radon-receive-drain, for
    /// [`slice::copy_from_slice`] seen from the slice's side.
    pub fn copy_to_slice(&self, offset: u64, output: &mut [u8]) {
        self.check(offset, output.len() as u64);
        // PAIR: no other fence. Compiler-only: keeps the copy after the caller's preceding
        // volatile accesses and barrier; the hardware ordering is the caller's doorbell_barrier.
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        // SAFETY: `offset .. offset + output.len()` is inside the window (checked above), which `new`'s
        // caller promised is mapped readable; `output` is an exclusive borrow of ordinary memory, so
        // the two ranges cannot overlap.
        unsafe {
            core::ptr::copy_nonoverlapping(
                (self.base + offset) as *const u8,
                output.as_mut_ptr(),
                output.len(),
            );
        }
        // PAIR: no other fence. Compiler-only: keeps the copy before the caller's following
        // volatile accesses, doorbell_barrier or IPC that publishes the bytes.
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }

    /// **Copy `source` to offset `offset`**, with one bounds check for the whole range. Not volatile,
    /// fenced against the compiler on each side, as [`copy_to_slice`](Self::copy_to_slice).
    ///
    /// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-10-25 (UTC) by lane milestone/53-radon-receive-drain, for
    /// [`slice::copy_from_slice`].
    pub fn copy_from_slice(&self, offset: u64, source: &[u8]) {
        self.check(offset, source.len() as u64);
        // PAIR: no other fence. Compiler-only: keeps the copy after the caller's preceding
        // volatile accesses and barrier; the hardware ordering is the caller's doorbell_barrier.
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        // SAFETY: `offset .. offset + source.len()` is inside the window (checked above), which `new`'s
        // caller promised is mapped writable; `source` is a shared borrow of ordinary memory, which
        // this process's own window cannot alias while it is borrowed.
        unsafe {
            core::ptr::copy_nonoverlapping(
                source.as_ptr(),
                (self.base + offset) as *mut u8,
                source.len(),
            );
        }
        // PAIR: no other fence. Compiler-only: keeps the copy before the caller's following
        // volatile accesses, doorbell_barrier or IPC that publishes the bytes.
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
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
        unsafe { core::slice::from_raw_parts(self.base as *const u8, self.length as usize) }
    }

    /// The mutable twin of [`as_slice`](Self::as_slice); same contract, exclusive access besides.
    ///
    /// # Safety
    /// As [`as_slice`](Self::as_slice)'s.
    ///
    /// Name: ratified 2026-10-08 (calef, #1842), with [`as_slice`](Self::as_slice).
    pub unsafe fn as_mut_slice(self) -> &'static mut [u8] {
        // SAFETY: forwarded from this function's own contract, verbatim.
        unsafe { core::slice::from_raw_parts_mut(self.base as *mut u8, self.length as usize) }
    }
}

/// **Order this process's stores to device-visible memory before a store to a device register
/// that announces them**, and a status read before the payload reads it gates.
///
/// For a driver whose doorbell is a direct store to a device-typed page in its own address space,
/// with no system call between the ring stores and it: `dmb sy` on aarch64, `fence` on riscv64,
/// `mfence` on x86_64. Stronger than [`crate::virtual_io::virtual_io_ring_barrier`], whose drivers notify
/// through a system call, and not to be merged with it. See notes/user-mode-runtime.md.
///
/// Name: ratified 2026-11-20 (UTC); first minted by the lane for milestone 494 (a driver for the network card) on 2026-10-04.
pub fn doorbell_barrier() {
    // No `nomem`. The Rust reference (Inline assembly, Options) defines it as a block that "does not
    // read or write to any memory accessible outside of the `asm!` block", which lets the compiler
    // cache values in registers and move loads and stores across it, and it adds that the compiler
    // "also assumes that this `asm!` block does not perform any kind of synchronization with other
    // threads, e.g. via fences". A barrier exists to order memory, so it must not claim either.
    // SAFETY: a barrier has no operands and cannot be unsound; it only constrains ordering.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dmb sy", options(nostack, preserves_flags));
    }
    // SAFETY: as above.
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!("fence", options(nostack, preserves_flags));
    }
    // SAFETY: as above.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
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

#[cfg(test)]
mod tests {
    use super::MappedWindow;

    /// Eight aligned words of ordinary memory, and a window over them.
    fn window(storage: &mut [u64; 8]) -> MappedWindow {
        // SAFETY: `storage` is live for the test and is 64 bytes of writable memory.
        unsafe { MappedWindow::new(storage.as_mut_ptr() as u64, 64) }
    }

    #[test]
    fn integers_round_trip_at_every_width() {
        let mut storage = [0u64; 8];
        let w = window(&mut storage);
        w.write_volatile::<u8>(1, 0xAB);
        w.write_volatile::<u16>(2, 0xBEEF);
        w.write_volatile::<u32>(4, 0xDEAD_BEEF);
        w.write_volatile::<u64>(8, 0x0123_4567_89AB_CDEF);
        assert_eq!(w.read_volatile::<u8>(1), 0xAB);
        assert_eq!(w.read_volatile::<u16>(2), 0xBEEF);
        assert_eq!(w.read_volatile::<u32>(4), 0xDEAD_BEEF);
        assert_eq!(w.read_volatile::<u64>(8), 0x0123_4567_89AB_CDEF);
        w.write_volatile(16, [1u32, 2, 3, 4]);
        assert_eq!(w.read_volatile::<[u32; 4]>(16), [1, 2, 3, 4]);
    }

    #[test]
    #[should_panic(expected = "outside the 64-byte window")]
    fn an_access_past_the_end_panics() {
        let mut storage = [0u64; 8];
        window(&mut storage).read_volatile::<u64>(60);
    }

    #[test]
    #[should_panic(expected = "outside the 64-byte window")]
    fn an_offset_that_wraps_panics() {
        let mut storage = [0u64; 8];
        window(&mut storage).read_volatile::<u8>(u64::MAX);
    }

    #[test]
    #[should_panic(expected = "not 4-byte aligned")]
    fn a_misaligned_access_panics() {
        let mut storage = [0u64; 8];
        window(&mut storage).read_volatile::<u32>(2);
    }

    #[test]
    fn a_sub_window_is_checked_against_its_own_length() {
        let mut storage = [0u64; 8];
        let w = window(&mut storage);
        let inner = w.window(16, 8);
        inner.write_volatile::<u64>(0, 7);
        assert_eq!(w.read_volatile::<u64>(16), 7);
        let caught = std::panic::catch_unwind(|| inner.read_volatile::<u8>(8));
        assert!(caught.is_err());
    }

    #[test]
    fn copies_move_bytes_and_allow_overlap() {
        let mut storage = [0u64; 8];
        let w = window(&mut storage);
        w.copy_from_slice(0, &[1, 2, 3, 4, 5, 6, 7, 8]);
        w.copy_within(0, 2, 6);
        let mut output = [0u8; 8];
        w.copy_to_slice(0, &mut output);
        assert_eq!(output, [1, 2, 1, 2, 3, 4, 5, 6]);
    }
}
