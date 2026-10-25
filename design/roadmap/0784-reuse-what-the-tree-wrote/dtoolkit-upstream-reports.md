# Reports to `dtoolkit`

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md), drafted from
[`device_tree_blob` against `dtoolkit`](dtoolkit-against-device-tree-blob.md). Name provisional.

calef approved all three on 2026-10-25 (UTC), and each is filed on google/dtoolkit:
[#101](https://github.com/google/dtoolkit/issues/101) (R1),
[#102](https://github.com/google/dtoolkit/issues/102) (R2) and
[#103](https://github.com/google/dtoolkit/issues/103) (R3). These are bug reports, not code, so
they needed no CLA. Each is written against 0.4.1 and `main` at `4a7faf2e7`; the filed issues carry
a self-contained Rust reproduction in place of the probe.
[dtoolkit-probe.sh](dtoolkit-probe.sh) reproduces each, in the mode named under it, through
`dtoolkit`'s public API alone.

## R1. `Fdt::new` rejects a whole tree over one property name of 32 characters (#101)

`Fdt::new` returns `InvalidPropertyName` for any blob that holds a property name
longer than 31 characters, or one with a character outside `[A-Za-z0-9,._+?#-]`. The whole tree
becomes unreadable, not just that property.

The Devicetree Specification gives 31 as the limit, but `dtc` compiles longer
names, and Linux ships them. `snps,dis-u2-freeclk-exists-quirk` (32 characters) is in
`arch/arm64/boot/dts/rockchip/rk3399-base.dtsi`, `rk3588-base.dtsi`, `rk3328.dtsi`, `rk3528.dtsi`,
`rk3576.dtsi`, `exynos/exynos2200.dtsi` and `arch/riscv/boot/dts/spacemit/k1.dtsi`, and the DWC3
driver reads it. Firmware that passes one of those trees to a kernel using `dtoolkit` hands it a tree
it cannot open, so the kernel finds no memory.

Take any valid blob, append `snps,dis-u2-freeclk-exists-quirk\0` to the strings
block (growing `size_dt_strings` and `totalsize`), and point one property's `nameoff` at it.
`Fdt::new` fails with `InvalidPropertyName`. The same change with a 31-character name parses.
(`dtoolkit-probe.sh agree`, the `prop-name-*` variants.)

Validate names only on the write side, or make read-side name validation opt-in
(for example `Fdt::new_strict`), so a reader accepts what `dtc` and Linux accept. If validation stays
on by default, a length limit that matches what Linux trees contain would be enough for this case.

## R2. Visiting every node through `children()` takes time quadratic in depth (#102)

`FdtChildIter::next` finds the next sibling with `next_sibling_offset`, which walks
the whole subtree of the previous child. A depth-first visit of every node therefore costs the sum
of all subtree sizes. On a chain of nested nodes that is quadratic.

Measured with `dtoolkit-probe.sh deep`, visiting every node with an explicit stack of `children()`
iterators, release build, on a Linux x86_64 host:

| tree | bytes | `Fdt::new` | visit every node |
|---|---|---|---|
| chain, 10,000 deep | 120,076 | 4.6 ms | 0.87 s |
| chain, 50,000 deep | 600,076 | 6.5 ms | 18 s |
| chain, 174,000 deep | 2,088,076 | 15 ms | 237 s |
| flat, 174,000 wide | 2,088,076 | 15 ms | 12 ms |

The 174,000-deep tree is inside Linux's 2 MiB `MAX_FDT_SIZE` and passes `Fdt::new`, so its input
can hold a caller that walks a validated tree for minutes. The crate's own fuzz target walks
the same way, recursively.

When a child has been fully walked, the iterator already knows where it ended;
resuming from there makes a full visit linear. A flat, single-pass iterator over the structure
block (each node with its depth) would serve callers that want every node and keep no stack.

## R3. `Display` panics on a tree nested 16,384 levels deep (#103)

`fmt_recursive` indents with `{:indent$}` and four columns per level. At 16,384
levels below the root the width passes `u16::MAX`, and formatting panics with "Formatting argument
out of range" (`src/fdt/node.rs`, in 0.4.1 line 98). The blob is 196 KiB and `Fdt::new` accepts it;
16,383 levels print.

A chain of 16,384 nested nodes named `a`, then `format!("{fdt}")`.
(`dtoolkit-probe.sh deep`.)

Cap the indentation, or write the indent in a loop. The recursion has no bound either,
so a smaller stack than the host's 8 MiB would overflow first; an iterative printer avoids both.
