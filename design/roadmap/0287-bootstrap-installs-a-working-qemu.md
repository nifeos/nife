---
status: BUILT
raised: 2026-09-13
built: 2026-09-13
---
# 287. `script/bootstrap` installs a working QEMU on Linux, instead of printing how to

This is AGENTS.md's third principle failing, and the tell is that the fix for it had already been
written once. *A newcomer must be able to succeed without asking anyone.* Milestone 117's second
stranger run found that `script/setup` could not complete on a stock Linux box; the remedy applied
was a better error message; the message never printed, would have been the ladder's floor if it had,
and named two commands that could not work. `notes/stranger-test.md` recorded it as fixed and stayed
wrong for twenty-eight days, which that note now says out loud.

## What was actually broken, measured rather than recalled

On a stock Ubuntu box, before this milestone:

```
$ qemu-system-aarch64 --version
QEMU emulator version 8.2.2 (Debian 1:8.2.2+ds-0ubuntu1.18)
$ qemu-system-riscv64 -device help 2>&1 | grep -c riscv-iommu
0
$ script/qemu-check; echo $?
qemu-check: QEMU 8.2.2 is missing: riscv-iommu-pci(riscv64)
1
```

**That part is not the bug.** `.qemu-version` pins 11.0.2, no Ubuntu release ships a QEMU with
`riscv-iommu-pci`, and `apt-get install` already fetches the newest package for the release, so there
is nothing better for apt to get. `script/ci-qemu`'s header has had this right all along, and the
tempting workaround (drop the IOMMU device when QEMU lacks it) stays rejected: it would let the
DMA-confinement tests pass while testing nothing.

Three defects sat on top of it, and each one hid the next, which is why a milestone was spent
here rather than a patch.

One: the Linux branch was dead. `script/bootstrap` guarded its advice with `[ "$os" = linux ]`
while `os="$(uname -s)"` produces `Linux`. Milestone 117's message had never once reached a reader;
bootstrap simply exited 1 with no Linux advice at all. Nothing caught it because nothing on Linux ran
bootstrap from cold again.

Two: printing is rung four. Even working, a script that knows the next two commands and asks a
human to type them is describing a mechanism instead of being one.

Three, and this is the load-bearing one: the printed remedy looped. `script/ci-qemu` installs
into `${QEMU_PREFIX:-$HOME/.cache/nife-qemu}`. The **only** thing in the entire tree that ever put
that prefix on PATH was `.github/workflows/ci.yml`, at three separate `$GITHUB_PATH` lines. Nothing
in `script/`, nothing in `helpers/`, nothing in `xtask`. So a Linux developer following the
instructions verbatim spent twelve minutes building the right QEMU, re-ran `script/setup` as told,
and `qemu-check`'s `command -v qemu-system-aarch64` found `/usr/bin`'s 8.2.2 again. Same failure,
same message, same remedy, forever.

Why nobody met it. Every contributor's machine was warm, and CI sets the PATH itself, so the only
configuration that exercises this path is a cold Linux clone: exactly what the stranger test exists
to be, and exactly what the fix it prompted was never run against.

## Who owns the PATH, which was the real question

The emulator is invoked by bare name from 38 sites across 16 files, in three languages:
`exec qemu-system-aarch64` at the bottom of `helpers/qemu-runner-aarch64.sh`, the riscv64 and x86_64
runners beside it, `helpers/qemu-uefi-x86_64.sh`, a `subprocess` argument list in
`script/netboot-rehearsal`, probe lines in `script/gates` and `script/cpu-matrix`, and
`script/qemu-check` itself. **So the mechanism is PATH inherited by children, not one spawn site to
patch.** Four options were priced.

Refused: `export PATH` from bootstrap and setup for their own children. It is the cheapest thing
that makes `script/setup` finish, and it is disqualified on behavior rather than on effort: a
process cannot edit its parent's environment, so the developer's *next* `script/test` in a fresh
terminal gets 8.2.2 again. That is the same loop one turn later, and `script/test` does not call
bootstrap (by design; notes/scripts.md says why).

Refused: resolve the emulator at each call site. Thirty-eight copies of one fact, in shell and
Python, and every future site a chance to forget. `script/ci-qemu`'s own header already argues
against exactly this shape for exactly this reason, about the device list: *"Two copies of 'what QEMU
must provide' would drift, and the copy in CI is the one nobody reads."*

Refused: put the resolution in `xtask`, on the grounds that xtask is where QEMU is launched. This
was the first recommendation and the premise is false. `cargo xtask` spawns the runner shells, so it
would cover `script/test` and its siblings, but it reaches neither `script/qemu-check` nor
`script/cpu-matrix`'s direct probe nor `script/netboot-rehearsal`. Covering those as well would mean
two mechanisms, and the Rust one would need its own copy of "where is the prefix and is it the pinned
version", which is the drift above.

Taken: one sourced fragment, `helpers/qemu-path.sh`, read by every entry point that can reach an
emulator. It has no shebang because it is sourced rather than executed: it exists to edit the
caller's PATH, which an executed script cannot do. One copy of the logic, one line per entry point,
and every one of the 38 bare-name invocations gets the right emulator without being touched.

Would we still choose it if the options cost the same? Yes, and that is the honest answer rather
than a convenient one. Against the call-site option it has fewer moving parts and fewer places to be
wrong, which is what this project means by elegance. Against the xtask option it is one mechanism
instead of two with a duplicated fact between them. The `export PATH` option is cheaper than all of
them and loses on a measurement, not on taste.

## What was built

- `helpers/qemu-path.sh` (**name provisional**): prepends
  `${QEMU_PREFIX:-$HOME/.cache/nife-qemu}/bin` to PATH when that prefix holds the pinned version.
  Idempotent, so an entry point calling another cannot stack the prefix up.
- `script/bootstrap`: the dead `linux` comparison is `Linux`; on a Linux `qemu-check` failure it
  now says what the next twelve minutes are for and then runs `script/ci-qemu`, sources the
  fragment, and re-checks. It also sources the fragment before its `command -v` probes, so a second
  bootstrap does not re-run the whole apt branch against a prefix that already holds what is wanted.
- Twenty-five other `script/` entry points: one `. helpers/qemu-path.sh` each, immediately after
  the `cd` to the repository root.
- `script/lint`, *the project's QEMU is on PATH*: a file under `script/` that runs `cargo xtask`,
  names a `qemu-system-*` binary, or calls a `helpers/qemu-*` helper must source the fragment.
- `notes/scripts.md` and `notes/stranger-test.md` (the correction above).

## The version gate, and the demotion it prevents

Prepending unconditionally would let a prefix left over from an older pin beat a system QEMU that is
perfectly good, which would have been a regression this change introduced on any distribution that
ships a new enough emulator. The fragment reads `.qemu-version` and prepends only on an exact match.
**That is a third reader of the pin, not a fourth copy of the number**: the file stays the single
source of truth, which is the distinction `script/qemu-check`'s header draws.

It finds the pin by walking up from `$PWD` rather than reading it from the current directory.
Every `script/` entry point cds to the root as its first act except `script/crate-probes`, which
keeps a `$ROOT` and never cds, so a contract spelled "be at the root" would have had exactly one
exception on the day it was written. Four lines removed the exception instead of documenting it.

## The gate is mechanical, and it over-approximates on purpose

`script/apropos` and `script/crate-probes` shell into `cargo xtask` and boot nothing today. They take
the line anyway. The alternative is a judgment call per file plus an allow-list, which is the shape
`script/lint` check 5 and milestone 283 both refuse to grow, and the line costs them nothing.

**A backticked mention is prose, not a call**, which is milestone 283's rule reused rather than
reinvented: these headers discuss `cargo xtask` constantly. Stripping backticked spans is what keeps
`script/lint` (whose only raw hit is a sentence inside a Python heredoc) and `script/board-netboot`
out without either needing an exemption. `script/lint` still trips the rule through the regex in the
check's own source, because a checker necessarily carries every pattern it looks for; it takes the
line rather than an exemption.

Verified in both directions. Four faults were injected one at a time and the gate watched to fire
on each: the source line dropped from `script/test`, the fragment deleted, a new entry point that
runs `cargo xtask` and forgets, and a backticked mention that must stay quiet (it did).

## Proof, because the point of this milestone is that reasoning was not enough last time

Reproduced first, on this Ubuntu box with apt's 8.2.2 on `/usr/bin`: `script/qemu-check` exit 1,
`riscv-iommu-pci` absent. Then, after the change:

- A completely clean environment (`env -i`, PATH holding only the system directories, so 8.2.2
  is the only emulator reachable by the old rules) runs `script/qemu-check` and gets
  `QEMU 11.0.2 matches the pin, with every required device present`.
- A genuinely cold prefix. `QEMU_PREFIX` pointed at an empty directory and `script/bootstrap` run
  against it: the Linux branch fires, the twelve-minute warning prints, `script/ci-qemu` builds
  11.0.2 into that prefix, the fragment picks it up, and the re-check passes, all in one command with
  nothing typed in between.
- A fresh shell afterwards finds `/root/.cache/nife-qemu/bin/qemu-system-aarch64`, which is the
  criterion that failed before: `script/setup` from cold completes (`==> QEMU present: QEMU emulator
  version 11.0.2`, with the apt branch skipped entirely), and a later `script/test` in a new terminal
  resolves the pinned emulator rather than apt's.
- The riscv64 kernel boots, which is the sharpest form of the proof available: `script/test
  --arch riscv64` from a shell whose PATH held only the system directories brought up OpenSBI and
  printed `nife on RISC-V (rv64, S-mode, Sv39)`. QEMU 8.2.2 cannot do that at all, because
  `-device riscv-iommu-pci` is not a device it has.

And a control run, because two numbers are worth more than one. This box is a headless sandbox,
so five post-run checks fail on both architectures: three `scanout` (no monitor for a screendump) and
`inbound`/`multicast` (host networking). Those are **not** this change: the same
`script/test --arch aarch64` with `QEMU_PREFIX` pointed at an empty directory, so the fragment
no-ops and 8.2.2 is used, fails with exactly the same five. The guest-side assertions pass in every
run (*"the compositor test passed"*, *"the display test passed"*); what cannot be observed is
host-side. See notes/load-sensitive-assertions.md.

**What is deliberately not claimed: `script/test` does not exit 0 on this box, and it is not this
change.** `crates/elf` fails twenty of twenty-five host tests here, the host pass runs before the
kernel legs, so the suite stops there. It reproduces on `main` and this milestone's diff contains no
`.rs` and no `.toml`. The kernel legs above were reached with `--test`, which skips the host pass by
design. The diagnosis is in the follow-on, and it is the honest shape of this result: the emulator
half is fixed and proven, and a different wall is standing behind it.

## BUGS

- **macOS is untested, and this lane could not test it.** The argument that it is unaffected is
  stronger than usual and is still an argument. On a `qemu-check` failure the old code evaluated
  `if [ "$os" = linux ]`, which was false on *every* machine, and then `exit 1`; the new code
  evaluates `if [ "$os" != Linux ]` and then `exit 1`. On macOS those are the same two statements, so
  that path is bit-for-bit what it was. The only other macOS-visible change is the added
  `. helpers/qemu-path.sh` lines, which no-op unless `$HOME/.cache/nife-qemu/bin/qemu-system-aarch64`
  exists, and `script/ci-qemu` (the only thing that creates it) refuses to run off Linux. Nobody
  has run any of that on a Mac, which is the honest end of the sentence: no gate in this repository
  runs on macOS, so the architect's own machine is the first one that will.
- The gate cannot check that the source line is in the right place. It checks presence. A line
  placed before the `cd`, or after the work it is supposed to affect, passes. Ordering inside a shell
  script is not something a grep can hold.
- The gate cannot see `helpers/`, `xtask`, or a Makefile. Its scope is `script/*`, on the
  reasoning that everything else is spawned by something in there and inherits. If a future cargo
  `runner` or CI step invokes an emulator without passing through a `script/` entry point, nothing
  fires.
- A prefix built from a different `.qemu-version` is silently ignored rather than reported. The
  fragment no-ops, `qemu-check` then warns about whatever system QEMU it finds, and nothing says "you
  have a stale build at `$HOME/.cache/nife-qemu`". Re-running `script/ci-qemu` fixes it; nothing
  tells you to.
- Sourced from outside the repository, the fragment is a no-op, because the upward walk for
  `.qemu-version` reaches `/` and stops. Only `script/crate-probes` can be in that position, and it
  boots no emulator.
- Bootstrap still installs apt's emulator packages on a cold Linux box and then shadows them.
  See the follow-on below; removing them was not proven safe here.

## Follow-on

- **Milestone 396.** Stop installing the QEMU packages apt cannot make useful. On a cold Linux box
  `script/bootstrap` installs `qemu-system-arm qemu-system-misc qemu-system-x86 ipxe-qemu ovmf` and
  then builds a QEMU that shadows the three emulator packages entirely. Measured on the built
  prefix, QEMU's own `make install` ships `efi-virtio.rom`, six `pxe-*.rom` option ROMs and every
  `edk2-*.fd` firmware into `$prefix/share/qemu` (71 files), which is also where
  `helpers/qemu-uefi-x86_64.sh` already looks first, so the two firmware packages look redundant too.
  That is where the evidence stops, and proving it needs the x86_64 UEFI gate and the netboot
  rehearsal run green with those packages absent, which is more than this lane could show. Nothing
  was removed. 396 names the four runs that would close it and prices the prize honestly: a few
  hundred megabytes, not correctness. Numbered on 2026-09-19 by milestone 433's drain of the pile.
- **Milestone 288.** `crates/elf`'s host tests assume the host is aarch64, so `script/test` cannot go
  green on an x86_64 Linux box: twenty of twenty-five tests fail at the machine check and the
  host pass never reaches the kernel legs. Found here because this is the first machine in the
  project's history to gate from a host that is not aarch64; it is pre-existing on `main` and this
  milestone touches no Rust. The crate exports `NATIVE_MACHINE` for exactly this and its own test
  `Builder` hardcodes `EM_AARCH64` instead, while a second test's doc comment states the false
  premise out loud (*"these host tests build with `EXPECTED_MACHINE == EM_AARCH64`"*). Written up as a proposal, which
  milestone 288 has since absorbed and closed (`design/roadmap/0288-host-tests-that-assume-an-aarch64-host.md`);
  the proposal file was deleted at that merge, 2026-09-14, as one of two written eight days apart
  for the same defect. It matters to this
  milestone's own principle: 287 removes the first wall a Linux newcomer hits and this is the
  second one, louder, because twenty red tests in a crate named `elf` read as "this project is
  broken".
- **Recorded.** The six limitations above stay limitations and live in this block's `BUGS`, which is
  where the next person changing the fragment meets them. The one most likely to bite is the first:
  a macOS regression would show up as `script/bootstrap` doing something surprising on the architect's
  own machine, and no gate in this repository runs on macOS.

## Index row

Reproduced whole on a stock Ubuntu box. Milestone 117 (the stranger test)'s stranger run found
`script/setup` could not complete on cold Linux; the remedy applied was a better error message, and
three defects sat on it. The message never printed (`[ "$os" = linux ]`, while `uname -s` says
`Linux`). Printing is rung four. And the two commands it printed looped: `script/ci-qemu` installs
into `$HOME/.cache/nife-qemu` and the only thing in the tree that ever put that on PATH was
`ci.yml`, so twelve minutes of building ended at `/usr/bin`'s 8.2.2 again, forever.
notes/stranger-test.md had recorded it fixed and was wrong for twenty-eight days. bootstrap now runs
`ci-qemu` itself; `helpers/qemu-path.sh` (**name provisional**) is the PATH half, sourced rather
than executed because the emulator is named bare from 38 sites and inheritance reaches them all;
`script/lint` gates that every entry point resolves it.
