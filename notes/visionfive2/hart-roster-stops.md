# The S7 and the online set: bench stops one to three

*Name: ratified 2026-10-18 (calef, "Approve all").*

An appendix to [notes/visionfive2.md](../visionfive2.md). It records the 2026-08-14 bench stops
that came from the JH7110's five harts: the correction to this note's first claim, the vendor
tree's two lies about the S7, and the first online set not contiguous from zero.

## Five harts, one of which must not be started

The JH7110 is 1x SiFive S7 (hart 0) + 4x U74 (harts 1..4). The S7 is `rv64imac_zba_zbb`. It has no
MMU and no S-mode, and its cpu node says `status = "disabled"` [dtsi]. The U74s are
`rv64imafdc_zba_zbb`, `mmu-type = "riscv,sv39"` [dtsi], exactly the kernel's contract.

## A correction, 2026-08-14

The roster half of this was already built when this note first claimed otherwise. `CpuList` has
read `status` since milestone 100 (read the machine's PSCI and its CPU list). And
`smp::bring_up_secondaries` refuses a disabled hart by name rather than starting it. What
`sbi_hart_start` would answer for hart 0 stays on the bench list, only to confirm the refusal is
the right call.

What was genuinely missing was built 2026-08-14. The ISA record (`isa::riscv64`) counted disabled
harts, so the S7's `rv64imac` narrowed the machine's common extensions. And an S7 whose tree spells
its MMU as `riscv,none` would have read as a machine that cannot run us at all. A disabled hart now
contributes nothing to the record. That is host-proven against the hand-written jh7110 fixture
(crates/machine_discovery/tests/riscv64_jh7110.rs).

This paragraph used to end on a roster limitation: `cpu::MAX_CPUS` was 4 against this SoC's five
described harts. It closed on 2026-08-14. The constant is 8 and the roster seats cores by hart id,
so hart 4 has a seat. [bench-boots-2026-08.md](bench-boots-2026-08.md) carries the details and what
boot 10 had to prove.

## Second bench stop: the vendor tree lies about the S7, twice

The fix above never fires on the real board. Everything in the parent cited from [dtsi] describes
mainline. The tree the flashed firmware hands over was read at the U-Boot prompt (`fdt print`), and
it says something else. Measured: all five cpu nodes carry `status = "okay"`. And cpu@0 carries
`riscv,isa = "rv64imacu"` with `mmu-type = "riscv,sv39"`. So the vendor tree marks the S7 okay and
claims it has an Sv39 MMU, and both are false: the S7 has no MMU and no S-mode.

With `status` telling that lie, hart 0 came up startable, and the kernel handed it to
`sbi hart_start`. Vendor OpenSBI died on it. The failure was an M-mode load access fault at
OpenSBI's own scratch area, with `mepc` inside OpenSBI, reported for hart 0, immediately after our
bring-up call. That is what a boot ending in an OpenSBI trap dump looks like when `mepc` is in
firmware and the hart is 0. The kernel code that caused it is a `hart_start` the roster should
never have issued.

The one truthful property on that node is the ISA string itself, and it answers by omission.
`rv64imacu` is the old spelling that lists privilege letters in the single-letter run. It spells
`u` (user) without `s` (supervisor). The U74s beside it say `rv64imafdcbsux`, four single letters
`b s u x` at the tail, with `s` present. A hart without S-mode cannot run this kernel, whatever the
rest of its node claims. So since 2026-08-14 startability requires supervisor mode. It is read from
the hart's own `riscv,isa` (`isa::riscv64::supervisor_mode_claim`, enforced in
`smp::read_cpu_list`). Such a hart is likewise kept out of the machine record's intersection and
`mmu` (`isa::riscv64`). The boot line names the exclusion in the machine's own terms: "cpu 0's
riscv,isa names user mode and not supervisor".

The rule needs a witness, and this is the part worth remembering before generalizing it. A missing
`s` alone proves nothing. Modern ISA strings spell no privilege letters at all: Linux rejects them,
and QEMU dropped `s`/`u` in 5.1. So QEMU `virt` today says `rv64imafdch_...`. The mainline jh7110
dtsi says `rv64imac_zba_zbb`/`rv64imafdc_zba_zbb`, silent about privilege on machines that have
S-mode. Absence of `s` is a denial only when a bare `u` in the same single-letter run proves the
writer was spelling privilege modes. Otherwise it is silence, and silence is not evidence of
absence. Multi-letter `_s`-prefixed extensions (`_sstc`, `_svadu`; QEMU's own string is full of
them) never count. Only the run before the first `_` is scanned.

It is host-proven on both generations of spelling and both JH7110 trees
(crates/machine_discovery/tests/riscv64_isa_strings.rs, riscv64_jh7110.rs,
riscv64_jh7110_vendor.rs). The mainline fixture's S7 is excluded by `status`, the vendor fixture's
by its ISA string. The same conclusion arrives through the two trees' different lies.

## Third bench stop: the online set is {1,2,3}, and the kernel indexed it as {0,1,2}

With the S7 refused, the machine's online cpus are harts 1..3. Hart 4 was past `MAX_CPUS`, then 4;
that was fixed later that day (see [bench-boots-2026-08.md](bench-boots-2026-08.md)). It was the
first time this kernel ever ran with a set not contiguous from zero. On QEMU `virt` the set is
always {0..n-1}. So every `0..online_count()` loop and every `rng % online_count()` pick had been
right by coincidence.

On the board, spawn placement's modulo-count produced index 0. It placed `init` into parked slot
0's inbox, which nothing drains, ever. It took three boots to pin. The placement is randomized, so
roughly one placement in three landed dead, and the symptoms disagreed with each other. One boot
showed outlaw's 3-then-0 syscall counts; the next two showed `init` hanging outright. The
thread-dump diagnostic added for it (commit `3833422`, watching the threads while the demo waits)
showed the shape in one line: a parked core's inbox holding a runnable thread.

It was fixed on the critical path in commit `1329874` (`smp::online_cpus()` / `nth_online`, with
placement and steal converted). The follow-up branch swept the remaining count-as-index sites: wake
targeting, both ISAs' IRQ-affinity round-robins, the hang watchdog's liveness scan and the suite's
own per-core loops. The {1,2,3} shape is host-proven in `crates/cpu_set`, since QEMU cannot boot
it.
