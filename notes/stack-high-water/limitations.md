# Stack high water: the BUGS list in full

*Name: provisional.*

An appendix to [notes/stack-high-water.md](../stack-high-water.md), whose BUGS section lists each
entry in one line.

- The per-IPC depth instrument measures two EL0 programs whose role numbers it copies.
  `ipc_stack_depth.rs` spells `os_primitives_benchmarker`'s roles (3, 4) and `soaker`'s (0, 1) as
  local constants, as `bench.rs` and `soak.rs` already do. AGENTS.md rule 7 says a number two
  binaries agree on belongs in a crate; this is the third copy of the first pair and the second of
  the other. A renumbering in either fixture would make the test spawn the wrong role and fail on
  samples, not silently. But the fix is a protocol crate per fixture, and it is not this lane's to
  mint.
- Per-IPC depth is not in a gate. The test asserts that every line is a measurement (enough
  samples, median above the floor, a clean null line) and nothing about the values, because they
  had never existed before 2026-09-19. The release figures are the ones a decision would quote, and
  they come from a bench boot no CI job runs. Promote to a ceiling (`count-at-most` shape) only if
  something starts depending on the value, which the coloring experiment in
  [per-ipc-depth.md](per-ipc-depth.md) would be.
- Depth reached before `paint_boot_stack` runs (a handful of early-boot frames) and never reached
  again is invisible, bounded below by the printed paint floor.
- The static frame sizes in [stack-frame-sizes.md](stack-frame-sizes.md) are per function, not per call chain.
  `-Z emit-stack-sizes` says what one frame costs. It does not say which frames stack on top of each
  other, so it cannot give a worst-case depth on its own. Pairing it with the watermark is what makes
  either number actionable, and a tool that walks the call graph closes the gap.

  `script/stack-depth-check` is that tool (2026-08-16). It was written after this entry and
  `script/stack-frame-check`'s twin of it had both stood unbuilt through two rounds of guard-page
  faults. It reads direct calls out of the disassembly, hangs these frame sizes on the graph, and
  takes the longest path from the entry points a kernel thread stack starts at. Its first answer:
  13792 bytes worst case on aarch64, 13344 on riscv64.

  Do not compare its `thread_entry` chain against a watermark, which is the mistake two drafts of
  that comparison made. The parent's number is whatever was on the stack, nested trap frames
  included. That chain is the thread's own work with no trap on it. Measured over 31 aarch64 runs,
  the watermark read 9536, 9640 and 10600 against a 9456 chain and a 13792 composed bound. The
  excursions are the size of a trap frame plus a handler. The comparison that means something is
  against the composed number.

  Its answer is still a lower bound rather than an upper one. Indirect calls are invisible to it, and
  assembly has no `.stack_sizes` entries at all. So `switch_to`'s 96-byte frame,
  `user_entry_trampoline`'s 272-byte reservation and `spawn_into`'s closure slot are uncounted.
- ~~Nothing gates frame size.~~ `script/stack-frame-check` does, since 2026-08-13, at the
  4096-byte guard page. The entry is kept because the sentence that follows it is still the
  argument for the gate: the 6816-byte frame was legal, compiled without a warning, and was found
  only because a stack overflowed and somebody went looking.
- The two thresholds are the same number for different reasons, and only one of them needs margin.
  The parent's thread row gates a measurement at 14336, and needs margin because the next run may go
  deeper. `script/stack-depth-check` gates a worst-case bound, which already covers every path it can
  see. So it fails at the stack size and only *warns* at 14336. Raise them together or they stop
  describing the same stack.
- A stack whose deepest word happened to store the paint value reads one word shallow.
- The live scans at end of suite are snapshots; a thread that deepens after being scanned is
  under-read by that run. Reaped thread stacks are exact.
- The instrument is `cfg(test)` only: a shell or bench boot measures nothing. The guard pages are
  not: they are in every build, which is what milestone 90 bought.
- The guards are absent on the coarse boot map. So a secondary is unprotected between
  `secondary_boot` and `mmu::init_secondary`, and the boot core between `_start` and `mmu::init`.
  Both windows are a few frames deep and neither has ever been the problem, but neither is zero.
- Nothing checks the guards after boot except the suite. `mmu::verify` runs once, before the map is
  installed. A later mapping that filled a guard page in would not be noticed until the test build
  ran. Nothing does this today, and the mapper refuses to overwrite.
- The boot core's slot in the region is mapped and never used. `MAX_CPUS` slots exist, and one is
  wasted so that slot index can stay CPU id. Any seat the machine does not fill idles the same way;
  the constant is a ceiling since the 2026-08-14 bump to eight. 68 KiB of address space each, no
  frames.
