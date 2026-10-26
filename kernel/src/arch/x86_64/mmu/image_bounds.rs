//! **The kernel image's bounds**, read back out of the linker script.
//!
//! Cut out of `mmu.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines).
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

// ---------------------------------------------------------------------------------------------
// The image bounds, read back out of the linker script. These are real: the addresses exist the
// moment the image is linked, and `stack.rs` (the overflow canary and the backtrace's is-this-code
// test) needs them before anything else here works.
// ---------------------------------------------------------------------------------------------

macro_rules! linker_symbol {
    ($(#[$doc:meta])* $name:ident, $sym:ident) => {
        $(#[$doc])*
        pub fn $name() -> u64 {
            unsafe extern "C" {
                static $sym: core::ffi::c_void;
            }
            (&raw const $sym) as u64
        }
    };
}

linker_symbol!(
    /// The boot stack's guard page: one page of address space beneath the stack, and a **hole** in
    /// the map [`initialize`](super::initialize) builds. `verify` asserts it is one; before
    /// `initialize` the boot map covers it like everything else, which is worth knowing while
    /// reading a stack-overflow report from the first few steps of the boot tour.
    stack_guard,
    __stack_guard
);
linker_symbol!(
    /// The low end of the boot stack (it grows down, so this is the limit, not the start).
    stack_bottom,
    __stack_bottom
);
linker_symbol!(
    /// One past the high end of the boot stack: where `rsp` started.
    stack_top,
    __stack_top
);
linker_symbol!(
    /// The first byte of the kernel's `.text`.
    text_start,
    __text_start
);
linker_symbol!(
    /// One past the last byte of the kernel's `.text`.
    text_end,
    __text_end
);
linker_symbol!(
    /// The first byte of the kernel's `.rodata`: readable, and executable by nobody.
    rodata_start,
    __rodata_start
);
linker_symbol!(
    /// One past the last byte of `.rodata`.
    rodata_end,
    __rodata_end
);
linker_symbol!(
    /// The first byte of `.data`. `.data`, `.bss` and the boot stack are contiguous in
    /// link-x86_64.ld, so one range from here to [`bss_end`] covers the writable image.
    data_start,
    __data_start
);
linker_symbol!(
    /// One past the last byte of `.bss`.
    bss_end,
    __bss_end
);
