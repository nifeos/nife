# The guard page under each secondary stack (milestone 90)

*Name: provisional.*

An appendix to [notes/stack-high-water.md](../stack-high-water.md), which summarizes it. It records
milestone 90 (a guard page under the per-CPU secondary stacks).

The parent's inventory found an asymmetry rather than assuming symmetry, and the asymmetry was real.
The boot stack and every kernel thread stack had an unmapped page beneath them, and the per-CPU
secondary stacks did not. A secondary that ran deep did not fault. It wrote over whatever `.bss` sat
below, which is the milestone 3 (hand out physical memory, and detect a smashed stack) failure mode (notes/stack.md) on a core that is not the one running
the tests.

## Why it had to move

It could not be skipped where it stood. The stacks were a plain array in `.bss`, and
`map_everything` maps `.data`..`__bss_end` in a single call. There was nowhere to put a hole. So the
fix is a move, and the move is what the milestone is. The array now carries
`#[unsafe(link_section = ".secondary_stacks")]`, and each linker script anchors a page-aligned
`(NOLOAD)` region around whatever it emits. The mapper then walks the slots in a loop, mapping only
each stack and never naming the guard. That is the same thing the boot stack's `__stack_guard` gets
by being skipped between `.bss` and `__stack_bottom`.

The layout, per core (`kernel/src/smp.rs`):

```
  slot n:  [ guard 4 KiB, unmapped ][ stack 64 KiB, kernel_data ]   stride 68 KiB (0x11000)
```

The region is `MAX_CPUS` slots, page-aligned at both ends, and it sits inside
`__image_start`..`__image_end`. So `image_size` in the arm64 Image header still covers it (the
bootloader will not drop a device tree on a stack). And the direct map still skips it, so there is
no second, mapped alias of a guard page. On aarch64 it lands at `__stack_top`,
0x4010c000..0x40194000; on riscv64 at 0x80272000..0x802fa000. Those are eight slots since the
2026-08-14 `MAX_CPUS` bump; the ranges were 0x400fc000..0x40140000 and 0x80266000..0x802aa000 at
four. `MAX_CPUS` stays in Rust and is not written again in either linker script, which is the drift
`cseam` teaches to avoid. A test holds the emitted region against the reserved one from the other
side.

`(NOLOAD)` is load-bearing, and one line of the linker script explains half a megabyte. A
zero-initialized Rust static in an explicitly named section becomes PROGBITS, and the flat binary
that QEMU loads would then carry 544 KiB of zeroes. Marking the output section `(NOLOAD)` makes it
`SHT_NOBITS` again: the ELF grew by nothing (`objcopy -O binary` still emits 421,888 bytes on
aarch64). The cost of the whole feature is 16 KiB of address space and zero physical frames.
Nothing zeroes the region either, which a stack does not need and the paint pass overwrites anyway.

## The proof is a page-table walk, not an overflow

`every_secondary_stack_sits_on_a_guard_page` (in `smp.rs`, portable, so it runs on both ISAs) asks
the live tables for each core's guard page and each side of it. It reads the root back out of
`TTBR1_EL1` / `satp`. The guard must not translate; the stack's bottom and top must.

It is deliberately not a deliberate overflow. A test that faults the kernel to pass is a test the
suite cannot survive. What would actually go wrong here is someone mapping the region as one range
again. The walk catches that, and an overflow test would too, but without killing the machine.
`mmu::verify` checks the same thing per core before installing the map, where the boot stack's guard
has always been checked. So a release build refuses to run on a map that lost the holes.

## What it does not cover

A secondary runs on the coarse boot map from `secondary_boot` until `mmu::init_secondary`, and on
that map the guard page is inside a 2 MiB block and is mapped. That is a handful of instructions of
Rust, and the boot stack's own guard has exactly the same window. It is noted here rather than fixed,
because closing it means fine-grained tables before the MMU is on.

Sizing was not the finding, and it is not taken here. The secondaries run at 12% of 64 KiB. The move
does make shrinking cheap in a way it was not before. The size is one constant in `smp.rs`, slot
stride follows it, and nothing else in the image moves. So 16 KiB per secondary (4x the measured
depth, matching the thread stacks) would return 192 KiB of address space and cost one edit.
Recorded as an option; the guard, not the size, was the gap.

## The guard-page move changed nothing (milestone 90)

Re-measured after the secondary stacks left `.bss` for their own region, full suite, both ISAs, two
runs each (host load average ~8):

| Stack | aarch64, before | aarch64, after | riscv64, before | riscv64, after |
|---|---|---|---|---|
| boot | 53808 | **53808** | 54216 | **54216** |
| core 1 / 2 / 3 | 8504 | **8504** | 8448 | **8448** |
| thread max | 11352 (420 stacks) | **11352 (420 stacks)** | 11672 (415 stacks) | **11672 (415 stacks)** |

Byte for byte, including the paint floors (640 and 1024) and the stack counts, and byte for byte
across the two runs of each ISA as well. OpenSBI's lottery picked hart 0 for one riscv64 run and
hart 3 for the other. So the second run's three numbers came from *different harts on different
slots of the new region*, and were 8448 again. That is the same cross-hart reproduction milestone 84
saw, now over the moved stacks.

The stability is the expected result, and it is worth stating why. Depth is decided by which calls
run, and moving a stack's base address changes no call. Anything else would have meant the move
perturbed the code, and the number to explain would have been the difference. The suite grew by the
two tests this milestone added (aarch64 223 to 225, riscv64 224 to 226). Even that did not move the
boot stack's deepest byte, which says those tests are nowhere near the deepest chain.
