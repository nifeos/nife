---
status: BUILT
raised: 2026-09-09
built: 2026-09-19
---
# 268. Every architecture boots the same way: describe the machine, test yourself, hand over

Every rung is built on all three architectures, and the top rung is gated; see
[the third pass](#third-pass-2026-09-19-the-top-rung-gated).

The ladder was decided (calef, 2026-09-09, in conversation), and nothing in it was a design fork.

This page is complete on its own. The working behind it is in four appendices, indexed in
[0268-the-boot-ladder/README.md](0268-the-boot-ladder/README.md):

- [findings.md](0268-the-boot-ladder/findings.md): the four findings in full, and the naming.
- [first-pass.md](0268-the-boot-ladder/first-pass.md): the 2026-09-14 build and the `builder`
  ruling.
- [second-pass.md](0268-the-boot-ladder/second-pass.md): `x86_64` stops halting; two BUGS closed.
- [third-pass.md](0268-the-boot-ladder/third-pass.md): the prompt rung, its cost, and the PVH
  choice.

## What calef decided, so a lane does not re-litigate it

- The machine description prints on every architecture, in the kernel, before userspace. It is
  what you need to bring up new hardware, which is exactly when there is no userspace to run a
  program from. Same display everywhere even though the backing differs.
- Nothing halts. The default boot ends at the `swish` prompt, and the prompt is the signal that
  the boot finished. This replaces the halt as the terminal state.
- The boot self-tests come between the two: describe the machine, prove the kernel works on it,
  then hand over.
- The self-test reports; it does not gate. A failure prints its verdict and the boot continues to
  the prompt anyway, because *"I could see building tools to diagnose what broke but we can only run
  them if there is a prompt"* (calef). A machine you cannot log into is a machine you cannot fix.
- The verdict is not part of `swish`. The console is how this system reports to its operator,
  whether that operator is a person, an agent or a harness, and unread lines on a terminal are not a
  defect of that channel.
- `self_test` is ratified (calef, 2026-09-09) after the prior art turned out to be in this tree
  already.
- Parity is reached by leveling up, never by leveling down (calef, 2026-09-09):
  *"drive parity by not degrading functionality away from where we want to get but rather by driving
  everything towards where we want to go."* There is a specific trap in this milestone; see BUGS.

## The four findings this is built on

Each was measured on 2026-09-09; [findings.md](0268-the-boot-ladder/findings.md) has the detail.

1. `machine.rs` existed on x86_64 only. The other two printed the same device-tree facts scattered
   through `main.rs` and had no machine module at all.
2. aarch64 had no `exceptions::self_test`; riscv64 and x86_64 did, plus an unnamed kernel mapping
   check. The concept and the name were already in the tree, uncollected.
3. `Stage::Tour` could never fire on aarch64 or x86_64. Its one marker string lived in the RISC-V
   arm, so `--until tour` was unreachable there, and a missing marker looks exactly like a slow
   board.
4. The tour's checks printed `FAILED` and continued, reaching the same end state either way. That
   is rung four of AGENTS.md's ladder, and it is why finding 3 hid for months.

The name `self_test` was the tree's before the question was asked: two architectures already ran
`arch::exceptions::self_test()` at boot. It does not collide with `test`, which means the suite.
`POST` is Dell's, on xenon's own firmware screen.

## What to build

1. `machine` on all three architectures, in the kernel, before userspace. The parity claim is the
   same questions answered, not the same lines printed, because one machine has ACPI and two have a
   device tree. The required set: processors, memory, console, interrupt controller, timer rate,
   initrd, PCIe window, IOMMU. Each architecture answers in its own vocabulary; each answers all of
   them or says plainly that it cannot.
2. Collect the existing self-tests behind one `self_test`, give aarch64 the `exceptions` one it
   lacks, and run the set on all three.
3. One verdict line, and it is a contract. Stable text, identical on all three architectures,
   because its audience includes `board_console` and CI. The tour's failure was never that a human
   missed a line. The only matchable string lived in one architecture's arm, so on the other two
   there was no signal on the channel at all.
4. Nothing halts by default. The boot proceeds to userspace and to the `swish` prompt.
5. `board_console` learns the new ladder: `Handoff` -> `Banner` -> machine -> self-test verdict
   -> prompt. A failed verdict is an outcome distinct from a clean boot, so a degraded board on the
   bench does not read as a good one.
6. CI fails when a boot self-test reports a failure. With the boot no longer gating, this is the
   mechanism that keeps the verdict honest; without it this milestone recreates finding 4 with
   better prose.

The measurement builds keep parking before userspace, and that must not change. `bench`, `icount`,
`soak` and `jobmix` all deliberately stop short of it. `kernel/Cargo.toml`'s feature comments give
the reason: a board has no command line, so these are compile-time or they are nothing. "Nothing
halts" is about the default boot and only about it.

The proof: `--until` reaches the self-test verdict on all three architectures, and the same failure
injected into any one of them turns the verdict red on all three and fails CI. Not a survey of what
each arm prints, and not a marker that exists on one architecture, which is the defect being fixed.

## BUGS (as planned)

- The leveling-down trap. `arch::machine::attach_screen` is an x86-only kernel path, and a naive
  parity reading says delete it. It is the only thing in this system that puts pixels on real
  hardware, and xenon's monitor is the one working display we have. The other two grow a display
  path (milestone 157 (the U-Boot framebuffer handoff)); x86 does not lose one.
- Item 6 is the whole mechanism and the piece most likely to be dropped as "follow-up", which
  would ship finding 4 again under a ratified name. It was built.
- The required question set in item 1 is asserted rather than derived, from x86_64's existing
  output. Nobody has checked it against what a person actually wants when bringing up an unfamiliar
  board.
- The block did not say what the verdict line reads. That is a contract two programs agree on, so it
  is an architect's under the *move fast on what can be undone* tenet. The lane shipped a
  provisional wording and said so.

## What was built

### First pass (2026-09-14): every rung below the prompt

[first-pass.md](0268-the-boot-ladder/first-pass.md) has the transcripts. `script/boot-check`
reached a green verdict on all three architectures and, with the failure injected as a default
feature, a red one on all three with exit 1, which fails CI. `cargo xtask board-console --until
selftest` gave the same answers, because the bench tool and the gate are the same recognizer.

1. The machine description answers the eight questions on all three, each in its own vocabulary.
   `print_machine_description` stays in `kernel/src/main.rs`, where milestone 267's lint reads it.
2. `kernel/src/self_test.rs` runs five checks on all three: `exceptions`, `mapping`, `frames`,
   `timer`, `scheduler`. The two that were already in the tree left the tour and gained two
   architectures.
3. The verdict line is `nife self-test: 5 of 5 passed` / `..., 1 FAILED: <names>`. Wording
   provisional.
4. riscv64's default boot hands over to a `swish` prompt rather than halting, as aarch64 already
   did.
5. `board_console` gained `Machine` -> `SelfTest` -> `Prompt`, and a failed verdict is
   `Failure::SelfTestFailed`, not a stage. Eight new host tests.
6. CI fails on a red verdict through `script/boot-check`, a `local` row in `script/ci-build`'s
   table (milestone 286 (one enumeration of the checks that gate a pull request)).

The rungs are strings two binaries agree on, so AGENTS.md rule 7 made them a crate,
`crates/boot_ladder` (name provisional), read by the kernel, `swish` and `board_console`. A fifth
finding had finding 3's shape: `Stage::Banner` was unreachable on aarch64, which printed no
`nife on ` line; it does now. Two smaller defects were fixed on the way: riscv64's
`mmu::print_summary` was `unimplemented!()`, and x86_64's `exceptions::self_test` returned a
cumulative count rather than a delta.

`components/src/builder.rs` is retired, its claim one of the collected self-tests (calef,
2026-09-14: *"Retire builder"*). [Milestone 295 (retire
`components/src/builder.rs`)](0295-retire-the-builder-program.md) performed it.

### Second pass (2026-09-14): `x86_64` stops halting

Built in one lane with milestone 182; [second-pass.md](0268-the-boot-ladder/second-pass.md) has
the transcripts. The `x86_64` default boot hands the machine to the progenitor, which reaches ring
3 and builds the system. It could not yet offer a prompt, since its console server could not reach
port I/O under §121 (port I/O as a capability), and it said so instead of going quiet. Three gates
keyed on the old halt line now wait for `boot_ladder::SELF_TEST`. Two BUGS closed:

- "The self-test can hang the boot." `timer` and `scheduler` read through `self_test::Counter`,
  which reports a counter that read one value a million times running, and the injector now stops
  the counter, so every `--inject` run gates this bound.
- "Nothing proves an architecture ran the right five checks." `boot_ladder::SELF_TEST_CHECKS` is
  the one list; the kernel names a listed check that never ran, and `board_console` fails a verdict
  whose total is not the list's length.

### Third pass (2026-09-19): the top rung, gated

`script/boot-check` asserts the `swish` prompt on all three architectures;
[third-pass.md](0268-the-boot-ladder/third-pass.md) has the transcript. Each leg packs its
archive first, because the packer writes the measurement manifest `kernel/build.rs` reads.
`--inject` still comes back red and stops at the verdict.

The cost was measured before it was chosen: the whole gate went from 19.4 s to 25.6 s, all of it
packing archives. The `x86_64` leg boots the PVH `-kernel` image, not the UEFI one, which
`script/swish-check` boots under OVMF in about six minutes. This gate reads and does not type, so
it does not need that fidelity. At equal cost the customer's image would be the better thing to
boot, so this is a decision made on cost and written down as one.

Two things were found and fixed. `host::cargo` leaked `NIFE_DISK` into the runners, turning two of
three legs red; the leg now sets the child's environment explicitly. `Stage::Prompt`'s doc still
called the rung unreachable on `x86_64`.

## Follow-on

- **Milestone 182.** x86_64's boot had nowhere to land until it had a shell. This milestone's rungs
  all run before userspace, so it did not wait, but the top rung was unreachable there until 182.
- **Decision.** `design/decisions/0149-kernel-served-console-endpoint.md`, §149 (may the kernel
  answer on an endpoint): how `swish` reaches a console on x86_64 where §121 (x86 port I/O) leaves
  no userspace holder.
- **Milestone 269.** `machine` as a program that can be run from the prompt.
- **Recorded.** The `attach_screen` asymmetry is named in BUGS rather than left for a lane to
  rediscover, because the wrong move is the obvious one.
- **Done.** The ladder's top rung on `x86_64`. x86's console is a userspace driver holding a
  port-range capability, as §149 (may the kernel answer on an endpoint) resolved. Milestone 299 (the
  x86 port-range capability) built that capability and the prompt with it. Milestone 182 (x86_64's
  interactive boot) added the third `script/swish-check` leg, which types 60 of 64 script lines at
  it.
- **Done.** `script/boot-check` asserting the prompt on all three architectures. See the third pass.
- **Done.** [Milestone 295 (retire
  `components/src/builder.rs`)](0295-retire-the-builder-program.md) retired `builder`;
  [first-pass.md](0268-the-boot-ladder/first-pass.md) keeps the handoff.
- **Milestone 409.** Trim the duplicated bring-up narrative on riscv64 and `x86_64`. The constraint
  is the one that put those lines there: a `test` or `bench` boot compiles the description out and
  still has to say what machine it ran on.
- **Milestone 411.** A machine-description line for the framebuffer's geometry, which is what
  [milestone 411 (the machine description should say the screen's
  geometry)](0411-the-machine-description-should-say-the-screen-geometry.md) asks.
  `console::print_summary` reports a screen's address and length; the width, height and pixel order
  arrive in the handoff and are printed by the `x86_64` arm alone. A board with a monitor and no
  serial port is exactly the machine this block was written for.

## BUGS (as built)

- The machine description is printed twice over on riscv64 and `x86_64`: each arm narrates its
  bring-up (`isa`, `firmware`, `memory`, `pci`, `apic`), then the description answers the same
  questions in one block. Noise rather than a defect, and trimming is not free, because `test` and
  `bench` boots compile the description out. It is
  [milestone 409 (one machine description)](0409-one-machine-description-not-two.md).
- Closed 2026-09-14: "Nothing proves an architecture ran the right five checks." What remains is
  in `boot_ladder::SELF_TEST_CHECKS`'s doc: changing the set is one edit that review still has to
  judge.
- Closed 2026-09-14: "The self-test can hang the boot." What remains is a check whose *operation*
  never returns (a `yield_now` or a `map_page` that wedges), which needs a watchdog this module does
  not own; `kernel/src/self_test.rs`'s `BUGS` says so. `arch::timer::spin_for` keeps the
  stopped-counter exposure the self-test used to share.
- Closed 2026-09-19: "`script/boot-check` does not check the prompt." The planned fix was built:
  an archive on all three and the watch run to `Stage::Prompt`, once §149 was decided. The plan
  wrongly expected a second `script/swish-check`. That gate types and this one reads, so this one
  needs no disk and no typing bound.
- One emulator was found orphaned after about fifteen `x86_64` boots on 2026-09-19, reparented to
  `launchd` and still running half an hour later. The leg kills the runner's children before the
  runner, and that works on every watched run, so the mechanism is unknown and not guessed at. It
  is recorded at the kill site in `xtask/src/boot_check.rs`. The standing answer is AGENTS.md's:
  check `pgrep -l qemu` after a session and walk `ps -o pid,ppid` up before killing anything.
- The prompt rung is the shell's banner, not the `$ `. So `boot-check` asserts that `swish` started
  and printed, not that a prompt was offered or that anything could be typed at it.
  `crates/boot_ladder`'s BUGS has the reason: two bytes is too weak to key on after a kilobyte of
  hex. `script/swish-check` makes the stronger claim by typing, on all three since 2026-09-19.
- A riscv64 boot that wedges between its tour and its prompt takes the full 180-second cap to
  report. `watch::Policy::quiet_after` is suppressed from `Stage::Tour` up, and riscv64 alone prints
  its tour below the prompt. The suppression is right, since a default aarch64 or `x86_64` boot
  halting after the tour is a good boot; the cost lands on this gate's slowest failure case.
- The injected leg is not in CI. `script/boot-check --inject` rebuilds three kernels for one
  boolean, and it proves a property of the gate rather than of the change under test. It is run by
  hand when the self-test or the recognizer changes. A gate for the gate, left at rung four
  deliberately and said out loud.
- The `x86_64` tour's `kernel task` line can print `FAILED` on two cores. It was seen once on
  2026-09-14 under OVMF with `NIFE_SMP=2` (`a spawned thread ran and carried its captured state
  (0x0) FAILED`) and not on the run before. It waits a fixed eight `yield_now`s, and on two cores
  the thread can land on the other one, the VisionFive 2's lesson that `self_test::scheduler`
  applied by bounding on the clock. It gates nothing, and the `scheduler` self-test proves the same
  claim every boot. The fix is to give the tour line that bound or drop it as a duplicate; it is
  the tour's code, not this milestone's.
- `Stage::Tour` is still one architecture's rung, now documented as one. Leveling it up would give
  aarch64 and `x86_64` a marker for a tour they do not have; leveling it down would delete
  riscv64's, the trap this block warns about. The portable rungs that replace it for every tool are
  `Machine` and `SelfTest`.

## Index row

calef, 2026-09-09. Parity is the demonstrator's claim (§19) and three boot arms had quietly
diverged: `machine.rs` on one architecture, `self_test` on two, a `Stage::Tour` marker reachable
on one. Every architecture now describes its machine, runs the same five self-tests behind one
verdict line, and hands over to a `swish` prompt rather than halting; `script/boot-check` reads the
whole ladder with the same recognizer a bench run uses, and `--inject` proves it can come back red.
