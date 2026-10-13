# The boot ladder: what a nife boot says, in order, on every architecture

Milestone 268. A boot on this system climbs a fixed ladder, and each rung announces itself with one
line. Those lines are a contract: they are what `crates/board_console` matches on a bench and in
CI, and they are what a person reads off a photograph of a monitor when there is no serial port.

```
nife on aarch64 (EL1, MMU off: physical addresses until mmu::init)   <- the console works
...
nife machine: aarch64, 4 processor(s), 256 MiB, 100 Hz               <- discovery finished
nife self-test: 5 of 5 passed                                        <- the kernel works here
...
nife capability shell. naming a resource in a command IS granting it. <- userspace is up
$
```

The heads of those four lines live in one crate, `crates/boot_ladder`, because three binaries agree
on them (the kernel, `swish`, and `board_console`) and AGENTS.md rule 7 says that makes them a
crate. The tails carry numbers and are free to improve; a matcher keys on the head only.

## Why the ladder exists at all

Three boot arms had quietly diverged. `machine.rs` was on one architecture; `exceptions::self_test`
was on two; and `Stage::Tour`, the checkpoint the bench tooling relied on to mean "paging, traps,
the timer, the frame allocator, SMP and the scheduler all came up", was printed inside the RISC-V
arm of `kernel/src/main.rs` and nowhere else. So on aarch64 and `x86_64` there was no signal on
that channel at all, and a missing marker looks exactly like a slow board.

Milestone 268's lane found a fifth case of the same shape one rung lower: `Stage::Banner` matches
`nife on `, chosen generically so that "a recognizer that only knew the VisionFive 2's would report
a healthy aarch64 or `x86_64` board as never having booted" -- and aarch64 printed no such line at
all. The recognizer had anticipated parity that the thing it read did not have.

The general lesson, which is worth more than the fix: a marker that only one architecture prints
is indistinguishable from a marker that never fires, and neither one fails a build. The defense is
a shared definition (the crate) plus a gate that reads the channel (`script/boot-check`).

## The rungs, and who can reach each one

| Rung | Marker | aarch64 | riscv64 | `x86_64` |
|---|---|---|---|---|
| `Banner` | `nife on ` | yes | yes | yes |
| `Machine` | `nife machine: ` | yes | yes | yes |
| `SelfTest` | `nife self-test: ` | yes | yes | yes |
| `Tour` | `nife: the capability core runs on ` | no | yes | no |
| `Prompt` | `nife capability shell` | yes | yes | yes |

`Tour` is deliberately one architecture's rung and is documented as one. Leveling it up would
mean giving two architectures a marker for a demonstration tour they do not have; leveling it down
would delete riscv64's, which is the trap milestone 268's block warns about. The rungs that replace
it for every tool are `Machine` and `SelfTest`, which every architecture reaches.

`Prompt` needs an archive on every architecture, because the shell that prints it is loaded out
of one. On `x86_64`, DECISIONS §149 (may the kernel answer on an endpoint) settles how `swish` gets
a console, milestone 299 (the x86 port-range capability) built the capability that makes `console`
a userspace driver on that machine, and milestone 182 (x86_64's own interactive-boot entry point)
added the `script/swish-check` leg that types at it. `script/boot-check` asserts this rung on all
three architectures.

## The machine description: the same questions, not the same lines

`kernel/src/main.rs`'s `print_machine_description` answers eight questions on every architecture:
**processors, memory, console, interrupt controller, timer rate, initrd, PCIe window, IOMMU.** One
of these machines has ACPI and two have a device tree, so each answers in its own vocabulary: a
GICv2's distributor and CPU interface, a PLIC and this hart's S-mode context, a local-APIC/IO-APIC
pair. What is **not** allowed is a blank. An architecture that cannot answer says so in words,
because a missing line and a line nobody wrote look the same in a photograph.

It ends with `nife machine: ` rather than beginning with it, deliberately: reaching that line means
the whole block printed, which is the claim a ladder rung should make. A header would only mean the
block started.

Nothing in the body may be gated on a boot-mode feature, and `script/lint` checks that. The reason
is `xenon`: at first light on that machine there was no serial console this project could read, so
these lines *were* the transcript.

## The self-test: it reports, it does not gate

`kernel/src/self_test.rs` runs five checks between the description and the hand-off: `exceptions`
(a breakpoint is caught and stepped over), `mapping` (a fresh page is mapped, written, read back and
unmapped at an address computed from this machine's own RAM), `frames` (one frame out of the
allocator and back, with the accounting agreeing at both ends), `timer` (the counter advances and a
tick interrupt arrives), `scheduler` (a spawned kernel thread runs and carries its captured state).

**A failure prints and the boot carries on.** calef, 2026-09-09: *"I could see building tools to
diagnose what broke but we can only run them if there is a prompt"*. A machine you cannot log into
is a machine you cannot fix.

That is exactly why the verdict has to be read by something other than a person. The boot tour's own
checks have printed `FAILED:` and carried on since they were written, and nothing ever read those
either; that is how a checkpoint reachable on one of three architectures hid for months.

## EXAMPLES

Boot all three, require a green verdict, and require each one to reach a `swish` prompt (a `local`
row in `script/ci-build`'s table, so the no-argument path runs it before a push and CI's test job
names it). About 26 seconds against a warm target directory:

```
$ script/boot-check
boot-check (aarch64): reached shell prompt (3867 bytes in 2.9s)
boot-check (aarch64): nife machine: aarch64, 4 processor(s), 256 MiB, 100 Hz
boot-check (aarch64): nife self-test: 5 of 5 passed
...
boot-check: every architecture climbed the ladder to a shell prompt, and the self-test verdict on
the way was green
```

One architecture:

```
$ script/boot-check --arch riscv64
```

Prove the gate has teeth, by making one check fail and requiring the run to come back **red**:

```
$ script/boot-check --inject
boot-check (aarch64): nife self-test: 3 of 5 passed, 2 FAILED: exceptions timer
...
boot-check: every architecture's self-test verdict came back RED, as the injection asked
```

Watch a real board over a serial cable and stop as soon as it has self-tested:

```
$ cargo xtask board-console --port /dev/cu.usbserial-0001 --until selftest
board-console: reached self-test verdict (6862 bytes in 12.4s)
board-console: machine: nife machine: riscv64, 4 processor(s), 8192 MiB, 100 Hz
board-console: self-test: nife self-test: 5 of 5 passed
```

Replay a capture through the same recognizer, which is how every marker in it is tested:

```
$ cargo xtask board-console --replay target/boot-check-riscv64.log --until selftest
```

## BUGS

- Closed 2026-09-14: "Nothing proves an architecture ran the *right* five checks."
  `boot_ladder::SELF_TEST_CHECKS` is the set, one list for every architecture and the recognizer;
  the kernel names a listed check that never ran and `board_console` fails a verdict whose total is
  not the list's length. What remains is that changing the set is one edit, which review judges.
- Closed 2026-09-14: "A self-test can hang the boot." `timer` and `scheduler` read through
  `self_test::Counter`, which reports a counter that read one value a million times running, and
  `timer` spins rather than sleeping. What remains is a check whose *operation* never returns, which
  needs a watchdog `kernel/src/self_test.rs` does not own and says so.
- The prompt rung is the shell's banner, not the `$ `. `crates/boot_ladder`'s own BUGS has the
  reason. So `script/boot-check` proves `swish` started and printed, not that a prompt was offered
  or that anything could be typed at it. `script/swish-check` makes that stronger claim by typing.
- The injected leg is not in CI. `--inject` rebuilds three kernels for one boolean, and what it
  proves is a property of the gate rather than of the change under test. Run it by hand when the
  self-test or the recognizer changes. This is rung four of AGENTS.md's ladder and it is said out
  loud rather than implied.
- The riscv64 and `x86_64` arms narrate their own bring-up and the description then repeats some
  of it. Noise rather than a defect, and trimming it is not free: those arms' lines are what a
  `test` or `bench` boot has instead, since the description is compiled out of both. It is milestone
  409 (`design/roadmap/0409-one-machine-description-not-two.md`).
- **The wordings are provisional** (milestone 268 (every architecture boots the same way)). They are
  contracts, so they are an architect's under AGENTS.md's *move fast on what can be undone* tenet; a
  lane ships one and says so rather than waiting.

See `design/roadmap/0268-the-boot-ladder.md`, `notes/board-console.md`, and `notes/visionfive2.md`.
