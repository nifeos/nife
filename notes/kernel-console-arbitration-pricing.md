# Pricing milestone 342

calef approved a pricing lane on milestone 342 (the kernel and the `console` server drive one UART
from two address spaces) on 2026-09-27, rather than a build. This is a design fork, decided at §175
(where the kernel's own output goes once userspace owns the console), status PROPOSED, raised
2026-09-19. §175 already measured the fault-report cost and laid out four options (A second port, B
a drained buffer, C a claim with a panic exception, D leave it).

This note answers the two things §175 said were still open: the hardware fact per board, and the
panic cost under B and C. It also prices a fifth option calef asked for (release builds go quiet,
seL4's shape), and corrects one thing §175 undercounted: fault reports are not the only thing the
kernel writes after handoff.

Milestone 342's own roadmap block was stale: it said "nothing in `design/decisions/` answers it" and
carried `decision_dependencies: unwritten`. §175 has existed since 2026-09-19. That block is
corrected in this lane's commit.

## 1. What the kernel writes after handoff (more than §175 counted)

§175's `KERNEL_FAULT_TOKENS` measurement (six tokens, three lines, ~150 bytes, once per faulting
user thread) is real and correctly sourced from `swish_check::KERNEL_FAULT_TOKENS`, and it is
identical on all three architectures: `kernel/src/arch/{aarch64,riscv64,x86_64}/exceptions.rs` each
print the same shape (`user thread N killed: ...`, a register line, `the kernel is fine.`).

But two more kernel `println!` sites fire in ordinary, non-fault operation, and they are the ones a
swish-check run actually hits:

- **`kernel/src/cap.rs:394-401`, `announce_peak`**: the kernel prints `capability slots: N of M at
  peak` every time the process's capability-table high-water mark reaches a new peak and holds for
  a few scheduler passes. Its own doc comment calls this out directly, naming milestone 230
  (`script/shell-check` is red on `main`): *"That is the same two-writers-one-stream confusion
  milestone 230 spent a whole lane on."* This is gated off under `cfg(test)`/`system_tests`. It is
  on under the interactive boot swish-check uses.
- `kernel/src/progenitor_stack.rs:166-177`, `announce`: prints `progenitor stack: N of M bytes at
  peak, K spare` on the same new-peak-and-hold pattern. It triggers from the idle loop on aarch64
  and riscv64, on every reschedule with nothing to run. On x86_64 it triggers from `on_yield`,
  throttled to one look per 256 yields (`kernel/src/progenitor_stack.rs:150-160`), specifically
  because doing it every yield "would print more lines on a multi-core machine, where a kernel line
  and a userspace one can shuffle byte by byte". `on_yield` is gone since milestone 505 (an x86_64
  input driver that never lets the core idle). x86_64 reports from the idle loop too.

A captured aarch64 swish-check transcript (CI run 36329333745, 2026-09-27, the aarch64 leg) has six
of these lines in a 111-line, 17.6-second transcript. Several land right after a `$ ` prompt and
before the next command's echo, e.g. `$   progenitor stack: 18648 of 49152 bytes at peak, 30504
spare` immediately preceding `echo hello world | wc`. That is not a fault; it is routine gauge
output, once per new stack or capability-table peak. On a shell session that spawns a program per
command, which most of swish-check's 111 lines do, new peaks are common, not rare.

The tree already found the progenitor-stack gauge doing exactly this, and patched around it without
fixing the write: `xtask/src/swish_check.rs`'s `GaugeFilter` strips `"  progenitor stack:"` lines
out of the transcript before any check reads it, because *"the first version with the gauge in it
waited thirty seconds for a prompt that had already been printed, then read `echo hello world | wc`
as having answered nothing."* That is this milestone's failure mode, already reproduced and already
fixed once, in the harness rather than at the source. `capability slots:` has **no equivalent
filter**: `swish_check::swish_check_boot`'s slot-gauge check finds it with a plain `rfind` over
whole lines, and the general command-echo wait (`swish_check_boot`'s `wait_after` closure) does an
exact substring match with no interleaving tolerance. If a capability-slot peak announcement lands
between a prompt and the next command's echo, the wait fails exactly the way the progenitor-stack
gauge already failed once: `"the prompt never echoed \`{line}\`"`.

## 2. Confirming the flake is this interleaving

Two independent, already-in-tree, real captures, not reasoning from a plausible story:

- `xtask/src/swish_check.rs`'s own `SHREDDED_CI_TRANSCRIPT` fixture, captured verbatim from real CI
  run 33707574930 (milestone 230): the kernel's `the kernel is fine.` fault line and the
  progenitor's `...struction budget dropped...` line are shredded into each other character by
  character, into `the kernel iis fnit: constiner.` and `uction budget dropped`. Both strings are
  destroyed; neither is invented (`find_marker`'s guiding principle, `xtask/src/swish_check.rs`
  passim). This is the two-writers-one-stream defect, captured, not inferred.
- The `GaugeFilter` doc comment, above: a real, dated, already-fixed prior occurrence of this exact
  failure message shape, caused by a gauge rather than a fault, landing between a prompt and a
  command's echo.

The three 2026-09-27 instances named in this lane's brief are #1412, #1404, and #1402's riscv64 leg
("the prompt never echoed `packages/uptime/0.1.0/uptime`", run 36329333745 attempt 1). I could not
recover a raw transcript for that specific riscv64 leg. That job's aarch64 leg failed first, on an
unrelated `std-grep` out-of-memory defect, and `bash -e` stopped the step before riscv64's plain
swish-check ran in that run. I did not find matching failures in #1404's or #1412's retained CI logs
within this lane's budget either; their 2026-09-27 failures were a build-script panic and an
unrelated riscv64-model infra error respectively.

I am therefore not asserting those three are proven instances of this defect. I am asserting that
the defect is real and already captured once, verbatim (above). It is also mechanically primed to
recur in exactly this shape: `package install downloads/uptime.nifepkg` is exactly the kind of
command that grows the capability table, and the very next line in the script is
`packages/uptime/0.1.0/uptime`. A new capability-table peak announced between those two lines would
produce precisely the quoted failure, by the same mechanism already caught once for the other gauge.

## 3. Hardware: does each board have a second usable UART?

Read from datasheets, device trees and vendor manuals, no board touched, per this lane's limit.

| board | SoC UART count | second UART usable today, without new wiring |
|---|---|---|
| **argon** (Jetson TX1, Tegra X1) | 4 defined in `tegra210.dtsi` (UARTA-D) | **No, unconfirmed at best.** UARTA is the current debug port; UARTD is enabled but wired internally to the on-module BT/Wi-Fi chip, not any external header. A second external UART (`J17`, `/dev/ttyTHS2`, shared with the camera connector) is claimed by one NVIDIA-forum post, not by NVIDIA's own carrier-board spec: unconfirmed provenance. |
| **radon** (VisionFive 2, JH7110) | 6 defined in `jh7110.dtsi` | **No, not without rewiring.** StarFive's own Quick Start Guide and 40-Pin Header user guide state plainly that UART is only exposed on the 40-pin GPIO header. A second UART instance is reached by re-muxing other pins of that same header, not a separate connector, which repurposes GPIO pins this project's bench rig may already use for something else. Re-pinning the header is bench-hardware work, out of this lane's scope. |
| **xenon** (Dell OptiPlex 7050 Micro) | Super I/O chip class (generic NCT6776-class) supports two; Dell's own Owner's Manual shows the board does not wire a second one out | **No, confirmed no.** The system-board layout and the ports/connectors spec table both show exactly one serial connector, marked optional. The BIOS's COM1-COM4 menu selects which legacy address the one port answers at; it is not four simultaneous ports. |
| QEMU **virt** (aarch64) | (a QEMU machine, no SoC) | **Yes, free.** `hw/arm/virt.c` already defines a second PL011 (`VIRT_UART1` at `0x09040000`); it appears the moment a second `-serial` is passed. |
| QEMU **virt** (riscv64) | (a QEMU machine, no SoC) | **Yes, free.** `hw/riscv/virt.c` defines `VIRT_UART1` at `0x1000a000`, gated on a second `-serial`. |
| QEMU **q35** (x86_64) | (a QEMU machine, no SoC) | **Yes, free.** `pc_superio_init` wires up to `MAX_ISA_SERIAL_PORTS` (4) unconditionally; COM2 at `0x2f8` needs only a second `-serial`. |

This is the finding that should be read first. Option A is free in CI, under QEMU, and confirmed
absent or unconfirmed on real hardware on all three boards, worst on xenon where it is flatly
impossible without adding a card. A CI gate that passes on a second-port kernel while every real
board still corrupts its bench log would be a worse failure than today's, because it would look
fixed. Architectural parity is a gate (AGENTS.md rule 5), and this decision "binds every
architecture." A second port cannot be the whole answer; at most it is a QEMU-only mitigation with
no story for xenon at all.

## 4. What each option costs in a panic

The kernel's own policy for its internal re-entrant case is already settled, and instructive:
`console::force_unlock` (`kernel/src/console.rs:677-690`) breaks its own lock on a fault, so the
panic message is never lost behind a held lock, and says so: *"Output may be spliced. That is a fine
price for getting the message out at all."* The cross-process case (kernel vs. the userspace
`console` server) is the same trade, one level up, and the options differ in whether they can even
try.

- **A (second port).** Immune by construction where it exists: the server never touches the
  kernel's port, so a panic can never be spliced with server output. Priced above as unavailable on
  two of three boards.
- **B (buffer the server drains).** This is the worst option in a panic, and §175 already named why:
  *"a panic is exactly when the draining server may be the thing that died, so a buffered panic is a
  panic nobody reads."* A pure B with no escape hatch means a wedged or dead server is a kernel that
  goes completely silent at the one moment silence is worst. Any B that adds a direct-write escape
  for panics has stopped being B; it is C, with a buffer bolted on for the case that does not need
  one.
- **C (a claim, respected except in a panic).** Matches the kernel's own existing philosophy.
  Outside a panic, the kernel already writes almost nothing: part 1's fault reports, plus the two
  gauges above. So "wait for the claim" costs nothing routine. In a panic it overrides, same as
  `force_unlock` today, and output may splice with whatever the server was mid-writing. That is an
  accepted cost already, never worse than what happens now.
- **D (leave it).** Identical panic behavior to C: both write directly, unclaimed, today. It has
  none of C's routine-case improvement, and is priced only as the baseline.
- **R (kernel goes quiet post-handoff in release builds, seL4's shape).** Read faithfully (part 6),
  seL4's own answer to "what does a panic cost in release" is not splicing, it is **nothing**.
  `include/assert.h` compiles `fail(s)` straight to `halt()` with no `printf` call at all when
  `CONFIG_PRINTING` is off, because the `printf` bodies are removed from the verified binary
  entirely, not merely suppressed at runtime. Taken as far as its own prior art goes, R would mean a
  release nife kernel's panic prints nothing either. That **reverses** what `console::force_unlock`'s
  own comment already decided for this kernel: getting the message out, spliced or not, is worth the
  price. A faithful R undoes that choice for exactly the builds where a panic is most likely to
  matter to somebody. A looser R, quiet gauges but still-splicing panics, is not seL4's shape; it is
  C with a release/debug flag bolted on. See part 5 for why CI would not notice either version.

## 5. What each option does to the flake specifically, R priced separately

In `swish_check::swish_check`, swish-check's `--release` flag exists and is explicitly "Not in
CI." CI's `build + test` job builds and boots the debug/dev kernel on every leg, confirmed directly
from the run 36329333745 log: `Finished \`dev\` profile [unoptimized + debuginfo]`. Option R gates
kernel chatter on the release/debug distinction, so since CI never builds release, **R changes
nothing about the CI flake this lane exists to price**. It might reduce chatter on a shipped,
release-mode board deployment. But the bench sessions on argon/radon/xenon that motivate this
milestone (`notes/bench-runbook.md`, `xtask/src/bench.rs`) are not confirmed to run release either.
The progenitor-stack gauge's own comment says the debug build is the deeper, more informative one,
which is why swish-check measures debug by default. R also has no story for a release-build panic;
it still needs an answer from A, B, C or D, so it would sit beside one of those rather than replace
it. And it is new machinery: every kernel print site would need a release/debug distinction that
does not exist in the tree today (`kernel/src/console.rs`, `kernel/Cargo.toml` have no such gate
now).

Options A, where hardware allows, plus B-with-escape and C, all remove the two ordinary-operation
gauge prints from the collision surface: moving them off the shared wire, buffering them, or making
them wait for the claim. D leaves the flake exactly as observed today.

## 6. Prior art, read

- Fuchsia's `debuglog` (`zircon/kernel/lib/debuglog/debuglog.cc`, fetched from
  fuchsia.googlesource.com) is option B done properly: a bounded kernel ring buffer (`DLog::Write`),
  drained by userspace over `zx_debuglog_read()`. Its panic path is C wearing B's clothes:
  `dlog_panic_start()` sets a flag that makes every further `DLog::Write()` return `ZX_ERR_BAD_STATE`
  immediately, so the buffer stops accepting input. `dlog_bluescreen_init()` then prints straight to
  the serial console through `dlog_serial_write()`, bypassing the queue and drainer thread entirely.
  The boot option `kernel.bypass-debuglog` forces all kernel output through that direct path, buffer
  disabled, for exactly the case where the buffer's own drainer cannot be trusted.
- Linux's `printk` ring buffer (`kernel/printk/printk_ringbuffer.h`, `kernel/printk/printk.c`,
  fetched from torvalds/linux) is drained by attached consoles and by userspace (`/dev/kmsg` via
  `devkmsg_read`) from the same structure, concurrently, using the ring buffer's own lock-free
  synchronization. On panic (`kernel/panic.c`), `console_flush_on_panic()` explicitly ignores the
  console lock rather than trying to take it. Its own comment says semaphores are not NMI-safe, and
  that a trylock would be pointless because a contended lock must be ignored either way, and it
  force-flushes every registered console.
- Both systems land on the same shape this lane is pricing as B/C: normal traffic through a buffer a
  userspace reader drains, and an explicit flag that stops trusting the buffer the moment a panic
  starts. A direct write bypasses everything else once that flag is set.
- seL4 (`config.cmake`, `include/machine/io.h`, `include/assert.h`, `src/assert.c`, all fetched from
  github.com/seL4/seL4) is option R's actual prior art, and it is more radical than the roadmap
  block's one-line gloss suggests. `CONFIG_PRINTING` is a compile-time gate: when it is off (forced
  off under `CONFIG_VERIFICATION_BUILD`), `printf` macros to `((void)(0))` and the bodies of
  `_fail`/`_assert_fail` are not compiled in at all, not silenced, absent. `fail(s)` then expands
  directly to `halt()`. A verified seL4 kernel that hits an internal invariant violation prints
  **nothing**, and halts. That is a stronger and different claim than "quiet except panics": seL4's
  own answer to "what does a panic cost" is silence, not a spliced message. Debug builds keep
  `CONFIG_PRINTING` on and print normally, the same shape as everything else this tree does today.

## 7. AGENTS.md's seven questions

1. What else was considered, and why did each lose? R, priced here, loses because CI never builds
   release, so it does not touch this milestone's motivating flake. A pure B with no panic escape
   loses to §175's own argument: a dead drainer means total silence in a panic.
2. What does this tree already do in the analogous case? The kernel's own internal `force_unlock`
   (part 4): splice the output rather than lose it, on panic, already. C and D inherit this; A
   sidesteps it; B without an escape breaks it.
3. Prior art outside the tree, read not recalled? Part 6: Fuchsia's debuglog and Linux's printk ring
   buffer both converge on C wearing a buffer, B in the tree's ordinary case, C's override on panic.
4. Is the premise true? Checked, not assumed. That fault reports are the whole collision surface is
   false: part 1 found two more live writers. That a second port is a clean answer is false on two
   of three boards, confirmed by datasheet, not assumed. That this is really the interleaving
   described, and not a coincidence, is confirmed by a captured, in-tree, byte-exact fixture (part
   2), not inferred.
5. What does each option cost, measured? Part 1 sizes the collision: two gauges, new-peak-triggered
   and common on a per-command-spawn session, plus one ~150-byte fault report per faulting thread.
   Part 3 prices A per board. Part 4 prices each option's panic behavior against the kernel's own
   existing splice-on-panic precedent.
6. How reversible is it, and who has already acted on it? Fully reversible today: nobody has built
   against a UART-arbitration contract yet, because there is not one. It becomes expensive the
   moment a second board is bought expecting a free UART (A), or a program is written assuming
   ordering guarantees a claim would provide (C). That is why it is calef's call now rather than
   later.
7. Would we still choose this if both cost the same? Not answerable here. That is exactly the
   irreversible-fork case AGENTS.md says to give options on rather than recommend. It is what §175
   already declined to do for the same reason: A and C cost genuinely different things, a hardware
   fact per board versus a flag and an exception. The one measurement that would make them cost the
   same, rewiring a board for a second UART, is bench work this lane was told not to do.

The effort sentence: every option here is more work than D. None of A, B, C or R was chosen, or
would be rejected, for being less work than another. The actual costs priced above (a hardware fact
that is mostly "no," a panic-silence failure mode, a new release/debug axis that does not touch CI)
are what decide between them, not effort.

## What is blocked until this is answered

Milestone 342, and every bench session on argon, radon and xenon whose log a human or
`serial_console` reads. `design/roadmap/0342-kernel-console-arbitration.md`'s `decision_dependencies`
is corrected in this lane's commit to `175`, from `unwritten`.
