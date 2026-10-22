---
status: NOT-STARTED
raised: 2026-09-14
milestone_dependencies: none
decision_dependencies: 186
machine_requirements: none
specific_machine: none
needs_person: no
---
# 406. Nothing on a pull request boots the riscv64 tour, so its userspace step is unasserted

Not started, and **the title's claim is no longer true as written**, which is the first
thing a reader needs. Filed 2026-09-14 as an unnumbered proposal by milestone 289 (RISC-V tour)'s
lane. Numbered
2026-09-19 by milestone 433's drain of the proposal pile, with the premise re-read the same day.
What closed, and it closed within hours of this being written: `script/boot-check` was added by
milestone 268 on **2026-09-14**, sits in `script/ci-build`'s `local` tier (line 114) and runs on
every pull request in the same job as `test` and `swish-check`.
`.github/workflows/ci.yml` names the gap in its own words, that boot-check *"is the only thing on a
pull request that boots the DEFAULT riscv64 or `x86_64` kernel at all"*. So the default riscv64
kernel **is** booted on a pull request, by the recognizer this block said to reuse, and the sentence
under "The claim in one line" is false. What is still true is the narrower half, and it is the
work. `script/boot-check` boots with no initrd and no disk, by design, and says so. Every rung
it reads is printed before userspace exists, so it stops at the machine description and the
self-test verdict. Nothing on a pull request reaches `Stage::Tour` or asserts `userspace_ran()`. The
userspace-builder step, the preemption claim, the UART driver's device IRQ, the virtio and PCIe
probes and the hardware-entropy line are still asserted by a person remembering to type a command.
The table below is accurate for every row it lists and is missing a row for `script/boot-check`.
Two names in its text have moved and are corrected in place. `script/gates` was retired by
milestone 286 (one enumeration of the checks), its work now being `script/ci-build`'s table, which
is where `boot-check` went.
`script/soak` is `script/soak-test` since milestone 297.
*(Number provisional until the merge queue lands it.)*

The decision is
§186 (where a riscv64 tour-boot check runs, what it asserts, and what it is called), written up
2026-09-19 by milestone 435's slice-c lane because this gate named no section. **That lane judged
the gate and not the title**. The title's claim is false as written, the status paragraph above says
so, and retitling a block is an architect's rather than a sweep's.
Where the check goes is an architect's: a CI job, a row in `script/ci-build`'s table,
or a `script/cadence-check` entry. It boots a kernel and it is not free. Milestone 232 (audit
every check) refused to assume CI is the answer for an expensive instrument, and that refusal
applies here unchanged. The wiring and the
measurement are a lane's; the placement is not.

## The claim in one line

The **default** riscv64 kernel, which is the build `script/board-image` puts on a card, is booted by
nothing that runs on a pull request.

## What each riscv64 check actually boots, traced rather than assumed

| check | build | reaches the tour? |
|---|---|---|
| `script/test` (riscv64 leg) | `#[cfg(test)]` | No. `main.rs`'s test arm runs `test_main()` and exits via semihosting **before** the tour block. |
| `script/cpu-matrix` | the same test build, five CPU models (`rv64 sifive-u54 rva22s64 rva23s64 thead-c906`) | No, same arm. |
| `script/swish-check` (riscv64 leg) | `--features shell` | No. `boot_progenitor` runs (through `riscv_hand_over`) and the boot thread leaves the scheduler before the tour. |
| `script/bench --riscv --check` | `--features bench` | No. `bench::run()` parks before the tour. |
| `script/icount` | `--features icount` | No, same shape. |
| `script/soak-test --arch riscv64` | `--features soak_test` | **Yes**, whole tour, then the workload. Not in CI. |
| `script/job-mix --arch riscv64` | `--features job_mix` | **Yes**, same shape. Not in CI. |
| `script/board-image` | `board` | **Yes**, on a board. Not in CI, and not in QEMU. |

Both `bench::run()` and `icount::run()` are `-> !` and end in `arch::halt()`, so those two are not
"skips the rest" by accident; they cannot reach the tour by construction. The `#[cfg(test)]` arm ends
`test_main(); arch::halt();` for the same reason.

So every step of that boot is asserted by nothing on a pull request: the userspace-builder step
(`init/build`), the preemption claim, the UART-driver step's device IRQ, the virtio and PCIe probes,
the hardware-entropy step's line, and the final banner. Some of what they demonstrate is covered
independently by the riscv64 kernel suite, which is why this is a gap rather than a hole. What is not
covered is the boot as a sequence, which is exactly what a board produces and what
`crates/serial_console` was written to read. They are checked today by a person remembering to type
`script/soak-test`, which is AGENTS.md's rung four holding a claim `design/fatal-risks/README.md` cites.

## Why this is worth a milestone rather than a patch

It is what made a live step look dead. Milestone 289 was minted on the reasonable suspicion that
`components/src/builder.rs` was vestigial. Every signal that would have said otherwise was outside
CI: a crate's host tests over captured board logs, two `script/` tools nobody runs on a branch, and a
bench note. A step nothing asserts and a step nothing needs produce the same evidence, and this tree
has already spent one lane telling them apart.

The recognizer already exists and is tested. `crates/serial_console` parses this exact transcript:
`Progress` ratchets `spl`/`opensbi`/`uboot`/`handoff`/`banner`/`tour`, recognizes five failure
markers, and sets `userspace_ran` from the `init/build` line. `script/soak-test` already judges a QEMU run
with it. So the missing piece is a caller that boots the default kernel with `-initrd` under
`helpers/qemu-bounded.sh` and asks `Progress` whether it reached `Stage::Tour` with `userspace_ran`,
not a new instrument.

## What it would take, priced

The boot itself is one `cargo xtask` verb over machinery that is all present: `initial_ramdisk_riscv()`,
`helpers/qemu-runner-riscv64.sh`, `crates/serial_console`. A bounded tour boot is seconds, not
minutes, because the tour halts on its own; the cost is the riscv64 kernel build, which
`script/ci-build` already pays for other reasons.

The open questions are an architect's and are the reason this is not just done:

- Where it runs. A CI job of its own, an arm of `script/ci-build` (which is where
  `script/swish-check` and `script/boot-check` were both put, for the same "reuse the build"
  reason), or a cadence.
- What it asserts. Reaching `Stage::Tour` with `userspace_ran()` is the floor. The device-IRQ and
  preemption lines are the other two claims a reader would expect a tour check to make, and asserting
  them means recognizing them, which is two more `Progress` fields.
- Its name, since it is a new `script/` entry point or a new xtask verb, and names are an
  architect's.

## The aarch64 and x86_64 halves, named so they are not discovered later

aarch64's equivalent of the tour is the `initboot` path and it *is* exercised, by
`script/swish-check`. x86_64 has neither a shell boot nor a tour step that loads userspace from an
archive, and `swish_check()` says so in its own comment. So this proposal is riscv64-shaped on
purpose, and a general "boot every architecture's default kernel and read the transcript" is a larger
thing that should be argued for separately rather than assumed here.

## Index row

Milestone 289 was minted on the reasonable suspicion that `components/src/builder.rs` was vestigial,
and found instead that a step nothing asserts and a step nothing needs produce identical evidence.
The riscv64 tour is the case: every riscv64 check on a pull request parks before it, because the
`cfg(test)` arm exits through semihosting and `bench::run()` and `icount::run()` are `-> !` and end
in `arch::halt()`, so they cannot reach it by construction. Half of that closed the same day this
was written: milestone 268's `script/boot-check` boots the default kernel on all three
architectures in `script/ci-build`'s local tier, which is the caller this block asked for. It boots
with no initrd, so it stops at the machine description and the self-test verdict, and what is still
asserted by nobody is the boot as a sequence past that point: `Stage::Tour` with
`userspace_ran()`. That is what a board actually produces and what `crates/serial_console` was
written to read. The recognizer exists and is tested, `Progress` already ratchets the stages and
sets `userspace_ran` from the `init/build` line, and `script/soak-test` already judges a QEMU run
with it. So the missing piece is an initrd and two assertions rather than an instrument. What is
calef's is where it runs, what it asserts beyond the floor, and its name. The aarch64 and x86_64
halves are named rather than left to be discovered: aarch64's equivalent is exercised by
`script/swish-check`, and x86_64 has neither a shell boot nor a tour step that loads userspace from
an archive.
