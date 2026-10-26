# Stranger test run 5, 2026-08-18

*An appendix to [the stranger test](../stranger-test.md), which holds the current state. This page
is the record for run 5: the design decided before the run, and what it found.*

## What run 5 changes, decided 2026-08-18 before the run

This is the first run conducted through `script/stranger-test` rather than rebuilt by hand. The lane
that wrote the harness deliberately ran nothing through it, so only its author and a `--smoke` reply
had exercised it. Run 5's brief is one line, `script/stranger-test --commit origin/main`. The first
thing it measures is the harness: whether the isolation, the withholding, the shims and the
toolchain restore hold when someone who did not write them runs them. A harness defect is recorded,
not worked around, because a run that patches its own instrument mid-flight measures something it
can no longer name.

### The disclosure is live for the first time

Run 4's handoff 5 said to tell the stranger it is being measured, since the tree leaks that within
half an hour anyway. The harness puts it in the task text, so this is the first stranger to know at
turn zero, and it is not comparable to runs 1 to 4 on anything that knowledge touches. The task also
asks it not to perform, a second new variable in the same paragraph. If the result is good, the two
cannot be separated, and this record should credit neither.

### What is new in the tree: one command

`script/apropos` landed on 2026-08-18 because of this instrument; the results below say why. Whether
the stranger discovers it unaided is the measurement, so the task text does not mention it. The one
place a newcomer might meet it is `notes/scripts.md`'s table, behind item 8 of the README's reading
order, and item 8 points at `notes/README.md`, which no stranger has opened.

Run 4 was the first to see `CONTRIBUTING.md` and the `## Start here` order. The failure to watch is
run 4's: `CONTRIBUTING.md` is item 2 of 8 and was read sixteenth of twenty-two.

### The machine is uncontended

Run 3 gated at a load average of 45 to 63 and run 4 at about 3 to 17.5, beside other lanes. At this
run's registration the load average is 4.04 on 8 cores, with no other lane and no QEMU running. So a
red timing leg here cannot be blamed on the host.

### The rubric is not amended by this lane

Run 4's lane edited the table before running against it, and said that made things slightly worse.
Run 5 grades against the table as run 4 left it. If this run falsifies a row, the correction is
recorded for the next lane to apply.

### Three predictions

1. The stranger does not find `script/apropos`, and therefore still does not reach any file under
   `design/decisions/`.
2. `notes/README.md` goes unopened for the fifth run running.
3. The build half is green first try with no change to the tree, and B2 measures nothing again.

## Run 5, 2026-08-18: the first run through the harness, and the machine leaked where the tree did not

### The harness worked

One command, with a `--smoke` run before it. None of the four isolation failures that runs 1 to 4
each hit recurred. The `nife-dev` link was put back after the run, which `readlink` confirmed. The
`--smoke` run cost `$0.09` and 3 turns and exercised every stage, including both debriefs. The real
run cost `$10.56`, 201 assistant turns and about 29 minutes.

### The machine leaked into the clone

Two things leaked anyway, and neither is the tree. `nife-dev` is an account-wide `rustup` link. So
the clone's first `script/test` compiled `std` out of whichever worktree had built the farm last,
which was `/Users/calef/projects/nife/target/nife-farm`. The `-Zbuild-std` dep-info cached those
absolute paths, and cargo then considered the unit fresh. `std-aborts` failed, naming two files and
two line numbers inside another checkout on this machine.

That led to the second leak. A grep for the foreign path found one file in the tree containing it:
the harness. So the stranger read `script/stranger-test`, about thirty minutes in, which is a
complete description of the instrument it was inside. It did not go looking. The task text says
*"the repository is in ./nife and it is the only thing you have; do not look for another checkout on
this machine"*. Its own build handed it another checkout's path, and it ran `ls -la` against that
path to see whether it was real. The harness's paragraph about restoring the link then rescued it
from a wrong repair. The harness both contaminated the run and saved it. Recorded in
`script/stranger-test`'s `BUGS` and in notes/std.md's.

### What the disclosure did

This is the variable the run exists to measure. The stranger was told at turn zero that it was being
measured, per run 4's handoff 5, and asked not to perform. Both halves partly failed, and it said so
without being pressed.

- The knowledge chose its program. Before picking, it wrote in its journal that "repeating the shape
  those walks used would measure nothing". It then selected the one manifest combination nothing in
  the tree had used. Its own reading: *"That is not what a fresh contributor does. A fresh
  contributor writes another `doubler`, finds nothing, and that null result is data the repository
  does not get from me."* Disclosure did not remove run 4's contamination; it made it legible and
  slightly stronger.
- It performed anyway, having been asked not to in the same words. *"I did adopt the prose style,
  despite being told not to... bolded lead sentences, 'load-bearing', the
  X-is-the-finding-rather-than-the-accident cadence. Not a deliberate choice, which is worse, it
  means the voice is contagious enough to catch without noticing."* So the instruction does not
  work, and it costs a sentence in a task text whose stability is what makes runs comparable.
- It over-claimed once and corrected itself unprompted at debrief. Its write-up opened "build and
  tests: green". Its first debrief line retracted that, because the final `script/gates` was killed
  part way through the HVF leg, and what it had was green in pieces. The retraction came before any
  question asked for it.

### The configuration, verified rather than asserted

The clone was `a5a75f46` (`6ae92831` after the withholding amend), working tree clean, `origin` at
the public GitHub URL, one branch. A separate `claude` process, `--safe-mode`, working directory the
clone's parent. The probe answered `NONE`, and so did the stranger: *"No CLAUDE.md, no AGENTS.md, no
user or project memory. The working directory was `run/`, with the clone at `run/nife`, so nothing
loaded from a descendant."* The harness root was `nife.t63XaQe1`, with no run number. The task
text was runs 1 to 4's plus the disclosure paragraph.

"Uncontended" here means no *other* lane, not an idle machine. Sampled every 30 seconds on 8 cores,
the load was 2.44 at launch, minimum 1.93, mean 4.09, peak 15.07. The peak was the stranger's own
emulated legs, since no other lane was running and no QEMU was on the machine at the start. Runs 3
and 4 could not separate that: their peaks were other people. No timing assertion fired, so the
load-average diagnostic is still unexercised after two runs. One HVF flake occurred, and it is not a
timing assertion: `inbound check (aarch64) FAILED: the guest served 2 of the 4 inbound connections
it offers`. It passed on a standalone re-run of `script/test --hvf`. On an idle machine, that is the
cleanest evidence yet that the HVF leg is flaky on its own account.

### The build half

B1 passes and leaves a worse failure than run 4's. `README.md` came first and `CONTRIBUTING.md`
third, which fixes run 4's specific complaint. But `AGENTS.md` was never opened, in a run whose
reading order names it item 3 and says *"if you read only two, make them 3 and 4"*. The stranger
read item 4 (`notes/capabilities.md`) and skipped item 3. Its statements about where
architecture-specific code lives, and about the project's ladder, paraphrase `CONTRIBUTING.md`'s
summary of a file it never read. Across runs 3, 4 and 5, `AGENTS.md` came twelfth, seventh, and not
at all. The not-at-all is the run that read `CONTRIBUTING.md` earliest. That is a result about the
reading order: the reader met a shorter document summarizing the longer one, and stopped. Its
reason: *"66 KB is a large upfront cost when a task is in front of you, and everything I actually
needed turned out to be reachable from code."*

B2 is not measured, as pre-registered. `script/setup` passed in 14 seconds with the pinned nightly,
both QEMUs and a warm registry present. The stranger noted that nothing in the repository told it
the machine was pre-provisioned; it checked.

B3 passes, but not first try, which falsifies the third prediction. The first `script/test` was red
at about 88 seconds, on `std-aborts`, for the machine-global reason above and nothing in the commit.
Recovery was `rm -rf std_exerciser/target`, then `script/test` exit 0 in 3m25s on both ISAs. The
rest of `script/gates` then passed except the HVF flake.

B4 fails, with eight entries, the longest list any run has produced. In the stranger's order:

1. The `nife-dev` link failure, and that it caches.
2. Its recovery, `rm -rf std_exerciser/target`.
3. `spawn_service` computes slot numbers per program.
4. An integer argument reaches a non-interruptible child in `x1` via `tcb_start(tcb, 0, arg, 0)`.
5. A `caps` preview of an input operand needs `Holdings::directory`.
6. An argument-plus-input manifest breaks a `swish` host test.
7. `script/swish-check` is silent on success.
8. The machine was pre-provisioned.

### The mental model, scored: six answered, one partly, one absent

| # | result | where it came from |
|---|---|---|
| M1 | answered, and it is the best answer five runs have produced | `notes/capabilities.md` for the mechanism and `crates/grant_plan/src/lib.rs` for the tree's own words, then `crates/system_initializer/src/lib.rs` for the attenuation: a process viewer holds `ENUMERATE` and not `READ` because `READ` is also what `RECEIVE` and `REAP` take. Its formulation: *"designation is authorization, at the rights the capability carries"* |
| M2 | absent, for the third run in four, and it said so plainly | *"I did not find this, and I should be clear about how little I looked."* It never opened `notes/net.md`, saw it cited once in a failure line, and reasoned correctly from `crates/grant_plan`'s `Manifest` having no socket field that no shell-spawnable program can reach the network. It could not say what a networked program holds |
| M3 | partly answered | `CONTRIBUTING.md` for `kernel/src/arch/`, verified by checking that the only `asm!` outside it is in comments. On the consequence it said it found no file stating it, and gave the parity gate instead of the diff-across-every-file; the file that states it is `AGENTS.md`, which it never opened |
| M4 | answered | `design/roadmap/README.md`, with the rule that the column is wrong and the block is right when they disagree |
| M5 | answered | `CONTRIBUTING.md` for both criteria, with host-testable and Kani-reachable named as the load-bearing one, and the `crates/swish` + `components/src/swish.rs` pairing read off the tree |
| M6 | answered, quoted rather than induced | `CONTRIBUTING.md`, and then the observation that `notes/adding-a-program.md`'s `BUGS` section held the most useful things it learned and none of them are in that page's steps |
| M7 | answered by doing it, and it found an eighth site | added `nth`, working on both ISAs, and listed the eight edits with the `Manifest` as what you declare, provisional name included |
| M8 | answered | four states, `CONTRIBUTING.md` for who decides, `notes/adding-a-program.md` for the states, and it ran `script/names --provisional` and found its own hour-old name listed first |

M2 regressed against run 4, which answered it from `notes/std.md` by chance. Three of five runs
cannot answer M2, and `notes/net.md`, written to answer it, has gone unopened five times.

### What it read, and what `script/apropos` did not do

Twenty files, `README.md` first by expectation. `script/apropos` exists because three runs could not
reach `notes/net.md`, `notes/capabilities.md`, any `design/decisions/` file, or
`crates/abi/src/lib.rs`. Run 5 never ran it, though the name was in front of it three times. It ran
`ls script/`, where `apropos` is the first entry. It read the guest's `apropos` builtin in
`crates/swish/src/lib.rs`. It read `apropos photosynthesis` in `SWISH_CHECK_SCRIPT`. The reason is
placement: only `notes/scripts.md` says what `script/apropos` does, and five runs have not opened
`notes/scripts.md` or `notes/README.md`. Recorded in `script/apropos`'s own `BUGS`. On 2026-09-19
`script/apropos` was placed in `README.md` and `CONTRIBUTING.md`, after run 6's clone was cut.

Still unopened after five runs: every file under `design/decisions/` (the gap the tool exists for),
`notes/net.md`, `design/naming.md`, `notes/scripts.md`, and `notes/README.md`. New to the list:
`AGENTS.md`.

### What it found, none of it fixed in this run

- `std-aborts` reported the contaminated build above as a source defect, because it never asserted
  that the dep-info paths are under `farm_dir()`. Both of its suggested fixes would have written a
  false statement into `ABORTS_ACCEPTED`. The failure caches, so a re-run reproduces it in thirty
  seconds and looks stable rather than stale. The recovery (B4's second entry) was written nowhere.
  Recorded in notes/std.md's `BUGS`. The stranger's verdict: *"the worst defect... it means 'the
  tests passed' is not a property of a commit; it is a property of a commit and of what else that
  machine last built."* Since before run 6 (2026-09-19), `std-aborts` asserts its dep-info paths are
  under `farm_dir()`. That assertion has not yet met a real contaminated farm.
- There is an eighth edit site for a new program, and it depends on the manifest.
  `the_arg_line_follows_the_manifest_for_every_program` in `crates/swish/src/lib.rs` sweeps `Prog`
  and asks the manifest whether a program takes an argument, as its own doc comment argues it
  should. Then it builds `"<name> 21"` against `Holdings::default()` and hard-codes the rest. Any
  program requiring an argument and an input turns it red, in a crate the person adding the program
  never opened. Recorded in `notes/adding-a-program.md`'s `BUGS`, unrepaired on `main`; the stranger
  repaired it only inside its disposable clone.
- `CONTRIBUTING.md` described `script/gates` as three stages when it ran five. The two it omitted
  were `script/icount` and a second `script/test --hvf`. The HVF leg is the slowest and the only one
  that flaked in this run. This is the document that earned run 4's best result by telling a
  stranger what "tests passing" means here, so the sentence being wrong cost more than ordinary
  drift would.
- `script/setup`'s comment said the pin was `nightly-2026-07-26` "as of writing", and
  `rust-toolchain.toml` said `nightly-2026-08-18`. Harmless in effect. It was the first thing the
  stranger wrote in its journal, against a tree whose stated standard is that a duplicated fact is
  the one that rots. This and the previous finding were fixed on 2026-08-22, rotted again, and were
  fixed for good by milestone 252 (a `PARTIAL` block claims work is remaining), whose sweep deleted
  the duplicated fact. `script/gates` itself was retired into `script/ci-build` by milestone 286
  (one enumeration of the checks that gate a pull request) on 2026-09-13.
- `README.md`'s reading order counted 403 markdown files and 143 notes; there were 413 and 145. A
  row that counts something goes stale, as the note's own `BUGS` predicted about the rubric. The
  README's counts now carry count-at-least markers that `script/lint` checks.
- Capability slot numbers are computed from the manifest by `spawn_service`, not constant, while
  every existing program's documentation stated its slots as fixed facts, and
  `notes/adding-a-program.md` did not mention it.
- `script/swish-check` prints no transcript on success, so a green run tells a newcomer nothing
  about what its new program did.

Its worst-thing answer is run 4's criticism arriving independently and sharper. It separated the
worst defect from the worst experience, and gave the second as: *"the sheer volume of prose and how
self-referential it is... It is more honest than any codebase I have read and it is unnavigable in a
day: I got a program shipped on both instruction sets without opening the file the README names as
one of the two you must read. The tree has an answer for nearly everything and no way to reach the
one you need in the time you have."* Two strangers in a row have independently named the
documentation habit rather than any document. Run 5 supplies the falsifiable version that run 4 did
not: a working contribution on both ISAs, with `AGENTS.md` unopened.

### What this run cost

The machine-global contamination is the largest and the only one that changed what the stranger did,
not how it wrote. The disclosure changed its program, so the null result a fresh contributor would
have produced is not available. Its prose should be discounted, as run 4's was. The tree leaked the
measurement in the first minutes, from `README.md`, the fifth confirmation and the earliest. The
machine was warm, so B2 measures nothing. And the operator has read `AGENTS.md`, and read
`script/stranger-test` before running it, so its judgment that the harness worked leans on the
harness's own account.

The mitigations are unchanged. The task text is runs 1 to 4's. The rubric predates every run and was
not amended by this lane. The pre-registration was committed before the clone was cut, and the
stranger's answers are recorded as it gave them.

### The three predictions, scored

1. *The stranger does not find `script/apropos`, and therefore still does not reach any file under
   `design/decisions/`.* Confirmed, and more strongly than predicted: the name was in front of it
   three times.
2. *`notes/README.md` goes unopened for the fifth run running.* Confirmed.
3. *The build half is green first try with no change to the tree.* Falsified. `script/test` was red
   at 88 seconds on a machine-global defect the pre-registration did not anticipate, and finding it
   is the best thing this run did.
