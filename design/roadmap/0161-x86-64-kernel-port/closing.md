# Milestone 161 appendix: closing the follow-on, 2026-09-19

An appendix to [milestone 161 (the x86_64 kernel port)](../0161-x86-64-kernel-port.md), moved out
of that page on 2026-10-12 (UTC) under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md).
The main page is complete without it; this is the working of the lane on
`milestone/161-x86-64-port-remainder`, which closed the four items the 2026-09-03 follow-on left
open. Long sentences were split to meet [§213 (writing standards)](../../decisions/0213-writing-standards.md);
no fact changed.

Each item was re-checked against the tree first, because the list said "Checked 2026-09-03". One had
already been done by another milestone: SMP failure (3), by
[milestone 316 (which core booted)](../0316-x86-smp-two-cores.md). The other three were open as
written.

## 1. Blocks in `crates/page_tables`

`PageFormat` grew two required methods, `block_entry` and `is_block`. They have no defaults, so a
fourth format has to say what it does. `Mapper` gained `map_block` and `map_span`; the latter maps
the largest leaf that fits, up to a ceiling the caller passes. Its existing walks learned that a
block is a leaf met early. `map` refuses to descend through one. `unmap` refuses to take a page out
of one (`MapError::InsideBlock`). `translate` stops at one. Nothing splits a block.

`PageSize`, `map_block`, `map_span`, `InsideBlock` and `UnsupportedSize` are provisional names.
VT-d declines blocks (`UnsupportedSize`) until something reads `SLLPS`.

- Parity (rule 5): no scope note is needed. aarch64 and riscv64 had block mappings only in their
  boot maps: two 1 GiB blocks and six gigapages, hand-built in assembly and Rust respectively. Their
  fine maps used 4 KiB leaves exactly as x86's did. All three formats now encode blocks, all three
  are proved, and all three kernels put RAM in blocks and devices in pages.
- Measured on x86_64: on QEMU's 256 MiB, 560 KiB of kernel page tables became 60 KiB. At 4 GiB,
  8,252 KiB became 64 KiB with 1 GiB leaves, or 72 KiB with 2 MiB leaves (`-cpu qemu64`, no
  `Page1GB`).
- Proofs: eight new Kani harnesses. Two are over the leaf choice (soundness: a block never leaves
  its span; completeness: no larger fitting leaf is skipped). Two per format cover the block
  encoding over every address and flag set, and that no table pointer reads as a block. No existing
  harness was changed. Five carry replayable falsification patches, swept red.
  notes/verification.md has the table.
- Recorded, not fixed: the kernel does not read the MTRRs. A block over RAM relies on firmware
  keeping that RAM one memory type (`mmu.rs`'s BUGS).

The fastpath is byte-identical to `main` on all three architectures, measured after merging it:

| arch | figures |
|---|---|
| aarch64 | 5,356 / 7,028 / 1,504 |
| riscv64 | 4,644 / 5,948 / 1,870 |
| x86_64 | 6,256 / 8,190 / 1,701 |

These are the same figures `script/fastpath-footprint` reports for `main` alone. On the pre-merge
tree, the block check the walk now makes cost aarch64's `syscall_entry` four bytes (1,504 to 1,508).
That was measured by neutralizing the check and remeasuring. Merging `main` moved the inlining, and
it is gone.

The gate's riscv64 and x86_64 `syscall_entry` figures sit above `bench/fastpath-*.txt`, and did so
before this lane. That drift is `main`'s, and is left for whoever re-saves those baselines.

## 2. `CR4.PGE` and `CR4.PCIDE`

Measured with an initrd attached, so the EL0 benches that switch `CR3` actually ran on x86_64. The
icount tick counts were byte-identical with PGE on and off on every line. Plain TCG timing
overlapped within 1%. Both are the expected result. The pinned QEMU flushes its whole TLB on every
`CR3` write, whatever the two bits say. And icount counts instructions, which a TLB refill is not.
So the bits stay off and the question is not answered.

No `script/icount` x86 leg was built. The instrument of
[milestone 78 (the load-sensitive assertions)](../0078-load-sensitive-assertions.md) asserts
deadline-timer claims this port's periodic LAPIC timer cannot make. A leg would measure
instructions, which is the one thing already known not to move. The bench gap of
[milestone 186 (derive the architecture list)](../0186-architecture-list-derivation.md), CI running
two legs of three, is untouched.

What would answer it is KVM on cordoba, one `usermod` away, or xenon. PCIDE is a design, not a bit:
INVPCID on three flush paths plus a tag-reuse rule. `mmu.rs` says why.

## 3. SMP failure (1)

Reproduced at `-smp 4` on the first run: 26 of 40 plain boots reported a core "did not start". The
failing core had printed its own `cr4.smep` line from `secondary_main` first.

An instrumented build confirmed the cause in three failures of three. `cpu_start` re-read the online
count after the STARTUP IPIs and waited for it to move again. So a core that checked in during the
200 µs settle delay was counted absent. Fixed by reading the count once, before the INIT.

After: 40 of 40 at `-smp 4`, 20 of 20 at `-smp 8`, and 20 of 20 at `-smp 3`, all cores online. It
could reach two cores as well. That is plausibly the UEFI leg's one-in-three of
[milestone 412 (`script/test`'s UEFI leg asserts two cores online)](../0412-the-uefi-boot-gate-asserts-two-cores-that-do-not-always-start.md),
whose question 2 this answers: `ap_boot`'s "third or later" bound was wrong. That was not measured
here.

## 4. SMP failure (3)

Already fixed by milestone 316 (`BOOT_CPU_ID`); verified in the tree.

## The two-core runner default

Can the x86_64 runner default to more than one core? Not from this milestone. Both bring-up bugs are
closed. [DECISIONS §153 (how a two-core x86_64 test earns its place)](../../decisions/0153-two-core-x86-test-sequencing.md)
makes the flip the closing step of
[milestone 315 (a port revoke that reaches every core)](../0315-port-revoke-every-core.md). It comes
after that milestone's port-revoke broadcast makes `a_revoked_holder_faults_on_its_next_port_write`
reliable at two cores.
