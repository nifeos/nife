# The framebuffer contract: the appendices to notes/framebuffer-contract.md

[`notes/framebuffer-contract.md`](../framebuffer-contract.md) carries the contract, the memory
story, the confinement hazard a GPU adds, what the tests prove, the seams left open, BUGS, and where
the pieces are. These files carry the evidence its prose budget has no room for.

- [`iommu-escape-test.md`](iommu-escape-test.md): why the escape test reads the IOMMU's fault queue
  and not the device, and the RISC-V fault-queue overflow it ran into.
- [`scanout-referee.md`](scanout-referee.md): what makes the host-side scanout check a real check,
  the 2026-08-26 resize bug that was a guest hang, and the 2026-10-12 dumps QEMU's monitor refused.
- [`later-rungs.md`](later-rungs.md): what the compositor (milestone 33 (a compositor)) and the
  display terminal (milestone 29 (a display terminal)'s text increment) did with this contract.
- [`flushed-hang.md`](flushed-hang.md): the second `FLUSH` that never returned, and the boot
  evidence that resolved it.
- [`surface-geometry.md`](surface-geometry.md): why the surface is 1120 by 624, and the last
  frame's padding.
- [`kernel-wiring.md`](kernel-wiring.md): how `kernel/src/user/display_service.rs` wires the GPU
  driver, its clients and the firmware-screen terminal, and why each holds what it holds.
- [`the-display-tests.md`](the-display-tests.md): what each test in
  `system_tests/src/user/display_tests.rs` exercises, beyond what the parent says it proves.

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes, which split the parent
under §212 (a prose budget). The directory follows §212's siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

The stems below were minted the same day by the same lane. Each is named for its content, and calef
names files.

- `iommu-escape-test` Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.
- `scanout-referee` Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes,
  after the parent's own word for the check.
- `later-rungs` Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.
- `flushed-hang` Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.
- `surface-geometry` Name: provisional, minted 2026-10-24 (UTC) by comments/929-crates-17.
- `kernel-wiring` Name: provisional, named for its content.
- `the-display-tests` Name: provisional, named for its content.
