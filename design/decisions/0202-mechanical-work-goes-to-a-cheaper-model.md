---
status: AMENDED
raised: 2026-09-20
decided: 2026-09-20
ratified_by: calef
---

# 202. Mechanical work goes to a cheaper model, and the gates are why that is safe

*Amended 2026-10-02 (UTC): the routing rule is four Claude tiers and a new criterion, below. No
price is recorded, by ruling.*

calef, 2026-09-20: *"You have permission to route mechanical work to cheaper
models and should capture that somewhere durable. To start, Sonnet is already available under our
Claude subscription."*

## Amended 2026-10-02: route by whether anyone specified the valuable output

calef, 2026-10-02 (UTC), ruling on an unfinished draft of this amendment dated 2026-09-22: land the
routing rule and leave out every price, price table, ratio and per-model cost. *"Prices change daily
so they'll be stale in the tree."* So this records which tier gets which work and why, names tiers
rather than model versions for the same reason, and leaves cost to whoever routes: relative cost,
read from the provider's current list when routing.

calef, 2026-09-22: "Sonnet and Opus aren't comparable." The original two-way split (a cheap
model and an expensive one) hid the boundary that mattered most, which was between those two.

### The criterion, which is not difficulty

The ruling below asks whether work is *mechanical*. The sharper question is whether the valuable
output is something nobody specified, and that is what the evidence of 2026-09-21 separates.

Opus found that `size_of::<PerCpu>()` growing from 128 to 136 broke shift-based indexing and cost
5.4% of the IPC fastpath. It found that a capability parked in a thread's hand-off slot survived
every revocation sweep, and that an x86 calibration was wrong by a factor of eleven. None of those
was asked for. Sonnet did the promotion repair and the falsification refresh: complete
specifications, executed correctly, and it caught an error in the maintainer's own brief.

| tier | when |
|---|---|
| Fable | unproven here. No lane has been dispatched to one, and there is no evidence it earns its higher relative cost over Opus. Recorded as unknown rather than reserved for something |
| Opus | the answer is not in the brief: design forks, adversarial passes, anything where *what did you notice* is the deliverable |
| Sonnet | the specification is complete and the difficulty is in the execution |
| Haiku | the specification is complete, the execution is routine, and project conventions matter, because it reads `AGENTS.md` and the brief can therefore be short |
| rented | a gate is the entire standard, and the brief must carry every rule, because `--bare` discards the constitution |

The table in "The ruling" stands as examples of the Sonnet and Haiku rows. Its "Stays" column is
the Opus row, now stated by criterion rather than by list.

The brief is part of the cost. Haiku reads the tree's conventions, so its brief is short; a
rented model does not, so its brief is long. Four briefs written on 2026-09-22 each re-carried the
citation-gloss rule, the never-edit-another-block rule and the provisional-name rule, because a
rented model cannot read them. Weigh that against the token price when routing.

The boundary to watch is Opus against Sonnet, not Claude against rented. Most lanes on
2026-09-21 had complete specifications, written by the maintainer, and went to Opus by habit. Under
this criterion they were Sonnet's.

### What this does not say

It is not a ceiling on spending. calef, 2026-09-22: *"we can use more of any of the Claude
models. We just have to pay for them."* The goal is work per unit of budget, not a smaller bill:
a task a cheaper tier does as well as Opus buys more work for the same money, so reaching for the
expensive tier is not caution. It is waste with a good excuse.

## The constraint this answers

calef's inference budget is $200 a month and the capacity runs out weekly. The limit reached is a
rate limit rather than a bill, so the lever is routing rather than spending.

Measured on 2026-09-20, five lanes reported 2.0 million tokens between them (a crypto provider at
641k, a screen handshake at 486k, floating-point state at 415k, a refusal backfill at 271k, a naming
sweep at 184k) across 1,454 tool calls. The judgment in those lanes was a handful of choices each:
eager against lazy FP save, a command-line knob against a kernel feature, which refusals earn
numbers. Everything else was execution against gates.

## The ruling

**Mechanical, gate-covered work is briefed to a cheaper model. Judgment stays where it is.**

| Goes to the cheaper model | Stays |
|---|---|
| Mutation-survivor triage, test writing against a stated property | Design forks, and anything `AGENTS.md` calls an architect's |
| Record sweeps, glosses, index and citation repair | Naming, which is an architect's under milestone 115 (the names that were ratified) |
| Parity ports where the shape is already set by another architecture | Merge conflict resolution and the merge queue |
| Backlog work a script can verify: counts, censuses, triage to a ledger | Anything touching the syscall surface or a wire format |
| Benchmark harnesses and their plumbing | A first implementation whose shape nobody has chosen yet |

Sonnet first, because it costs nothing new. It is available under the existing subscription, so
the first trial risks no money and no new vendor. Cheaper hosted models are a later question, and
this section does not decide it.

## Why this is safer here than it would be in most codebases

**This tree has an unusually strong machine-checked floor**, and that is the whole argument.
`script/lint`, `script/test` on three architectures, `script/citations --ratchet`, `script/roadmap
--check`, `script/names`, `script/verify`, the mutation gate, the fastpath footprint and the icount
tripwire all fail loudly at a lane that gets something wrong. A weaker model's mistake is caught by
a gate rather than by a reader, which is the condition under which cheap inference pays. Most
projects do not have it; this one was built that way for a different reason and now collects a second
dividend.

## What this does not claim, and how it will be judged

**The cost of a weaker model is not tokens, it is cleanup.** A lane that ignores "do not touch
`design/decisions/`", or writes a gloss from memory, or reports a finding that is not true, costs
more maintainer attention than it saved. That has happened with a frontier model twice today: one
lane reported a `script/names` blind spot that had been closed for a month, and another measured an
hour of benchmarks against a tree it had clobbered.

So the trial is bounded and measured rather than assumed:

- First lane: mutation triage on a single crate. Every outcome is machine-checkable (a test that
  kills the mutant, an equivalence the next run still reports, an exclusion carrying its reason), and
  milestone 326 (nobody has been assigned to turn a mutation score upward) has set the standard.
- What is recorded: what the lane produced, what the maintainer had to redo, and whether the
  gates caught what went wrong or a person did.
- What would reverse this: cleanup costing more attention than the routing saves. That is a
  judgment an architect makes on the record above, not a number a script returns.

## Trial 1, `board_console`, 2026-09-20: passed, and the prediction was wrong

The work. 43 mutation survivors in `crates/serial_console`, briefed identically to a frontier
lane (same hazards, same gates, same stopping rule), so that the comparison measures the model
rather than the brief.

The result, re-derived by the maintainer rather than relayed. `script/mutation -p
board_console` reports 324 mutants, 4 missed, 283 caught, 31 unviable, 6 timeouts, which
reproduces the lane's own report to the unit: 43 survivors to 4, 83.2% to 96.6%. Thirty-nine killed
by a test with each kill verified by re-running the sweep, one argued equivalent, three recorded as
gaps needing a real tty, six timeouts unchanged under the file's own convention.

| | trial | frontier lanes the same day |
|---|---|---|
| tokens | 369k | 184k to 641k |
| tool calls | 152 | 120 to 593 |
| wall clock | about an hour | 28 to 70 minutes |
| maintainer repair | none | -- |

The maintainer predicted where it would fail and was wrong, which is worth recording because the
prediction was written before the lane reported. The expectation was competent kills and weak ledger
prose, with equivalence arguments that assert rather than demonstrate. The one equivalence claim
does the opposite. It names the mutation (`BootProgress::reach`'s `>` becoming `>=`), the only
variant that can reach the extra branch, the invariant that makes the two rungs the same
(`every_profiles_depths_count_from_one_without_gaps`), and therefore why the reassignment is
unobservable. It also recorded that at the function, per this file's own convention, rather than
only in the ledger.

One judgment call above a triage lane's obvious remit, recorded because the next such lane will
face it. It refactored production code: `candidates()` now delegates to a new `scan(dir: &Path)` so
the directory walk can be tested against a temporary directory instead of whatever is plugged into
the machine. The maintainer allowed it: it mirrors the crate's existing `choose`/`pick` split and is
documented at both the function and the test. A stricter reading of the brief would have recorded a
gap instead, and the brief should say which reading it wants rather than leaving a lane to guess.

What this does not yet establish. One trial, on a crate chosen to be favorable: host-side, small,
uncontended, with every outcome machine-checkable. Two or three more before the routing is settled,
and **external research stays on the frontier model**, because it is the one category this tree has
no gate behind: a fabricated summary passes every check green.

## Trial 2, `video_terminal`, 2026-09-20: passed, on the tree's largest untriaged set

79 survivors to 16, 79.0% to 95.8%, five tests closing 63 of them and the remaining 16 argued
equivalent with no exclusion and no recorded gap. The maintainer re-ran the sweep: 392 mutants, 16
missed, 361 caught, 15 unviable, which reproduces the lane's report exactly. 286k tokens, 130 tool
calls, no maintainer repair.

The claim worth checking was "all sixteen are equivalent", because a lane that wants to be
finished can rationalize there and no gate would catch it. Three were read closely and they hold.
The four `CellRect::union` selectors are equivalent for a reason that is a proof rather than an
observation. A selector of the form `if a < b { a } else { b }` returns the same value on both
branches whenever `a == b`. `<` against `<=` disagrees only about which branch fires at exactly
that point, so no input can separate them rather than merely no input a test tried.

And it discriminated where it would have been easier not to. The size clamp had four survivors;
it called the two `>`-against-`>=` mutants equivalent by that same argument and the two `==` mutants
a real behavior change, because `==` clamps only the boundary value and lets everything past it
through. It then wrote a test for those two. A lane looking to declare victory would have called all
four equivalent.

One caveat against counting this as a hard test. `video_terminal` has no Kani harnesses and no
loom model, so it carried none of the "the mutant was never the crate's code" artifact that bit
earlier sweeps, and its before-numbers matched the census row for row. It is a cleaner crate to
triage than average.

## Trial 3, `machine_discovery`, 2026-09-21: the brief was wrong and the lane caught it

The maintainer picked the crate badly. It was chosen off the census record as the hardest
remaining case, a boot-path parser with 77 survivors whose callers are three kernels. It had
already been triaged, on 2026-09-19, by a lane that landed through an integration branch and so
left no obvious pull request. That triage found 58 killed, 11 equivalent, 8 recorded gaps, all
accounted for in `notes/mutation-testing/machine-discovery.md`, which the maintainer did not read
before briefing.

That is the third crate in a row whose census row did not mean what it appeared to, after
`work_steal_slot`'s 54.2% (a loom model) and `multicast_dns_protocol`'s 82 survivors (a crate
deleted the day after the census measured it). The lesson is not about models: pick a triage crate
from the ledger and recent history, not from the census.

What the lane did with a bad brief is the actual result. It did not redo the work. It identified
the earlier lane by commit, re-derived the sweep against the current tree, and checked whether two
pull requests that landed *after* that triage had introduced anything. The sweep came to 714
mutants, 612 caught, 19 missed and 9 timeouts. Every one matched an already-argued equivalent,
recorded gap or noticing-timeout by file, line and operator, and the 21 new mutants from those later
pull requests were all caught. It concluded that nothing was owed and wrote a 30-line dated
addendum rather than a milestone's worth of redundant tests.

**Refusing to manufacture work is the behavior this routing most needed to demonstrate**, and no
gate would have caught the opposite. A lane that had written 19 tests against already-argued
equivalents would have produced a green pull request full of waste.

The one genuine lapse, and it is an instruction-adherence one. The lane ended its turn to wait
for its own background mutation run, which `AGENTS.md` names as the failure mode rather than
patience. Neither frontier lane that day did this. It cost one resume message and no wrong work, and
the two earlier trials could not have surfaced it because their sweeps were short enough to run in
the foreground. That is where a cheaper model drifts: long-running jobs and the discipline around
them, which is a briefing problem before it is a model problem.

## Where the two trials leave this

The routing works for this class, and the class is now well defined: machine-checkable outcomes,
a standard already written down, and a blast radius that ends at tests and a ledger. Both lanes came
in below the median frontier lane of the same day (184k to 641k tokens) and neither needed a
correction.

**The failure that would change this is not a bad test.** It is a plausible-sounding equivalence
argument that is wrong, because that is the one output no gate in this tree can check. Both trials
cleared that bar in a way a reader could verify. A third trial is running on `machine_discovery`, a
boot-path parser whose callers are three kernels, which is the first genuinely hard case.

## The note this supersedes

A standing maintainer note says to omit the `Agent` tool's model parameter so a lane inherits the
session's model, because naming one pins it to a possibly-older alias. That remains right for
judgment lanes and is now wrong for mechanical ones, which name their model deliberately. The
distinction is the table above.

## What a brief owes when it routes down

The brief carries more of the hazard, not less. A lane that cannot infer the trap has to be told
it. The gloss must sit on the same line as its number, the ratchet reads the committed tip, a
worktree shares one stash stack, `origin/*` is not a fixed point. Those are already in the briefs
this tree writes; routing down makes them load-bearing rather than courteous.

And the lane line stays. Every pull request and comment an agent writes opens by saying it was
written by an agent, whichever model wrote it. The model is not the point; the honesty is.
