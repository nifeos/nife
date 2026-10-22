# The first pass, 2026-09-14: what was built and how it was proved

An appendix of [milestone 268 (every architecture boots the same way)](../0268-the-boot-ladder.md).
The main page keeps each item, each finding and the `builder` outcome in summary and is complete
without this file. This one keeps the transcripts, the per-item detail and the `builder` reasoning
as the lane wrote them.

The block was PARTIAL that day, and deliberately so. Every rung below the last one landed on all
three architectures on 2026-09-14. The last rung was reached on aarch64 and riscv64 and was still
gated on `x86_64`. That is exactly what the block's own gate line said would happen, so the gate
was live rather than stale and the status had to admit it. See [what `x86_64` could not
reach](#what-x86_64-could-not-reach-and-why).

## The proof condition, answered

> *`--until` reaches the self-test verdict on all three architectures, and the same failure injected
> into any one of them turns the verdict red on all three and fails CI.*

Both halves, measured on 2026-09-14:

```
$ script/boot-check
boot-check (aarch64): reached self-test verdict (2404 bytes in 2.4s)
boot-check (aarch64): nife machine: aarch64, 4 processor(s), 256 MiB, 100 Hz
boot-check (aarch64): nife self-test: 5 of 5 passed
boot-check (riscv64): reached boot tour complete (6862 bytes in 2.8s)
boot-check (riscv64): nife machine: riscv64, 4 processor(s), 256 MiB, 100 Hz
boot-check (riscv64): nife self-test: 5 of 5 passed
boot-check (x86_64): reached self-test verdict (6182 bytes in 2.9s)
boot-check (x86_64): nife machine: x86_64, 1 processor(s), 254 MiB, 100 Hz
boot-check (x86_64): nife self-test: 5 of 5 passed
boot-check: every architecture reached the self-test verdict and it was green
$ echo $?
0
```

And red, with `self_test_injection` made a *default* feature so that the ordinary gate met it
(rather than the `--inject` mode, which expects red and would have proved only itself):

```
boot-check (aarch64): nife self-test: 4 of 5 passed, 1 FAILED: exceptions
boot-check (aarch64): FAILED. The default boot did not reach a green self-test verdict. ...
boot-check (riscv64): nife self-test: 4 of 5 passed, 1 FAILED: exceptions
boot-check (riscv64): FAILED. ...
boot-check (x86_64): nife self-test: 4 of 5 passed, 1 FAILED: exceptions
boot-check (x86_64): FAILED. ...
$ echo $?
1
```

Exit 1 is what fails CI: `boot-check` is a `local` row in `script/ci-build`'s table (milestone 286
(one enumeration of the checks that gate a pull request)), and CI's test job names it beside `test`
and `swish-check`. The verdict has been seen red on all three, which is the half of a gate that
usually never gets checked.

And through the other reader too. The proof condition names `--until`, which is the bench tool
rather than the gate, so it was checked there as well: `cargo xtask board-console --until selftest`
reaches the verdict on all three captures and exits 0, and on the three injected captures it reports

```
board-console: failed: the kernel's boot self-test failed on this machine: exceptions. The boot
continued to userspace anyway (it reports, it does not gate), so the board is up and degraded
rather than dead
board-console: self-test: nife self-test: 4 of 5 passed, 1 FAILED: exceptions
```

and exits 1. The two readers are the same recognizer, which is the point: a gate that used a second
reader would be gating something a bench run does not measure.

## The six items

1. The machine description answers the eight questions on all three. `print_machine_description`
   stays in `kernel/src/main.rs`, because milestone 267 (the milestone tour is three things wearing
   one name)'s lint reads it there by name. It is now called from every architecture's arm rather
   than only from the shared aarch64 path. Processors, memory, console, interrupt controller, timer
   rate, initrd, PCIe window, IOMMU, each in its own vocabulary. The interrupt controller is a
   GICv2's two register blocks on aarch64, a PLIC and its S-mode context on riscv64, the
   local-APIC/IO-APIC pair on `x86_64`; the IOMMU is SMMUv3, riscv-iommu and VT-d. Four new
   per-architecture summaries were written for it (`arch::irq::print_summary`,
   `arch::iommu::print_summary`) plus one portable one (`console::print_summary`).
2. `self_test` collects the set and runs it on all three. `kernel/src/self_test.rs`, five
   checks: `exceptions`, `mapping`, `frames`, `timer`, `scheduler`. Two of them were already here
   and uncollected, exactly as this block said: `arch::exceptions::self_test` (riscv64 and
   `x86_64` had it, aarch64 now does) and the RISC-V tour's unnamed `kmap test`. Both were removed
   from the tour and gained two architectures.
3. One verdict line, `nife self-test: 5 of 5 passed` / `..., 1 FAILED: <names>`. Wording
   provisional, as the block's `BUGS` instructed.
4. Nothing halts by default on riscv64 either. Its tour used to end in `arch::halt()`; it now
   ends in `riscv_hand_over`, the same call `--features shell` makes, so the default boot ends at a
   `swish` prompt. aarch64 already did this. Transcript of a riscv64 default boot with an archive:
   `nife: the capability core runs on RISC-V.` then `nife: handing the system to the userspace
   progenitor.` then `nife capability shell.` and a `$ ` prompt.
5. `board_console` learned the ladder. Three new stages, `Machine` -> `SelfTest` -> `Prompt`,
   and a failed verdict is `Failure::SelfTestFailed`, not a stage, so a degraded board does not
   read as a good one. `cargo xtask board-console --until machine|selftest|prompt`. Eight new host
   tests.
6. CI fails on a red verdict: `script/boot-check`, a `local` row in `script/ci-build`'s table
   (milestone 286 (one enumeration of the checks that gate a pull request)), named by CI's test job
   beside `test` and `swish-check`.

## `crates/boot_ladder`, which was not in the plan

The rungs are strings two binaries agree on, so AGENTS.md rule 7 makes them a crate. That is not a
formality here: the one marker that existed before was a literal inside the RISC-V arm of
`main.rs` and a second copy of it inside the recognizer, which is finding 3's mechanism. Three
binaries now read one definition: the kernel, `swish`, and `board_console`. Name provisional.

## A fifth finding, the same shape as finding 3

`Stage::Banner` was unreachable on aarch64. It matches `nife on `, deliberately generic so that
"a recogniser that only knew the VisionFive 2's would report a healthy aarch64 or x86_64 board as
never having booted" (`progress.rs`, written in anticipation). aarch64 printed no such line at all:
its description opened with a bare `nife`. So the rung below the one this milestone was written
about had the same defect, found the same way, by looking for the marker rather than by anything
failing. aarch64 now prints `nife on aarch64 (EL1, MMU off: physical addresses until mmu::init)` as
the first line after `console::init`, which is where the other two have always printed theirs.

Two smaller ones, both fixed:

- `arch::riscv64::mmu::print_summary` was `unimplemented!()` and had never had a caller. Calling
  the same name on all three architectures found it on the first run, as a `[PANIC]` in the middle
  of the block it had just printed.
- `arch::x86_64::exceptions::self_test` returned the cumulative breakpoint count where the other
  two return a delta. Harmless while it had one caller; the second caller reported `2`.

## What `x86_64` could not reach, and why

The prompt. This block gated exactly that and nothing else, and the gate held: `x86_64` has no
entry point that hands the machine to a shell, so its boot still ends in `nife x86_64: boot
complete, halting.` after the self-test verdict. Both halves are needed and neither is this
milestone's. One is §149 (may the kernel answer on an endpoint), for how `swish` reaches a console
where §121 (port I/O as a capability) leaves no userspace holder. The other is milestone 182
(x86_64's interactive boot), for the entry point. `crates/serial_console`'s `Stage::Prompt` says so
in its own doc rather than leaving a reader to discover it from a watch that times out, and
`script/boot-check` says so in its `BUGS`.

Every rung below it landed on `x86_64` on the same terms as the other two.

## calef's ruling on `components/src/builder.rs` (2026-09-14), and what this lane did with it

calef, mid-lane: he expects `builder` to go away as a result of this milestone. The claim it carries
(*userspace, not the kernel, composes a process*) becomes one of item 2's collected self-tests,
running on all three architectures instead of one. Its name is parked for that reason: *"Skip this
one because it will go away with parity"*, so the provisional `process_builder` in its provenance
block is not settled and was not touched.

This lane did not remove it, and the reason is the ruling's own condition rather than scope. The
ladder subsumes `builder`'s claim on riscv64 and can show it. Item 4 made that architecture's
default boot hand over, so the same boot that runs `builder`'s `init/build` step now also watches
the progenitor at work. The progenitor loads, measures and composes the console, the line
discipline, the input driver and `swish` out of its own budget, and offers a prompt. That is the
same claim at a larger scale, on the only architecture that ever ran `builder`.

What is not true yet is "on all three". A compose-a-process self-test needs a compiled ELF to run at
user level, and `x86_64` has none: `user_rt` has no `x86_64` arms, so that architecture runs
hand-assembled children and cannot load a program by name. Removing `builder` today would relocate
the claim on riscv64 and leave `x86_64` where it already is, which is *not* a loss, but it would
also close the door on the version calef actually described. And it is not free: the trust root
(`kernel/src/trust.rs`), the measured-boot manifest (`xtask`'s `boot_programs`), the archive table,
and `board_console`'s `userspace_ran` with its captured board transcript all name it.

So it was proposed rather than performed, and the proposal is
[milestone 295 (retire `components/src/builder.rs`)](../0295-retire-the-builder-program.md) now. The
one-sentence decision calef could make: whether the retirement waits for an `x86_64` leg that can
run a compiled ELF, or goes ahead now on the strength of the progenitor carrying the claim wherever
a progenitor runs. He made it the same day, *"Retire builder"*, and milestone 295 performed it.

## The handoff as Follow-on recorded it

Milestone 295 is the proposal file promoted in place. calef answered the one-sentence question it
was written to ask with *"Retire builder"*, option (b), and the removal landed on 2026-09-14. What
follows is the block's own account of the handoff, kept as written except that the file it named
now carries a number and its long sentences are split.

Retire `components/src/builder.rs`. calef ruled on 2026-09-14 that it should go away as a result of
this milestone, because the claim it carries becomes one of item 2's collected self-tests on all
three architectures. Its name is parked for that reason, so the provisional `process_builder` in its
provenance block is not settled and was not touched. This lane did not perform the removal. The
ladder subsumes the claim on riscv64 and can show it: item 4 made that boot hand over, so the same
boot now watches the progenitor compose the whole system from its own budget. But a
compose-a-process self-test on all three needs a compiled ELF to run at user level, and `x86_64` has
none (`user_rt` has no `x86_64` arms). So the retirement waits on either that leg or a one-sentence
ruling that the progenitor carrying the claim wherever a progenitor runs is enough. The work when it
happens: drop the tour's `init/build` step and `kernel::user::riscv_initrd_demo`, drop `builder`
from `xtask`'s `boot_programs` and the archive table, and delete the program. Then give
`crates/serial_console` a live marker in place of `userspace_ran`'s `init/build`, since the captured
board transcript keeps needing the old one.

Correction, 2026-09-14 (milestone 182's lane): the premise that `x86_64` "has none" was false by the
time it was written into this paragraph and the section above. `crates/user_mode_runtime` has had
`x86_64` arms since milestone 161 (the x86_64 kernel port). That is why the x86 archive packs every
portable program and `std_exerciser` ran there at milestone 184 (std on x86_64). The same day the
progenitor itself was loaded from the archive and ran at ring 3 on `x86_64`. The two passages are
kept as the record of the reasoning; the retirement they discussed went ahead anyway.
