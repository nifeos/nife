# The four findings and the name, in full

An appendix of [milestone 268 (every architecture boots the same way)](../0268-the-boot-ladder.md).
The main page keeps each finding and the naming verdict in a sentence or two and is complete without
this file. This one keeps the measurements behind them as they stood on 2026-09-09.

## The four findings this is built on, each measured

1. `machine.rs` exists on x86_64 only. aarch64 and riscv64 discover the same facts from the
device tree and print them scattered inline through `main.rs`. So this is not moving one
architecture's output to the others. The other two have no machine module at all.

2. aarch64 has no `exceptions::self_test`. riscv64 and x86_64 both have one, called from
`main.rs:210` and `main.rs:686`, right after `arch::init()`: it fires a breakpoint, proves it was
caught and stepped over, and returns the count. `notes/riscv-port.md` calls it "a boot self-test".
There is a second, unnamed one at `main.rs:873`, a dynamic kernel mapping check that maps a fresh
frame at an unused high-half address and reads it back. The concept and the name are already in
this tree, on two architectures, uncollected.

3. `Stage::Tour` can never fire on two of three architectures. `board_console` matches the single
string `"nife: the capability core runs on "`, printed at `kernel/src/main.rs:1401`, inside the
RISC-V arm. The aarch64 tour prints no marker at all. So `--until tour` is unreachable on aarch64
and x86_64, and a missing marker looks exactly like a slow board. That is the checkpoint the bench
tooling relies on to mean "paging, traps, the timer, the frame allocator, SMP and the scheduler all
came up".

4. The tour's checks report failure by printing a word and continuing. It spawns two threads that
   never yield and counts preemptions; it reads a filesystem through the real driver and compares
   bytes. Both are real. Neither asserts: on failure they print `FAILED: a spinner did not run, or
   nothing was preempted.` and `the pcie driver reported the wrong bytes`, and the boot reaches the
   same end state either way. That is rung four of AGENTS.md's ladder, and it is why finding 3 hid
   for months: nothing was gating on the rung everyone believed in.

## Why `self_test`, and the prior art is ours

The candidates were `self_test`, `power_on_self_test`, `checkup` and `diagnostic`. The tree decided
it before the question was asked: `arch::exceptions::self_test()` already exists on two
architectures, already runs at boot, already means "the kernel proving something about itself", and
`notes/riscv-port.md` already uses the words "boot self-test" in prose.

The collision the maintainer worried about (against `kernel/src/testing.rs`, `script/test`, and the
Kani harnesses) is not real: the tree already draws the line consistently, with `self_test`
meaning the kernel checking itself at boot and `test` meaning the suite.

`POST` is taken, and not by us. `notes/xenon-firmware.md` quotes Dell's own "Power On Self-Test
(POST)" from xenon's firmware, which an operator meets on the screen before ours runs.

External prior art was recalled and not read, and is recorded here as a claim to check rather than
as evidence. Linux's crypto subsystem runs boot-time self-tests with per-algorithm pass lines. KUnit
runs in-kernel tests at boot behind a config. FIPS 140 mandates power-up self-tests that do gate,
and hardware calls the concept BIST. The decision did not turn on any of them.
