---
status: PROPOSED
raised: 2026-09-19
---

# 186. Where a riscv64 tour-boot check runs, what it asserts, and what it is called

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 406 (nothing on a pull request boots the riscv64 tour)'s
`DECISION` gate naming no section. *(Section number provisional until the merge queue lands it.)*

## What is being decided

Three things, and none of them is the wiring, which is a lane's:

1. Where the check runs. A CI job of its own, a row in `script/ci-build`'s table, or a
   `script/cadence-check` entry. It boots a kernel and it is not free.
2. What it asserts beyond the floor of reaching `Stage::Tour` with `userspace_ran()`.
3. Its name, since it is a new `script/` entry point or a new `xtask` verb.

## Is the premise true, and the title's half that is not

Milestone 406's title claim is false as written and its narrower half is true, which the block
records in its own status paragraph. Retitling it is not this section's and not a lane's.

Checked 2026-09-19 in this worktree:

- `script/boot-check` exists, sits in `script/ci-build`'s `local` tier at line 114, and runs on every
  pull request. So the default riscv64 kernel is booted on a pull request.
- `script/boot-check`'s own header, line 30, says "No initrd and no disk: every rung this reads is
  printed before userspace exists." It asserts the banner, that the machine description printed to
  its end, and the self-test verdict.
- `crates/serial_console/src/progress.rs:26` defines `Stage`, whose `Tour` variant is documented as
  one architecture's rung, riscv64's, printed only by the milestone-tour build.
  `BootProgress::userspace_ran` (line 323) is set from the `init/build` line.

So what is unasserted is the boot as a sequence past the self-test: `Stage::Tour` with
`userspace_ran()`, the userspace-builder step, the preemption claim, the UART driver's device IRQ,
the virtio and PCIe probes, and the hardware-entropy line. Those are checked today by a person
remembering to type `script/soak-test`, which is rung four holding a claim `design/fatal-risks.md`
cites.

## What this tree already does in the analogous case

The recognizer already exists and is tested, so this is a caller rather than an instrument.
`crates/serial_console`'s `Progress` ratchets the stages, recognizes failure markers, and
`script/soak-test` already judges a QEMU run with it. The missing piece is a caller that boots the
default kernel with `-initrd` under `helpers/qemu-bounded.sh` and asks whether it reached
`Stage::Tour` with `userspace_ran`.

Two prior rulings pull in opposite directions on placement, which is why this is a decision:

- `script/swish-check` and `script/boot-check` were both put in `script/ci-build`'s table, for
  the same "reuse the build" reason, and that is the tree's recent habit.
- §74 ruled the other way for audits: event triggers first, a count second, the calendar only as
  a backstop, on the reason that "eventually is the wrong word" for something that matters. And
  milestone 232 refused to assume CI is the answer for an expensive instrument, which is the
  posture that applies here unchanged.

**And `script/boot-check`'s own BUGS is the shape to copy rather than repeat.** It records that it
does not check the prompt, that `script/swish-check` reaches one on two of three architectures, and
that asserting it on two of three would be the defect milestone 268 existed to fix. A tour check is
riscv64-only by construction, so it meets the same objection and has to answer it out loud.

## What it costs, measured

The boot is one `cargo xtask` verb over machinery that is all present: `initial_ramdisk_riscv()`,
`helpers/qemu-runner-riscv64.sh`, `crates/serial_console`. **A bounded tour boot is seconds, not
minutes**, because the tour halts on its own; the cost is the riscv64 kernel build, which
`script/ci-build` already pays for other reasons.

So the marginal cost of the `script/ci-build` placement is close to zero, and the marginal cost of a
separate CI job is a runner slot, which AGENTS.md names as a real ceiling.

## Recommendation

A row in `script/ci-build`'s table, on the measured ground above: the expensive part is the
build, the table already pays for it, and a separate job buys latency isolation this check does not
need. A cadence is the wrong shelf for something whose marginal cost is seconds.

On what it asserts: the floor and no more, at first. `Stage::Tour` with `userspace_ran()` is one
assertion over an existing recognizer. The device-IRQ and preemption lines are the two a reader
would expect a tour check to make next, and each costs a new `Progress` field, which is a change to
a crate two callers read (`script/soak-test` and `cargo xtask board-console`). Widening the
recognizer is a separate, later question.

On the name: no recommendation. It is a new entry point and names are calef's.

## The other two architectures, named so they are not discovered later

aarch64's equivalent of the tour is the `initboot` path and it is exercised, by
`script/swish-check`. x86_64 has neither a shell boot nor a tour step that loads userspace from an
archive, and `swish_check()` says so in its own comment. So this is riscv64-shaped on purpose, and a
general "boot every architecture's default kernel and read the transcript" is a larger thing that
should be argued for separately rather than assumed here.

## How reversible, and who has acted on it

The placement and the assertions are high (a table row, a shell script), **the name is not**,
because it is an entry point a contributor learns and `notes/scripts.md` lists.

## What is blocked until this is answered

Milestone 406. The gap is real and unpoliced meanwhile: milestone 289 was minted on the
reasonable suspicion that `components/src/builder.rs` was vestigial, and every signal that would
have said otherwise was outside CI. A step nothing asserts and a step nothing needs produce the same
evidence, and this tree has already spent one lane telling them apart.
