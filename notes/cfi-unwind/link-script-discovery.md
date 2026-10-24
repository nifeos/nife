# The link-script discovery, and what was tried

*Name: provisional. After the parent's own heading.*

[`notes/cfi-unwind.md`](../cfi-unwind.md) says the CFI was being thrown away at link time, and where
the section now goes. This is the account of how that was found, the two placements that failed, and
the measurement that shows the one that worked costs the boot image nothing.

## The line that discarded everything

Adding `.cfi_startproc`/`.cfi_endproc` to the twelve files compiled cleanly and produced correct
`.eh_frame` content. But `llvm-objdump --dwarf=frames` on the linked kernel showed an empty
`.eh_frame` and an empty `.debug_frame`, for every architecture. `kernel/link-{aarch64,riscv64,
x86_64}.ld` each had:

```
/DISCARD/ : {
    ...
    *(.eh_frame*)         /* stack-unwinding tables; we never unwind */
}
```

## Three placements, two rejected

Removing the line was not enough by itself. Three placements were tried, in this order, and the
first two were rejected:

1. Left unmentioned (default orphan placement). LLD keeps `.eh_frame*` ALLOC, because the compiler
   marks it so. (`.debug_info` and `.debug_line` are never ALLOC and need no script attention.) It
   slots the section in with the other read-only data; on aarch64 that lands between `.rodata` and
   `.data`, before `__image_end`. Measured: `__image_size` grew by the CFI's own size, about 100
   KiB. That is the value this kernel's own boot code writes into the arm64/RISC-V Image header for
   a real bootloader, and it grew for a debugging convenience with nothing to do with how much RAM
   the image needs.
2. An explicit `(NOLOAD)` output section, to move it out of the way and off ALLOC. This turned
   it into `SHT_NOBITS`, the same representation `.bss` uses: LLD dropped the actual bytes, and
   `llvm-objdump --dwarf=frames` went back to empty. Wrong mechanism: NOLOAD is for reserving
   address space whose *content* does not matter, and CFI content is the entire point.
3. An explicit `(INFO)` output section, to keep the bytes and drop ALLOC. This let LLD place
   the section far from `.text` (multiple megabytes away in one measured build) and broke a
   `R_AARCH64_PREL32` relocation elsewhere in the image outright: `rust-lld: error: relocation ...
   out of range`.

## What worked, verified

An ordinary output section (no `NOLOAD`, no `INFO`), placed in the script text *after*
`__image_end`/`__image_size` are computed, rather than left to the orphan-placement heuristic:

```
.eh_frame : { KEEP(*(.eh_frame)) KEEP(*(.eh_frame_hdr)) }
```

This stays ALLOC, so the bytes survive. It keeps LLD's normal sequential layout, so it lands right
after `.bss` and the per-core stacks, close enough to `.text` that nothing overflows. And it costs
`__image_size` nothing, because that symbol is already assigned by the time this section exists.

Verified, not assumed. A from-scratch build of the commit immediately before this milestone
(`ece6d72c`) gives `__image_size = 0x350000` on aarch64; this tree, with CFI present, gives the
same `0x350000`. The flat `Image` binary the two builds' `objcopy -O binary` produce (aarch64's
QEMU/board boot artifact) is byte-for-byte identical, `cmp` confirmed.
