---
status: BUILT
raised: 2026-09-14
built: 2026-09-14
---
# 289. Does milestone 20's RISC-V tour still earn its place?

calef asked what `components/src/builder.rs` does and why it is still needed "in a world where we
boot to swish".

**The answer is keep, and the evidence changed the question.** The brief leaned toward retirement on
a premise that reads well and is false: that booting to `swish` means `builder` never runs. It does
mean that on aarch64, and it means it under `--features shell` on riscv64. It does not describe the
**default** riscv64 build, which is the one a board boots, the one `script/soak` and `script/job-mix`
run, and the only riscv64 build that exists outside a QEMU gate.

## What was asked, and what each question answered

### Does anything actually depend on the tour build?

Yes, four things, none of them a note.

`script/board-image` builds the tour. Its default is `FEATURES="board"`, and it takes no flag that
would build the shell kernel: the four modes are the plain tour, `--soak`, `--jobmix` and `--bench`,
and the last three *replace the end of* the tour rather than skipping it, which the script says in
its own argument check ("all replace the end of the boot tour; pick one"). So every VisionFive 2
(radon) boot this tree can produce runs the tour. `riscv_shell_boot` is `#[cfg(feature = "shell")]`,
`script/swish-check` is the only thing in the tree that builds that kernel, and it builds it for
QEMU, so nothing here produces a board payload that would run `progenitor` and no bench record in
`notes/visionfive2.md` shows one having run.

`crates/board_console` reads the builder's output line. `Progress::userspace_ran` is set by
`line.contains("init/build")` (`crates/board_console/src/progress.rs`), which is the line
`kernel/src/main.rs` prints when the builder's child reports 81. It is asserted by four host tests
and appears in three captured transcripts, one of them off the board itself:
`tests/fixtures/captured/vf2-2026-09-01-userspace.log`, whose filename is the word `userspace`
precisely because this program running is what distinguishes that capture from the others taken the
same day. `script/board-console` prints it as its own summary line, and `notes/board-console.md` states flatly that it is "the only
difference between the two successful captures": with an archive on the card, and without.

`script/soak --arch riscv64` and `script/job-mix --arch riscv64` both boot it with the archive.
Each builds `initrd_riscv()` (which packs `builder`) and runs the whole tour before the workload
starts. The soak is `design/fatal-risks/README.md`'s fifth-entry rehearsal, which is the tie-breaker
AGENTS.md names while the customer path is vacant.

The VisionFive 2 bring-up used the tour, with an initrd, and the builder step is what boot stages
4 and 5 bracket. This settles the claim the maintainer told calef and then withdrew as unchecked.
It was right. `notes/visionfive2.md`'s fifth stop resolves boots 7 through 9 with five independent
identifications, and three of them are facts about `builder` specifically: its exact syscall count
(14 of the boot's 20 ecalls, itemized), the retype kinds it issues (`ASPACE`, `FRAME`, `TCB`, **never
`ENDPOINT`**, which is what pinned the endpoint-naming order), and that it issues no receive of any
kind, which is what made the parked receivers in the dump impossible to attribute to it. The
breadcrumbs were added for boot 11 to bracket a failure that was *initrd-path-coupled*, which is to
say coupled to this step.

Booted, rather than read off the source. On 2026-09-14, in this lane, with QEMU 11.0.2:
`cargo xtask initrd-riscv`, the default release kernel, `helpers/qemu-bounded.sh 45`:

```
  init/build  : the userspace builder loaded 'least_authority_demo' from a 7648768-byte archive
                and built it as a child; the child sent 81 (expected 81)
  ...
nife: the capability core runs on RISC-V.
```

and that transcript fed straight back through `script/board-console --replay`, which answered
`reached boot tour complete` and `the userspace progenitor built its child`. So the whole chain is
live end to end: the kernel loads `builder`, `builder` composes a process, and the recognizer this
tree already ships reads the result.

### Is anything proven only by the trimmed path?

Yes. `builder` is the only userspace-loads-userspace demonstration that reaches real silicon, and its
trimming is the reason it can.

`progenitor` proves the claim far harder in QEMU: console server, input driver, line discipline,
shell, terminal sink, job undertaker, one program on three architectures since milestone 266, gated
by `script/swish-check`'s riscv64 leg in CI. If the board could boot it, this milestone would have
retired the tour. The board cannot. `riscv_shell_boot` needs the PLIC initialized, the NS16550's
registers delegated as a `DeviceFrame`, and the UART interrupt routed, and **the UART's PLIC source
number is not the same on the board as in QEMU** (10 on QEMU `virt`, 32 on the JH7110; it was a
hardcoded QEMU constant until `user::uart_irq_and_source()` learned to read the machine's own tree,
and `notes/visionfive2.md` carries it in BUGS). `builder` takes an untyped budget and a report
endpoint and nothing else: no interrupt controller, no device capability, no IRQ delegation. So on
silicon it separates "the capability core composes a process" from "this board's interrupt wiring is
right", and the tour's own history is that the second one is the half that breaks.

## What this milestone changed

Nothing was deleted. One defect was found on the way, and three records were made to say what is
true of the system today.

The breadcrumb table was wrong about the two stages that matter most to a triage. `main.rs`'s
table read `10 = the final banner printed; halting` and stopped there. Stage 10 is set at the end of
the hardware-entropy step, *before* the banner; stage **11** is the one that means the tour finished,
and it is what `user.rs`'s hang watcher keys on (`boot_stage() >= 11`). A reader triaging a board log
that reported stage 10 would have concluded the boot completed when it had not, which is the exact
class of mistake the breadcrumbs exist to prevent. Milestone 159 added the step and moved the
meaning; the table it moved past was never updated. Corrected, with stage 11 named.

`components/src/builder.rs` now says what rests on it, at the thing itself rather than in this
block, because the next person to ask calef's question will be reading that file and not this one
(AGENTS.md's ladder, rung three). It carries a `BUGS` section for the first time.

The tour's own comment now names `crates/board_console` as a consumer of its output line, so
"nothing reads this" is falsifiable from the call site.

## What is an architect's, and is not decided here

**The category.** `builder` sits in `components/`, and milestones 39 and 175 both listed it as a
`fixtures/` example. Milestone 175 is explicit that those lists classified `builder` and `worker`
"by repetition rather than by ruling", and calef broke the identical tie for `worker` on 2026-09-13
by ruling it the canonical minimal program and keeping it in `components/`. The recommendation here
is the same answer with a stronger reason: the kernel measures `builder` against the trust root on
the default riscv64 boot (`kernel/src/user.rs`'s `trust::require("builder", ...)`), which is what
it does for the first process and for nothing else.

The obvious objection, stated rather than left for a reader to find. `boot_programs()` is
`["progenitor", "builder", "hello"]` on riscv64, and `hello` lives in `fixtures/`, so "it is in the
trust root" is not on its own a classification. `xtask`'s own doc says why `hello` is there:
`spawn_progenitor` enters it directly for milestone 19d's test roles, and `trust::require`
refuses any entry the trust root does not name, so the entry exists rather than the check being
relaxed. That is a test path. `builder` is entered on the default boot, with no test build and no
role argument, which is the distinction the recommendation actually rests on.

The one record that disagrees is `notes/trusted-init.md`, which groups it with "test or demo programs
rather than the shipped system"; that sentence is about which loaders extend the measurement chain,
and it is correct about that. This milestone added a caveat there about where the first of the three
runs.

The name. `builder` is on `script/names --unratified`. It is a generic word of exactly the class
AGENTS.md names to avoid, and its own provenance block admits it was "never argued for directly".
Proposed provisionally, with the refusals: see the block in `components/src/builder.rs`.

## Follow-on

- **Milestone 406.** No pull-request check boots the default riscv64 kernel with its archive,
  so the tour's builder step, its device-IRQ step and its banner are asserted by nothing that runs
  automatically. That is what made a live step look dead. Numbered on 2026-09-19 by milestone 433's
  drain of the pile, with the headline narrowed: `script/boot-check` (milestone 268, the same day
  this bullet was written) does boot the default kernel on every pull request, with no initrd, so
  what is left unasserted is the tour past the self-test verdict.
- **Recorded.** The unmeasured child load stays in `notes/trusted-init.md`'s "Still not covered",
  where it already lived, with one caveat added there rather than given a second home: that note
  groups the three uncovered loaders as "test or demo programs rather than the shipped system", and
  on riscv64 the first of them is the program the kernel loads and measures on the build that goes to
  a card. It had a second home in the `builder` program's own `BUGS`, and that home is gone: calef
  retired `builder` on 2026-09-14 (milestone 295) and the file went with it. The note is now the only
  copy, which is where this bullet said it already lived.
- **Done.** Milestone 295 carried this, by answering it out of existence rather than by choosing.
  The bullet asked for two calls from calef, the category (`components/` versus `fixtures/`) and a
  replacement for the generic name `builder`; on 2026-09-14 he retired the program in one sentence,
  so neither question has a subject any more and `script/names --unratified` no longer lists it. The
  write-up lived in the program's own provenance and `BUGS` blocks and went with the file; commit
  `0fa40ee8` is where a reader can still read it.

*(Both bullets above were edited by milestone 311's lane, which is not this block's own, and the
edit was forced by a gate. They cited a file milestone 295 deleted, and `script/roadmap --check`
fails a `**Recorded.**` bullet naming a path that does not resolve. It had been passing only because
that check's list of real directories still said `user/`, which milestone 175 split in two, so it
did not recognise a `components/` citation as a path claim at all. Milestone 311 derives the list
from the tree; these two bullets are the first thing it found.)*

## BUGS

- Nothing in CI asserts that the builder step ran. `script/test`'s riscv64 leg returns at the
  `#[cfg(test)]` arm before the tour; `script/cpu-matrix` runs that same suite; `script/swish-check`
  boots the shell build; `script/bench --riscv --check` parks before the tour. The only callers are
  `script/soak-test`, `script/job-mix` and a board, and none of those runs on a pull request. This
  is why the step looked vestigial: it is not unused, it is **unasserted**, and the two are
  indistinguishable from a grep. Written up as milestone 406,
  `design/roadmap/0406-nothing-in-ci-boots-the-riscv-tour.md`.
- `builder` loads its child unmeasured. Already recorded in `notes/trusted-init.md`'s "Still not
  covered", and this milestone raises what it costs rather than fixing it: that note prices the gap as
  affecting "test or demo programs rather than the shipped system", and on the board path `builder` is
  the first process. The remaining work there is the call, not the data.
- **This milestone measured the riscv64 tour and did not boot it on silicon.** Every claim above about
  the board is read from `notes/visionfive2.md`'s captures and from the source; radon was not at this
  lane's bench.

## Index row

calef asked what `components/src/builder.rs` is for in a world that boots to swish. The answer is
keep, and the premise was false: booting to swish describes aarch64 and
riscv64's `--features shell`, not the default riscv64 build, which is what `script/board-image` puts on a card. `progenitor` has never run on RISC-V silicon; `riscv_shell_boot` needs the PLIC, the NS16550 delegated and a UART source number that differs
between QEMU (10) and the JH7110 (32), and `builder` needs a budget and a report endpoint and
nothing else. `crates/board_console` parses its `init/build` line into `userspace_ran`, asserted
by four host tests and two captured board fixtures, and `notes/board-console.md` calls it the only
difference between the two successful captures. The VF2 bring-up did use the tour with an initrd:
three of the fifth stop's five identifications are facts about `builder`. Found the breadcrumb
table wrong about stage 10 (set before the banner, not after) and silent about stage 11 (what the
hang watcher keys on).
