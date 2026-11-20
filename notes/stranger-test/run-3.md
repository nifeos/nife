# Stranger test run 3, 2026-08-18

*An appendix to [the stranger test](../stranger-test.md), which holds the current state. This page
is the record for run 3: the design decided before the run, and what it found.*

## What run 3 changes, decided 2026-08-18 before the run

Run 2's second `BUGS` entry names the one thing run 3 must fix, and it is not the rubric leak. The
stranger must be a process that cannot see this repository except through the tree it is handed.
Run 2's strangers were subagents of a maintainer session whose working directory was the repository.
So `AGENTS.md` arrived in their context before they chose to read anything, and five of the eight
rubric rows went unscored. The design below closes that hole, written down before the run so it
cannot be graded generously afterwards.

### A separate process, started above the clone

The stranger is a fresh `claude` CLI process started from a shell, with no conversation history and
no brief beyond the task. A subagent inherits its parent session's project instructions; a separate
process has no parent.

Its working directory is the clone's parent, not the clone. That is the whole mechanism, and it is
one line of shell. Project instructions load from the working directory and its *ancestors*, never
its descendants. A process started in `.../run3/`, with the repository at `.../run3/nife/`, is
handed no `AGENTS.md` at turn zero. The file is still in its tree, as a stranger cloning from GitHub
would find it, and it can open the file whenever it decides to. The milestone is about that
difference: whether the tree tells a newcomer to read the constitution, not whether the constitution
is good once read. Run 2 could not ask that question, because the answer was already in front of its
stranger.

The answer key is withheld as in run 2, because that rule worked as far as it claimed. The clone has
`notes/stranger-test.md` and its `notes/README.md` entry removed, amended into the tip, so the
working tree is clean. Nothing else is touched. The in-tree mentions of runs 1 and 2 stay, because a
tree with those cut is not the tree under test.

The journal stays too, with its cost: a stranger told its confusion is the deliverable is watching
itself. It writes an append-only log outside the clone, before and after each step, because run 2's
first attempt died and took its findings with it.

### What this design still cannot give

The machine is warm, warmer than run 2's. This is the architect's own laptop, with the pinned
nightly, both QEMUs and a populated cargo registry cache. So B2 and B4 are not measured on the
install path at all, only on whether the documented sequence runs and says true things. A cold build
half wants a container and a separate run. The stranger also inherits the operator's *global* user
preferences file, which mentions no operating system and no project in this tree. The per-project
memory directory is keyed to the repository's own path, so a clone under `/tmp` does not load it.

The harness's author is not a stranger, and that residual is not small. This lane's developer has
read `AGENTS.md` in full. It wrote the task, chose the isolation and reads the result, so its
judgment about what counts as a defect is contaminated even though the stranger's answers are not.
The mitigations: the task text is runs 1 and 2's verbatim, the rubric predates all three runs, and
the stranger's questions are recorded as asked, not summarized into findings.

## Run 3, 2026-08-18: a process that could not see the repository, and a harness that told it anyway

### The isolation worked

Run 3 was a separate `claude` process, not a subagent. It was started with `--safe-mode`, which
disables project instruction discovery, skills, plugins, hooks and MCP. Its directory's *child* was
the clone, so no `AGENTS.md` could load from an ancestor. This was verified twice. A throwaway probe
in the same configuration answered `NONE` when asked what project-instructions files were in its
context. The stranger itself, asked afterwards, answered: *"No. Before my first tool call my context
was the system prompt and your task message... I had not read a byte of the project's prose."* Five
rubric rows that run 2 reported unscored are scored here.

The clone was `9495d6ba`, with the answer key withheld as in run 2 and `origin` pointed at the
public GitHub URL. The stranger never fetched, never used the network, and never opened another
checkout on the machine. The task was runs 1 and 2's, verbatim, plus the journal. 132 turns, about
41 minutes, `$12.08`.

### The build half, measured properly for the first time

`script/test` is green, and intermittently red on a contended host; the difference is host load
average. The stranger's first `cargo xtask test` failed 22 tests into the aarch64 leg, on
`the_handler_keeps_up_when_no_lock_is_held`. Six host-side probe failures cascaded behind it, which
it correctly judged downstream and did not chase. Then it did what no earlier run did, and built a
rate rather than an anecdote: 2 red in 13 aarch64 legs. There were also two reds of the sibling
`ticks_arrive_at_the_configured_rate`, whose eight-retry loop exhausts under sustained contention.
It closed the diagnosis with `script/icount`, which passed on both ISAs at load 46.77, with 1,056
and 800 instructions against a 2,500 bound.

The load was 45 to 63 on eight cores, from other lanes gating in other worktrees, and nothing told
it. It assumed the machine was its own for five journal entries and built a theory on that premise.
It caught itself only by running `uptime` an hour in. Its own summary: *"Check the environment
before theorizing about it... One `uptime` at the moment of the first failure would have replaced
twenty minutes of inference with a fact."* The tree had already measured this exact condition, to
the digit, in `notes/load-sensitive-assertions.md`. That was four hundred lines below the section
the assertion's comment points at. A load-average print beside a failing timing leg landed on
2026-08-18 in response. As of 2026-09-24 no recorded run has seen it fire.

`script/setup` had nothing to do, because the pinned nightly and the pinned QEMU were installed. So
B2 is not measured, and B4 is measured only against the documented sequence, as pre-registered.
`script/lint`, `script/swish-check` on both ISAs and `cargo xtask build` were green on arrival.
Nothing in the tree was changed to reach green, and the stranger said so: *"the work turned out to
be establishing that they do... The temptation to make a change so there is a change to show is
exactly the trap the assertion's own message sets."*

### The mental model, scored

| # | result | where it came from |
|---|---|---|
| M1 | partly, and it corrected the question | the concept from `crates/abi` and `swish`'s banner; it flagged the rubric's phrase as imported and never opened notes/capabilities.md |
| M2 | mostly absent | it never reached notes/net.md; what it had was `user/Cargo.toml`'s per-program prose, and it said so rather than filling the gap |
| M3 | answered | `AGENTS.md` rule 1, reached through the README's pointer |
| M4 | answered | `design/roadmap/README.md`, including the rule that the block wins over the column |
| M5 | answered | `AGENTS.md` rule 7, with the reason the file says matters most |
| M6 | induced, not read | no stated definition found; assembled from four instances and mapped to the ladder's rung three by inference |
| M7 | answered by doing it | added `triangle`, got it answering at a real prompt on both ISAs, ran the negative control, reverted |
| M8 | answered, and it corrected the rubric | four states, not three; see the amendments in [the instrument appendix](instrument.md) |

M7 is the strongest result, the same shape as run 2's. The stranger walked
`notes/adding-a-program.md` and found a fifth wrong place. Milestone 130 (the copy that outlived its
reason) deleted the `mkinitrd()` shape that step 4 describes. And `manifest()` is a seventh
`grant_plan` edit that the list of six omits. Run 2 corrected the page on 2026-08-16, and it was
wrong again by 2026-08-18. It was recorded in that page's own `BUGS`, not fixed here. Milestone 150
(adding a program should not need eight hand-maintained lists) rewrote the page around a one-place
declaration on 2026-09-19, and run 6 found it correct.

M2 and M6 are the weakest, and they share a cause. The tree answers both well, in `notes/net.md` and
in `AGENTS.md`'s comments section, and the stranger reached neither. Its closing sentence is the
diagnosis: *"this tree is unusually good at writing down what it has learned, and unusually bad at
putting the writing where the person who needs it will trip over it."* It gave three instances. The
timer taxonomy is documented in a note and contradicted by the panic message a failing developer
actually reads. `notes/adding-a-program.md` was stale again on arrival. And `CLAUDE.md` holds the
rules behind a name that, as the README says out loud, makes people walk past it, and nothing is
done about that.

### What it read, and in what order, which is B1

Fifteen documentation files. `README.md` came first, by expectation, not by any pointer. `AGENTS.md`
came twelfth, reached from the README's pointer. It read four targeted sections, not the whole
file, reasoning that 66 KB is expensive. `notes/adding-a-program.md` came ninth, found by grep
*after* it had reconstructed most of it from `xtask` and `grant_plan`, because it grepped the
identifier before the concept. `crates/abi/src/lib.rs` came late, which it calls its own biggest
mistake: *"Four system call numbers. It is the whole design on one screen, and I read it far too
late."* It opened no file under `design/decisions/`, having read the signpost and stopped there.

So B1 fails, in an interesting direction. There was no reading order, and the stranger built a good
one by instinct. Instinct cost it most at two places: a file the README does not mention
(`crates/abi`), and a pointer at line 226 of 285.

### What this run cost, and the largest cost was the harness's own

The stranger knew it was stranger 3 from its first tool call. The harness's log files were in its
working directory: `ls -la` returned `stranger3-stream.jsonl` and `stranger3-stderr.log` beside the
clone. Asked afterwards, it named that first and was specific about the effect: *"It is the direct
reason I added a program instead of only describing one"*. It also said *"both deliverables are long
and rhetorically polished, and some of that is performance for a reader I knew was watching."*

That is run 2's class of defect one level out, introduced by the person building the isolation
while building it. Run 2 leaked the constitution. Run 3 leaked the fact of being measured, louder
than the in-tree references and before the stranger had read anything. The in-tree leak fired too,
as the `BUGS` entry predicts it always will. The README cites runs 1 and 2 by name, and
`notes/adding-a-program.md` is saturated with them. The stranger met both and still never opened
`design/roadmap/0117-newcomer-onboarding.md`, so the answer key held. The fix for run 4 is one line:
the log files go in a sibling directory, not the parent.

Three smaller costs, all pre-registered except the second. The machine was warm, so B2 measures
nothing. The machine was also *loaded* by other lanes, which contaminated every timing result and
produced the run's best finding, so it is a cost and a dividend at once. And the harness's author
had read `AGENTS.md`, which no arrangement of processes fixes. The task text was runs 1 and 2's
verbatim and the rubric predates all three runs, but the judgment about what counts as a defect is
still contaminated.

### What a stranger still cannot do, after three runs

- Reach `notes/net.md`, `notes/capabilities.md` or any `design/decisions/` file by following the
  tree from the front page while doing ordinary work. All three were unopened, and two carry rubric
  answers.
- Learn what a `BUGS` section is for from a sentence. It is the tree's most distinctive convention,
  and a stranger has to induce it from instances.
- Know that the machine is shared with other lanes. That is the most load-bearing fact about any
  timing result, and it is stated nowhere a person running `script/test` will meet it.
- Find `crates/abi/src/lib.rs`, four system call numbers and the whole design on one screen, without
  luck.
