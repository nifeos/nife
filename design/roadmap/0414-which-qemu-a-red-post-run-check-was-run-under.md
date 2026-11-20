---
status: BUILT
built: 2026-10-16
raised: 2026-09-14
promoted_from: which-qemu-a-red-post-run-check-was-run-under
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 414. A red post-run check does not say which emulator produced it

Promoted from the proposal `which-qemu-a-red-post-run-check-was-run-under`,
filed 2026-09-14 by milestone 288 while establishing that a red kernel leg was the environment
rather than its change, on a box that happens to have two QEMUs installed. *(Number provisional
until the merge queue lands it.)*

A lane can close this. It is a line of output, and the question underneath it is
where the version is read rather than whether to print it.

**Premise re-checked 2026-09-19, true, with one correction.** *(2026-09-24: #1211 added the
emulator-dependence entry to the main page's `BUGS` in
[notes/load-sensitive-assertions.md](../../notes/load-sensitive-assertions.md), naming the 8.2.2 and
11.0.2 split; the version line stayed open.)*

## Built 2026-10-16 (UTC), lane `milestone/414-which-qemu-a-red-post-run-check-was-run-under`

The maintainer's ruling took both halves of the open question, and the implementation found no
reason to argue back. The version is printed once per leg, in the per-architecture banner
(`--- kernel tests, aarch64 (QEMU 11.1.2) ---`; riscv64 the same; x86_64 keeps its machine word,
`QEMU q35, 11.1.2`). Every red post-run check also carries the version on its own failure line:
the three `scanout` verdicts and the inbound prober's three failure arms. Green lines and the
checks' diagnostics are unchanged, so the cost is one string per red line plus a handful of
`qemu-system-<arch> --version` reads, milliseconds against runs measured in minutes.

One function reads the version, `host::qemu_version` in `xtask/src/host.rs` (name provisional,
minted by this lane). It resolves `qemu-system-<arch>` through PATH exactly as the runner's bare
`exec` does, so the number names the emulator that actually ran rather than one a config elsewhere
prefers. Nothing new learned how to find a binary: PATH is still the mechanism
(`helpers/qemu-path.sh`), and `.qemu-version` stays the pin, not the report. The parse anchors on
the word `version` rather than on field position, because `script/qemu-check`'s `awk '{print $4}'`
already lands inside the package suffix on distro builds (`qemu-system-aarch64 version 8.2.2
(Debian ...)`). A binary that cannot be asked prints `version unreadable` rather than guessing.

The banner's one parser, `helpers/booted_rows.py`, matches both the versioned and the bare
parenthetical now, and its selftest asserts both, so old logs still attribute their legs. X86_64's
machine word keeps that leg on the file-name fallback, as before. Honest limit: a version line
identifies the emulator, it does not make the checks less emulator-dependent. The disjoint failure
sets in the table above are still there to fall into. What changed is that a red line now names
which set it came from. `notes/load-sensitive-assertions.md`'s `BUGS` says the same where a reader
of a red check meets it.

## Follow-on

- **Milestone 930.** The red lines one harness out have the same hole, and this lane did not touch
  either: 412's lane owns the `swish-check` text and the UEFI lines are 516's. A red `swish-check`
  failure names `sendkey` and the monitor socket, both of which the table above shows behaving
  differently across versions. A UEFI failure line classifies the exit status without the version,
  though [notes/uefi-test-exit-rate.md](../../notes/uefi-test-exit-rate.md) exists because of a
  QEMU-version theory. The work is milestone 930 (a red swish-check or UEFI line does not say which QEMU ran it). A lane can claim it:
  `design/roadmap/0930-a-red-swish-or-uefi-line-does-not-say-which-qemu-ran-it.md`.

## What was measured

One tree, one kernel binary, one `script/test --arch aarch64`, two emulators on PATH in turn:

| Emulator | `a_keystroke_from_a_virtual_io_keyboard_becomes_a_terminal_byte` | the three `scanout` checks |
|---|---|---|
| apt's 8.2.2 | **FAILS**: `sendkey` never reaches the device | **pass**, pixel for pixel |
| the pinned 11.0.2 | **passes** | **FAIL**: *"no screendump was ever taken (did QEMU get a monitor?)"* |

`inbound` and `multicast` fail under both. The two versions fail disjoint sets of host-side
checks, and neither set is empty, so "the host-side referees fail here because the box is headless"
is true and is not enough to identify anything.

## Why it matters more than it looks

The post-run checks are this project's only evidence for the things a guest cannot witness about
itself: that pixels reached the *device*, that a keystroke crossed the monitor socket, that a host
process could connect into the guest. `script/test`'s own output already goes to some length to
make a red one readable (the `inbound` check prints the attempt histogram and tells the reader to
read the timestamps first). The emulator version is the one input to those checks that changes the
answer and is not in the report.

The concrete cost, paid twice in two days. A lane sees a red post-run check, cannot tell an
environment failure from its own regression, and spends a control run finding out. The block for the
`script/bootstrap` milestone landing with [#847](https://github.com/nifeos/nife/pull/847)
records three `scanout` failures on this box. Milestone 288 (host tests) saw them pass there and a different
check fail. Both accounts are correct and they read as contradicting each other.

## What would close it

`script/test` already resolves the emulator it runs. Print its version beside each post-run check
result, or once in the per-architecture banner, so a red line carries the one fact needed to
interpret it. `script/qemu-check` knows how to read a version and `helpers/qemu-path.sh` knows which
binary was chosen, so nothing new has to learn how. The question is which of the two is the right
place, and whether the banner is enough or each failing check should carry it.

**A smaller version of the same fix, if the banner is judged enough.** Say in the `BUGS` of
notes/load-sensitive-assertions.md that the post-run set is emulator-dependent, so at least a reader
who goes looking finds the table above rather than deducing it a third time.

## Index row

The post-run checks are this project's only evidence for what a guest cannot witness about itself:
that pixels reached the device, that a keystroke crossed the monitor socket, that a host process
could connect into the guest. A red one does not say which emulator produced it. Measured on one
tree and one kernel binary with two QEMUs on PATH in turn, the versions fail disjoint sets of those
checks and neither set is empty, so "the referees fail here because the box is headless" is true and
identifies nothing. The cost was paid twice in two days: two correct accounts of the same box read
as contradicting each other, and a lane spends a control run distinguishing an environment failure
from its own regression. `script/test` already resolves the emulator it runs, so the work is where
to print the version rather than whether to.
