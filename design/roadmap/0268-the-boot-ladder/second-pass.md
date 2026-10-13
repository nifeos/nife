# The second pass, 2026-09-14: the top rung's entry point, and two BUGS closed

An appendix of [milestone 268 (every architecture boots the same way)](../0268-the-boot-ladder.md).
The main page keeps what changed and which BUGS closed, and is complete without this file. This one
keeps the transcripts that proved it.

Built in one lane with [milestone 182 (x86_64's interactive
boot)](../0182-x86-64-interactive-boot.md), whose block carries the entry point in full.

## `x86_64` no longer halts

The default boot ends by handing the machine to the progenitor, loaded from the archive and
measured, through the same function riscv64's default boot enters
(`kernel::user::riscv_shell_boot`). It reaches ring 3, builds the system, and stays running. What it
cannot do is offer a prompt, because the console server and the input driver it builds trap on first
use: a ring-3 process cannot reach port I/O (§121 (port I/O as a capability)). The boot says so
rather than going quiet:

```
nife: handing the system to the userspace progenitor.
  uart irq: line 4 (machine description)
  ...
nife x86_64: the progenitor is running at ring 3; 2 of the processes it built stopped on purpose.
  no prompt  : the console server cannot reach COM1 from ring 3 (§121); how a shell gets a console here is §149, not yet decided.
```

The body it shares with riscv64 changed (every grant names its slot), and `script/swish-check`
still reaches and drives the prompt on aarch64 and riscv64 after it.

"Nothing halts" now holds on all three architectures. The rung that is still missing on `x86_64`
is the prompt, and it waits on one ruling.

Three gates keyed on the old halt line (`uefi-boot`'s serial and screen markers,
`script/netboot-rehearsal`) and now wait for `boot_ladder::SELF_TEST`, the last line every boot
prints whatever comes next. `uefi-boot` also requires the hand-over, and passed under OVMF.

## "The self-test can hang the boot": closed

`timer` waited in `wait_for_interrupt` until the counter passed its window, so a stopped counter, or
a machine whose interrupts never arrived, parked the boot forever; `scheduler`'s deadline was
unreachable on a stopped counter. Both now read through `self_test::Counter`, which reports a
counter that read one value a million times running (every counter this kernel reads changes at
least every 250 ns), and `timer` spins rather than sleeping.

Proved with the injector, which now also stops the checks' copy of the counter. Before, on
`x86_64`:

```
  self-test       : frames     ok      one frame out and back at 0x993000; ...
boot-check (x86_64): went quiet after machine described (5014 bytes in 30.8s)
boot-check (x86_64): FAILED. A kernel built with `--features self_test_injection` must print a RED verdict and this run did not ...
```

After, on all three:

```
boot-check (aarch64): nife self-test: 3 of 5 passed, 2 FAILED: exceptions timer
boot-check (riscv64): nife self-test: 3 of 5 passed, 2 FAILED: exceptions timer
boot-check (x86_64): nife self-test: 3 of 5 passed, 2 FAILED: exceptions timer
boot-check: every architecture reached the self-test verdict and it was RED, as the injection asked
```

with `timer FAILED  the counter stopped at 0x268: 1000000 reads without a change`, each verdict in
under a second. A plain `boot-check` stays green on all three. Because the injector keeps stopping
the counter, every `--inject` run is now also a gate for this bound.

## "Nothing proves an architecture ran the right five checks": closed

Before, measured by cutting `scheduler` from `x86_64` and setting the count to four, the edit a
lane makes to keep `self_test`'s `debug_assert` quiet:

```
boot-check (x86_64): nife self-test: 4 of 4 passed
boot-check: every architecture reached the self-test verdict and it was green
```

`boot_ladder::SELF_TEST_CHECKS` is now the set, one list for every architecture and the recognizer.
The kernel counts against it and names a listed check that never ran, and `board_console` fails a
verdict whose total is not the list's length. After, the same cut:

```
  self-test       : scheduler  FAILED  did not run on this architecture
boot-check (x86_64): nife self-test: 4 of 5 passed, 1 FAILED: scheduler
boot-check (x86_64): FAILED. ...
```

The injected verdict reads `3 of 5 passed, 2 FAILED: exceptions timer` rather than the `4 of 5,
1 FAILED: exceptions` in [the first pass](first-pass.md#the-proof-condition-answered), because the
injector injects two faults now; that transcript is kept as the record of the first pass.
