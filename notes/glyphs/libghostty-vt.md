# What adopting libghostty-vt would cost now

An appendix to [notes/glyphs.md](../glyphs.md), which keeps the recommendation. This file holds the
pricing behind it. It is a recommendation, not a decision.

## What it is, and why it is named

The roadmap names libghostty-vt as the strongest form of the claim of milestone 23 (a
capability-routed component OS with live replacement). It is Ghostty's extracted VT core: no libc, a
C ABI, written in Zig. It needs a supplied allocator, per
[the proposal](../../design/roadmap/0705-the-graphical-terminal-runs-full-screen-programs.md).
Milestone 36 (a foreign-language component, seam first) built the C seam to de-risk it: DECISIONS §31
(the foreign-language seam), [c-seam.md](../c-seam.md). The Rust engine is built, so the comparison
rests on facts.

## What it would buy

A vendor component in a language we do not use, capability-confined and hot-swappable, is the thesis
in its strongest available form: the more unverified the component, the more the confinement has to
prove. And a real VT engine is *much* more complete than ours. It has scrollback, reflow on resize,
UTF-8 and grapheme clustering, the DEC modes, mouse reporting, and a conformance history against
`vttest` that we would otherwise be writing from scratch for years.

## What it would cost, concretely, now that the seam and the Rust engine both exist

1. A Zig toolchain in the build, for one component, pinned. Milestone 36 already accepted a `clang` in
   the build for C, and priced that. Zig is a second one, and it is the cost that does not go away.
2. The seam is proved, but the shape is not free. The C seam of §31 holds *no capabilities and makes
   no syscalls*: the Rust shim holds everything and passes buffers. A VT engine fits that shape almost
   perfectly (bytes in, grid out, no IO), the same sans-IO property `crates/video_terminal` has. So the
   port is a shim that feeds bytes and reads cells, not a rewrite of `display_terminal`.
3. The grid readback is the actual work. Our engine gives `pixel(x, y)` as a pure function, which is
   what makes the three-witness proof possible. libghostty-vt's C ABI gives cells. The shim would have
   to walk them, and the *expected-picture* definition would have to move to the Zig side, or be
   reimplemented against its cell layout. The proof structure, not the rendering, is what would have
   to be rebuilt.
4. Their API is in flux. So any adoption pins a version, and takes the divergence-management
   discipline the vendored RedoxFS already has (the vendoring policy of DECISIONS §18 (the PCIe
   transport)).
5. `crates/video_terminal` would not be deleted. It is about 1,750 lines including its tests and its
   keymap; it was 1,500 when this was written. That is a hedged magnitude, re-measured at each
   documentation sweep rather than gated, because a line count moves on every test anyone adds. It is
   the thing that makes the host-side scanout check possible. Keeping it as the reference
   implementation the foreign one is *checked against* is more valuable than either alone. It is a
   better milestone-23 demonstration too: swap the engine, run the same suite, compare the grids.

## The recommendation

Adopt it as a *second* engine behind the same seam, not as a replacement. Do it when there is a
reason to want scrollback and UTF-8, rather than a reason to want a Zig dependency. The milestone-23
claim is strongest when the two engines can be swapped under a suite that grades both, and that is
only possible because the Rust one exists. If the answer is "not yet", nothing is lost.
`display_terminal` is a component behind an endpoint, so swapping it later is a component change,
which is the property this increment was asked to keep, and did.
