# Stranger test run 6, 2026-09-19

*An appendix to [the stranger test](../stranger-test.md), which holds the current state. This page
is the record for run 6: the design decided before the run, and what it found.*

## What run 6 changes, decided 2026-09-19 before the run

Nothing about the instrument changes, on purpose. Run 6 is `script/stranger-test --commit
origin/main`, the same command as run 5, with the task text, the debrief and the rubric exactly as
run 5 left them. It is the first run in thirty-two days and the first the cadence owed: `--due`
exited 1 on 2026-09-19 with "Run 5 was 32 days ago and the cadence is 30". Whether the cadence
*caused* this run is answered in the run's record, not assumed here.

### What is new is the tree

About 2,500 commits landed between run 5's clone (`a5a75f46`, 2026-08-19) and this one (`ebe6b784`,
2026-09-19), touching 714 files. The changes a stranger doing this task is most likely to meet:

- Milestone 150 (adding a program should not need eight hand-maintained lists) was BUILT today. A
  program is one `[[bin]]` block instead of eight hand-maintained lists, and
  `notes/adding-a-program.md` was rewritten around it. Three strangers nominated this, and run 6 is
  the first to walk the result, on the day it landed.
- Milestone 175 (split `user/`) split `user/` into `components/` and `fixtures/`, so every earlier
  run's directory names are stale in its own record.
- `script/gates` is gone. Milestone 286 (one enumeration of the checks that gate a pull request)
  replaced it with `script/ci-build`, whose table is the one list of checks. `CONTRIBUTING.md` now
  calls it the one command to remember. Run 4's best unforced result came from that paragraph's
  predecessor.
- Run 5's handoffs landed. `std-aborts` now asserts its dep-info paths are under this worktree's
  `farm_dir()`, which is the defect that contaminated run 5 in its first ten minutes.
- Naming moved to `design/naming.md` under §155 (the naming conventions move out of the
  constitution), and most of the tree's names were ratified on 2026-09-14. `AGENTS.md` is 65 KB,
  about what it was.
- `script/apropos` is still not named in `README.md`'s start-here order or in `CONTRIBUTING.md`.
  This lane moves it there, and the clone is cut from `main` before that change, deliberately. So
  run 6 measures the placement run 5 measured, and the next run measures the fix.

### The machine is shared

Two other lanes are running QEMU on this machine (the stick program, and milestone 182 (x86_64's own
interactive-boot entry point)). A timing leg that goes red here may be the host. The load-average
print that run 3 bought has never fired in a recorded run, and this is its best chance yet. One
deliberate contamination: the harness runs with `VERIFY_JOBS=2` in its environment. A stranger that
runs `script/verify` then shares the machine's memory budget with the other two lanes instead of
taking all of it. The stranger can see that only by running `env`.

### Four predictions

1. The stranger does not run `script/apropos`, and does not open `notes/net.md`, so M2 is absent or
   partly answered for the fourth run in six.
2. It adds a program, and the walk of `notes/adding-a-program.md` finds at least one thing wrong,
   because the page was rewritten today and no stranger has walked it.
3. `script/test` is green without any change to the tree. If it is red, the failure names this
   machine rather than accusing another checkout's source, because run 5's `farm_dir()` fix is in.
4. It reads `CONTRIBUTING.md` before `AGENTS.md`, and opens `AGENTS.md` late or not at all, as run 5
   did.

## Run 6, 2026-09-19: a different model, a program in two edits, and a stranger that provisioned the machine

### The cadence is not what started it

`--due` had been exiting 1 since 2026-09-17. But the weekly workflow runs on Mondays and was green
on 2026-09-14, so nothing could go red before 2026-09-21. This run happened because a maintainer
briefed a lane. That is a finding, and milestone 495 (a
cadence that says "due" reaches nobody) is its proposal, not started as of 2026-09-24.

### Three attempts were refused before any measurement

`script/stranger-test --commit origin/main` with the CLI's default model, `claude-opus-5[1m]`,
stopped at the third turn twice. `--model opus` (`claude-opus-5`) stopped the same way once. Each
said: *"safeguards flagged this message ... `[reasoning_extraction]`"*, having read `README.md` or
`git log` and nothing else. `--smoke` on the same model passed, so the task text trips it, not the
tree. Each refused attempt cost about `$0.17`. The harness then debriefed the dead session anyway,
which is now fixed: it stops instead. Run 6 is the first run on a model chosen by the operator,
`--model sonnet` (`claude-sonnet-5`). Runs 1 to 5 did not record their model, so a difference
between runs 5 and 6 may be the model as much as the tree.

### The configuration, verified rather than asserted

The clone was `ebe6b784` (`7889a8cc` after the withholding amend), answer key absent, tree clean,
probe `NONE`. The stranger's answer to the first debrief question agreed: *"No CLAUDE.md, AGENTS.md,
or equivalent was pre-loaded, and no user/project memory referencing nife or any prior run was
present."* `VERIFY_JOBS=2` was in its environment, as registered; it never ran `script/verify`. Two
other lanes were running QEMU, and the load average ran between about 2.5 and 7.4 on 8 cores. It
took 281 assistant turns, 18:25 to 18:44 UTC for the task plus four minutes of debrief, and `$6.54`
for the stranger process alone.

The summary's `reached_withheld_note` warning fired, and it is the known false positive. The
transcript names the file because `script/lint` printed it (below) and because the stranger grepped
for references to it. It never ran `git show` or anything else that would recover it.

### Two operator interventions

The protocol says no help mid-run, and a run that hides help is not a measurement. Protecting the
machine outranked the protocol once. About eleven minutes in, the stranger decided an x86_64 failure
(below) was a QEMU version mismatch. It ran `brew install meson`, downloaded QEMU 11.0.2, and
started building it into `$HOME/.cache/nife-qemu`. `helpers/qemu-path.sh` puts that prefix on PATH
for every checkout on the account whose `.qemu-version` matches. So the two lanes gating beside it
would have changed emulators mid-suite.

The operator killed the build and left the directory read-only. The stranger met the permission
error and called it *"cause unclear - possibly a leftover from the aborted first attempt"*. It ran
`chmod u+w`, rebuilt with two extra configure flags, and installed. After the run the operator
removed the prefix and uninstalled `meson`. So the one thing a newcomer on their own machine would
not have been stopped from doing, it did anyway. The intervention only cost it a rebuild and a
confusing permission error.

The second intervention is smaller. The operator re-ran `cargo xtask uefi-test` after the run to
check the stranger's diagnosis. That is scoring, not help, and the stranger never saw it.

### The build half

B1 passes, and for the first time a stranger opened a `design/decisions/` file. Its order, from its
own debrief: `README.md`, `rust-toolchain.toml`, `.cargo/config.toml`, `notes/adding-a-program.md`,
`notes/capabilities.md`, `CONTRIBUTING.md`, `design/roadmap/README.md` (part), `AGENTS.md` lines 1
to 140, `notes/program-manifest.md`, `design/decisions/README.md` and
`0158-a-program-is-declared-once.md` (part), `AGENTS.md` again (lines 692 to 851),
`design/naming.md` (part), `notes/grant-expression.md`, `design/fatal-risks/README.md`, then source.
It went to item 7 of the reading order fourth, because that is the page for its task, and used the
order as an index, as run 4 did. The stranger opened `AGENTS.md`, unlike in run 5, and read it in
two slices.

B2 is not measured, as registered. The toolchain was pinned and present, and its first entry says
*"this sandbox was clearly prepared in advance for this repo"*. What it measured instead is what a
newcomer does when the machine is *not* right for the pin: it built the pinned QEMU itself.

B3 passes on two architectures and failed at the last step of the third. `cargo xtask test` was
green on aarch64 (325 passed, 2 skipped), riscv64 (327 and 2), x86_64 under PVH (244 and 42), and
the NVMe-behind-a-root-port leg. Then `uefi-test` printed `test result: ok. 215 passed, 71 skipped`,
followed by `uefi-test: qemu exited Some(1), not 3`. Two QEMU lines just before it read as the
cause: `vtd_iova_to_sspte: detected sspte permission error` and `vtd_iommu_translate: detected
translation failure`. They are not the cause. The operator's re-run, on the same tree, machine and
Homebrew QEMU 11.1.1, printed the same two lines and passed. They are the deliberate DMA-escape
tests' faults, and nothing says so where they are printed.

So the stranger's headline finding, that QEMU 11.1.1 against the 11.0.2 pin *"flips a real pass/fail
gate"*, is a confident wrong answer, and the tree induced it. It had a red gate, an unexplained
error line, a documented version mismatch, and a comment in `script/qemu-check` saying the mismatch
only shifts benchmark numbers. At debrief it noticed that it had never confirmed the fix. That is to
its credit, and it did not stop it building an emulator first.

B4 fails, with six entries:

1. A bare `cargo build --workspace` fails on the host, with an error that reads like a broken tree.
2. The test log is binary to `grep` without `-a`.
3. The pinned QEMU cannot be had on macOS by any documented route.
4. Building it by hand needs `--disable-cocoa --disable-pvg`.
5. `helpers/qemu-path.sh` honors a hand-built prefix on macOS too.
6. The VT-d lines above are expected.

### The mental model, scored: seven answered, one absent

| # | result | where it came from |
|---|---|---|
| M1 | answered | `notes/capabilities.md`, quoted: the slot indexes a table the process cannot write, *"That is the entire security mechanism"*, no separate check |
| M2 | absent, for the fourth run in six, and it said so | looked in `notes/capabilities.md`, `notes/grant-expression.md` and `design/fatal-risks/README.md`; inferred a userspace `net_stack` from test names and said it was inference. *"I never opened a notes/network.md or equivalent - if one exists, I didn't find it."* |
| M3 | answered | `AGENTS.md` rule 1, with the diff-across-every-file consequence, and `CONTRIBUTING.md`'s restatement |
| M4 | answered | `design/roadmap/README.md`'s vocabulary table, and that the column is generated from each block and checked |
| M5 | answered | `CONTRIBUTING.md` and `AGENTS.md` rule 7, and `design/naming.md`'s crate-and-program-share-a-name sentence |
| M6 | answered, quoted | `CONTRIBUTING.md`, then three `BUGS` sections seen in use |
| M7 | answered by doing it, and the page held | added `quadruple` through milestone 150's shape: the `[[bin]]` block, then the four shell-spawnable edits, each omission caught by `cargo build` or a named host test. Its live-prompt check was killed at session end, so it never saw the program answer |
| M8 | answered | `AGENTS.md` for who decides, `notes/adding-a-program.md` for the four states |

M7 is the result three strangers asked for. Runs 3, 4 and 5 each said that adding a program should
not need eight hand-maintained lists. Milestone 150 landed the one-place declaration the day of this
run. The first stranger to walk it found nothing wrong with the page, and confirmed that every edit
the page names is one the machine demands. Its words at debrief: *"every one of these omissions was
caught immediately ... matching what the doc claims, not something I had to take on faith."*

### What it found, ranked by what it cost, and where each one now lives

1. The x86_64 `uefi-test` red and the diagnosis it invited. This cost it the rest of the run: two
   QEMU builds, a `brew install`, a machine-global write, and a re-test it never saw finish. Homes:
   `notes/x86-uefi-boot.md`'s `BUGS` (the red after a passing suite, and that the VT-d lines are
   expected). `notes/qemu.md`'s new `BUGS` (the macOS pin gap, the configure flags, and the
   account-wide prefix). `script/stranger-test`'s `BUGS` (a stranger can write to the machine). Why
   QEMU exited 1 rather than 3 is unmeasured; it is milestone 516 (`uefi-test` can exit 1 after its
   own suite has passed), not started as of 2026-09-24.
2. The session ended with no write-up. It backgrounded a re-test and a `script/swish-check`, wrote
   *"Waiting for both background runs to complete"*, and `claude -p` ended the run there, killing
   both. The debriefs recovered its findings because they resume the session, but the deliverable
   was never written. Home: `script/stranger-test`'s `BUGS`, with two remedies, both the operator's
   call.
3. `README.md` said QEMU is pinned to an exact version on every machine, and on a Mac it is not. The
   stranger quoted that sentence as the worst thing about the repository. Fixed in this lane on
   2026-09-19: the sentence now says where the pin holds and where it does not.
4. `CONTRIBUTING.md` said the project has two ISAs, in five places, while every other document and
   the build say three. Its first confident wrong claim, found in minutes. Fixed in this lane on
   2026-09-19.
5. `script/lint` in the clone announces the withholding. It prints *"notes/stranger-test.md is
   missing, so there is no rubric to ask and no answer key to withhold."* The stranger read it and
   concluded an answer key had been withheld. Home: `script/stranger-test`'s `BUGS`.
6. A bare `cargo build --workspace` fails on the host with "unwinding panics are not supported
   without std". Fixed in this lane on 2026-09-19: `CONTRIBUTING.md` says so where a person first
   builds.
7. The serial log is binary to `grep`. Home: `notes/qemu.md`'s `BUGS`.
8. The journal landed in the harness root, not beside the clone. Home: `script/stranger-test`'s
   `BUGS`.

What did not recur is the comparison runs 3 to 5 make possible. Run 5's machine-global `std-aborts`
contamination did not happen: `script/test` built its own farm, and the `nife-dev` link was restored
afterwards. `notes/adding-a-program.md`, stale on arrival for runs 2 and 3, was right for the second
time running. The eight-list complaint, made by three strangers in a row, was not made.

What did recur: M2 is absent again. `notes/net.md`, `notes/scripts.md` and `notes/README.md` went
unopened for the sixth run. `script/apropos` was never run; its name appeared in the transcript only
as text inside files the stranger grepped. It was placed in `README.md` and `CONTRIBUTING.md` on
2026-09-19, after this run's clone was cut, so run 7 measures that placement.

Its worst-thing answer is runs 4 and 5's criticism, arriving a third time with a new instance. In
its words: *"finding it required wading through hundreds of files of dense, self-referential, often
beautifully-argued prose that costs real time to read and doesn't, on this evidence, reliably
converge on the actual state of the build."* The instance is weaker than it claims, since the gate
it said QEMU broke was not broken. But the mechanism it describes is the one that produced its
mistake: the true fact, that those VT-d lines are expected, was written nowhere it looked.

### The four predictions, scored

1. *No `script/apropos`, no `notes/net.md`, M2 absent or partly.* Confirmed on all three. Placement
   was unchanged in the tree it was handed, by design; this lane placed it, and run 7 measures that.
2. *The walk of `notes/adding-a-program.md` finds at least one thing wrong.* Falsified. The page
   milestone 150 rewrote that morning held.
3. *`script/test` green with no change, or a red that names this machine.* Falsified, in a third
   direction: red on one leg, intermittently, with an error that named nothing and a nearby line
   that pointed at the wrong culprit.
4. *`CONTRIBUTING.md` before `AGENTS.md`, `AGENTS.md` late or not at all.* Confirmed on the ordering
   (sixth and eighth), and falsified on "not at all": it read two slices.

### What this run cost

The largest contamination is the model change, which is new and cannot be separated from anything
else that moved since run 5. The others:

- The operator intervened once, for the machine rather than the run, and the stranger routed round
  it.
- The tree told it that it was measured, from `README.md`, `CONTRIBUTING.md`'s `BUGS` and
  `notes/adding-a-program.md`'s table. It named the effect: *"part of my attention went to the fact
  of being measured rather than to the kernel"*. It went looking for doc drift because the tree
  primed it to.
- Its journal caught the house style, and it said so.
- No write-up exists.
- The operator has read `AGENTS.md` in full, as every operator has.

The mitigations are the usual three: the task text and rubric are unchanged, the predictions were
committed before the clone was cut, and the answers above are the stranger's words. Cost: `$6.54`
for the scored run, about `$0.52` for the three refused attempts, and `$0.06` for a `--smoke` that
showed the refusals came from the task text.
