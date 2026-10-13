---
status: NOT-STARTED
raised: 2026-10-12
promoted_from: tooling-sorted-by-who-needs-it
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 902. nife's tooling, sorted by who needs it, before any of it moves

*(The title and slug are provisional.)*

calef, 2026-10-12 (UTC): *"It
seems like we have a fair amount of automation for how I work in the nife tree. Maybe we need to
step back and figure out what's mine and what another developer or architect needs and extract that
out."* He agreed to an inventory first. This is that inventory. It moves nothing. The title, the slug
and every repository name below are provisional, for calef's naming pass.

Reuse: nothing is built here. The prior art below is read for the shape of a split, and every
option that builds reuses GitHub reusable workflows, as 887 already does.

## The four classes

Every file in `script/`, `helpers/`, `briefs/`, `.github/`, `.githooks/`, `.claude/` and
`notes/skills/`, every section of `AGENTS.md`, and the notes and tenets that exist only to run the
agent workflow, put in exactly one class:

1. Build and prove the OS: tests, proofs, falsifications, fuzzing, benches, QEMU runners, coverage,
   the toolchain and the supply chain. Every contributor needs these.
2. The tree's standards: lint and its ratchets, house style, citations, and the decisions, roadmap
   and names tooling. Every contributor and any architect needs these.
3. Running a fleet of agent lanes: the claim, the merge drain, the architect label, hold and ruled
   bots, `record-ruling`, `needs-maintainer`, the smelter identity, effort, session hooks, the
   briefs and the lane skills. Needed by calef, or by anyone who works his way.
4. One machine: facts true only of calef's desk, which belong in the private lab repository.

An item that honestly sits between two is recorded as straddling both (`1/3`, `2/3`), not forced.
The per-file tables, with what calls each file, what it calls, its length and whether it names
calef, are in four appendices:
[scripts](0902-tooling-sorted-by-who-needs-it/scripts.md),
[helpers in classes 1 and 2](0902-tooling-sorted-by-who-needs-it/helpers-classes-1-and-2.md),
[helpers in class 3](0902-tooling-sorted-by-who-needs-it/helpers-class-3.md), and
[CI, hooks, instructions and notes](0902-tooling-sorted-by-who-needs-it/ci-hooks-and-instructions.md).

## What was measured

Measured 2026-10-12 against `e3cc3f128`, with `git ls-files` and line counts. Callers come from a
path search over `script/`, `helpers/`, `.github/`, `.githooks/`, `.claude/`, `.cargo/` and
`xtask/`, so a file named in a comment counts as a reader. Every edge in the next section was then
checked by hand as a real invocation.

| Class | Files | Lines | Of which |
|---|---|---|---|
| 1, build and prove | 94 | 24,353 | 44 scripts, 32 helpers, 12 workflows |
| 2, standards | 48 | 24,276 | 11 scripts (lint alone is 4,074), 27 helpers |
| 3, the fleet | 147 | 26,181 | 60 helpers, 46 notes, 9 briefs, 8 workflows, 7 scripts |
| 1/2 | 1 | 240 | `script/crate-standalone` |
| 1/3 | 9 | 2,862 | CI time and budgets, `scope-merge-base`, `cadence-check`, `ci-log-baseline` |
| 1/4 | 1 | 716 | `script/netboot-rehearsal` |
| 2/3 | 14 | 5,990 | `script/metrics`, `catch-up`, `rule-violations`, the decisions skill, the commit-msg hook, empty diff |
| 3/4 | 2 | 329 | `helpers/prune_toolchains.py`, `briefs/bench-session.md` |
| Total | 316 | 84,947 | |

Class 3 is the largest by files and by lines. Of its lines, 4,481 are the recorded GitHub responses
the `needs-maintainer` selftest replays, and 7,355 are notes.

`AGENTS.md` (267 lines) is mostly class 2. The skills index (25 lines) and the agent pull request
line (8) are class 3. Principle 2, the method as a result, straddles 2 and 3 and is Fork 3 below.
Testing and Environment (20 lines) are class 1.

Who is named. calef is named in 81 of the 117 class-3 rows, 36 of 48 class-2 files and 53 of 94
class-1 files. No other person is named. Naming calef is not a class-3 marker on its own: most
class-2 files cite his rulings as the provenance of a rule, which a stranger needs.

Class 4 in a public tree. `script/netboot-rehearsal` carries one board's hardware address and the
house network's prefix as constants. `script/board-image` defaults to the same prefix. The values
are not repeated here. `briefs/bench-session.md` talks about outlets and serial ports but already
sends every unit-specific fact to the off-tree record. `script/effort` reads agent session records
from one machine's home directory, and `helpers/prune_toolchains.py` uninstalls toolchains from
the machine it runs on. Neither carries a machine fact, but both measure or change one machine.

## The edges that make extraction hard

Class 3 calling class 1 or 2 is normal: the drain reads `architect-label-rules.py` and
`ready-status.yml` runs `script/roadmap`. The hard edges point the other way, where a class-1 or
class-2 item cannot run without a class-3 one.

1. `script/lint` runs 22 class-3 checks. Nineteen are selftests of the queue and label machinery:
   queue eligibility, cancelled duplicates, `needs-maintainer`, `ci-failing`, `session_steps`,
   `smelter-token`, both `blocked-by` checks, `claim`, the queued-push refusal, the CI-poll hook,
   the label rules, `architect-ruled`, `record-ruling`, `hold-rerun`, `watcher_watch`,
   `merge_queue_share`, `debt_paydown_share`, and 4b's own selftest. Three are rules: 4 (branch
   shape, shared with `script/claim`), 4b (a `milestone/N-*` branch touches its block), and the
   agent-instruction index. That is about 170 of lint's 4,074 lines. Five more straddle: empty
   diff, the commit-msg hook, `scope-merge-base`, `ci_job_times` and `script/metrics --selftest`.
   Moving class 3 out deletes these sections or makes lint fetch another repository.
2. `.githooks/pre-push` calls `helpers/push-queued-check.sh`, which asks GitHub whether the branch
   is in the merge queue.
3. Five class-1 workflows mint a `nife-smelter` App token to commit results or open pull requests:
   `falsifications`, `mutation`, `toolchain-bump`, `vendor-watch` and `verify-shard-refresh`. They
   depend on milestone 128 (the automation gets its own identity), which is class 3.
4. `ci.yml`'s `draft gate` skips every job while a pull request is a draft. That is §90 (the claim
   is a draft pull request) built into the build itself.
5. `script/metrics` charts `effort.csv`, `commit-models.csv`, `merge-queue-daily.csv`, the
   context-per-turn series and the debt-paydown share beside kernel series, and `metrics.yml` runs
   the class-3 collectors. `script/cadence-check` calls `script/effort --check` and
   `watcher_watch.py`.
6. `helpers/scope-merge-base.sh` serves both CI's prose-only scoping (`ci.yml`, `verify.yml`,
   `codeql.yml`) and the architect label. `helpers/ci_job_times.py` loads `job-budget.py`, and
   `needs-maintainer.jq` reads its tracking issues.
7. Documentation: `AGENTS.md` links the lane skills, and `notes/scripts.md` lists class-3 scripts
   among the front doors. These are soft and move with an edit.

Class 3 also carries 131 milestone citations and 96 § citations across its files. Any of it that
leaves fires §201 (one roadmap until a citation has to cross)'s revisit, by that decision's own
event condition.

## Prior art, read 2026-10-12

Rust, Linux, Chromium and Kubernetes, with sources, are in
[an appendix](0902-tooling-sorted-by-who-needs-it/prior-art.md).

Two patterns hold across all four. The checks that read the code live with the code, and the
machinery that moves changes through review lives elsewhere. Where the machinery needs policy, the
policy stays in the tree as data the bot reads. Rust keeps `triagebot.toml` and `rust-bors.toml` at
its root; the latter's `labels_blocking_approval` is a list of labels that hold a merge, which is
what `needs-architect` is here. Kubernetes keeps `OWNERS` files beside the code and Prow's approve
plugin reads them; Chromium keeps `OWNERS` in-tree too. And the one in-tree check of bot config
runs in that direction only: tidy's `triagebot.rs` checks that the paths `triagebot.toml` names
exist. tidy never runs triagebot.

nife breaks the first pattern in one place, edge 1: its standards gate runs the bot's own tests.

## What already overlaps

- Milestone 887 (crate repositories share one CI template, adapted from nife's gates) has already
  sorted nife's gates for a crate repository. It keeps or adapts the architect label, the architect
  hold, `record-ruling`, the commit-msg trailer and empty diff. It drops the merge drain, trunk
  health, `ci-failing`, ready status, branch names and the agent instructions. So the governance
  half of class 3 is already planned for every crate repository and is not calef's fleet alone.
  887 also keeps the generic class-2 checks: the file-length, comment-block and prose ratchets,
  house style, em-dashes, links, pinned actions, swallowed failures, orphan Rust files and shellcheck.
- Milestone 857 (crates released on their own) ruled one repository per crate and one shared
  template through reusable workflows (its Fork 1). That template is where generic class 2 would
  live if it lived anywhere shared.
- Milestone 756 (the first code to leave this repository) and
  [notes/the-first-split.md](../../notes/the-first-split.md) measured the split for code, and
  refused moving `host-tools` first because it is the most entangled. The note already says lint's
  sections "assume one tree" and that a moving repository needs a subset. This inventory is that
  subset's measurement, from the other side.

So class 2 should become a reusable toolkit only in its generic half. The nife-record half
(`script/decisions`, `roadmap`, `names`, `citations`, `fatal-risks`, `audits`, `journeys`,
`catch-up` and their readers) reads records that exist only here, and 887 drops every one of them.

## Recommended destinations

| Class | Destination |
|---|---|
| 1 | Stays in nife. Programs leave with their packages under 756 and milestone 691 (packages move out of this repository, one per pull request), as `build-speedtest1.sh` already proposes to. |
| 2, nife records | Stays in nife. |
| 2, generic checks | Into 887's template repository once 887 is built, consumed by nife at a pin like any crate repository. Until then, nothing moves. |
| 3, governance mechanism | The rules stay in each repository as data; the bots become reusable workflows in 887's template (Fork 1, ruled A). |
| 3, the rest | One new public repository (provisionally `nifeos/fleet`, name not yet ratified, not yet created), consumed by nife at a pin (Fork 2, ruled A), after the edges are cut (Fork 4, ruled A). |
| 3, method metrics | Collectors move with the fleet; series, charts and caveats stay in nife (Fork 3, ruled A). |
| 4 | The two constants go to an environment variable with no default, and their values to the lab record. |

## Fork 1, ruled A: where the architect governance sits

calef, 2026-10-12 (UTC), [relayed by the maintainer session](https://github.com/nifeos/nife/pull/1976#issuecomment-6117241317): *"A"*.
Each repository keeps its own architect rules as in-tree data, and the label, hold and
`record-ruling` bots become reusable workflows in 887's template. The options below stay as the
record of what was weighed.

Background. §88 (`needs-architect` as a check) made the hold a required status check. The machinery
is eight helpers, four workflows and `script/record-ruling`: `architect-label-rules.py` (1,496 lines) decides from a diff
whether a pull request needs calef, and the hold, ruled and rerun workflows apply the answer. 887
will run the same hold in every crate repository. The question is whether this is the tree's
standard, calef's fleet, or two things.

| Option | Prior art | Cost |
|---|---|---|
| A. Split policy from mechanism. The rule table and the label names stay in each repository as data. The workflows, `record-ruling` and `open-question` become reusable workflows in 887's template. nife's lint checks the data against the tree and stops running the bots' selftests. | `rust-bors.toml`'s blocking labels and `triagebot.toml` in rust-lang/rust; `OWNERS` with Prow and with Chromium | The rule table is code today (Python patterns), so it is split into data plus an engine: one lane |
| B. All of it stays in nife as class 2, and 887 copies it | none found | Two copies of the hold, which milestone 236 (three derivations are copied between scripts)'s rule says will drift while both look authoritative |
| C. All of it moves into the fleet repository | none found | Every crate repository's required check depends on calef's lane tooling, and a fork of nife loses its hold |

Recommendation: A. Who needs an architect, and for which paths, is a fact about the repository and
its architect, so it stays with the repository. How a label gets applied is the same everywhere.
The fleet's drain also reads `open-question.jq` and the label rules, so under A it consumes the
template at a pin like any repository. Reversible until 887 is built, since nothing yet consumes
the template. Had calef said no, B was the fallback and 887 would have grown a copy.

## Fork 2, ruled A: one new public repository or several

calef, 2026-10-12 (UTC), [relayed by the maintainer session](https://github.com/nifeos/nife/pull/1976#issuecomment-6117241317): *"A"*.
One fleet repository, provisionally `nifeos/fleet`, beside 887's template. The name waits for his
naming pass, and no repository is created yet. The options below stay as the record of what was
weighed.

Background. With Fork 1's A, class 3 minus the governance mechanism is 134 files. Every
change that crosses a repository lands upstream first and is then pinned downstream by a separate
pull request, so each additional repository adds a pin to bump.

| Option | Prior art | Cost |
|---|---|---|
| A. One fleet repository, plus 887's template, which is already planned | depot_tools: one repository of workflow tools; Kubernetes: Prow and test-infra, two | One new repository and one pin in nife |
| B. Merge the fleet tooling into 887's template | none found | Every crate repository pulls lane tooling 887 drops |
| C. One repository per tool (drain, identity, metrics, hooks) | Rust: triagebot, bors and rustc-perf are separate | Four or more pins, and the drain, identity and label tools call each other |

Recommendation: A. The class-3 tools call each other densely (the drain alone loads eleven
of them by name, two of which are governance), so splitting them makes most changes cross a
boundary. B would ship a merge queue's tooling to
repositories that 887 says have no queue. Naming the repository and making it public is the
irreversible part; the code can move back. That part waits for the naming pass.

## Fork 3, ruled A: whether method metrics stay with nife

calef, 2026-10-12 (UTC), [relayed by the maintainer session](https://github.com/nifeos/nife/pull/1976#issuecomment-6117241317): *"A"*.
The collectors move with the fleet tooling, and the series, the charts and their caveats stay in
nife.

Background. Principle 2 says the method is a result and must be recorded with its caveats. The
method series are effort, context per turn, commits and lines per model, merge-queue share,
debt-paydown share and redo rate. Their collectors are class 3 (`script/effort` alone is 1,052
lines). Their data and the page that charts them, `notes/project-metrics.md`, are in nife, beside
`notes/how-this-is-built.md`. `script/effort` reads one machine's session records, so its input is
machine-bound wherever it lives.

| Option | Prior art | Cost |
|---|---|---|
| A. Collectors move with class 3; the series, the charts and the caveats stay in nife | Neither project below keeps data in the measured tree; this keeps it on purpose, for principle 2 | `script/metrics` keeps drawing the series but reads data written by a pinned collector |
| B. All stays in nife | none | Five collectors (1,796 lines) stay, and `script/metrics` stays mixed |
| C. All moves, and nife links to it | [rustc-perf](https://github.com/rust-lang/rustc-perf), "Website for graphing performance of rustc", publishing at perf.rust-lang.org; CNCF [devstats](https://github.com/cncf/devstats), "a toolset to visualize GitHub archives" | Principle 2's claim and its caveats sit a link away from the tree that makes them |

Recommendation: A. The claim belongs in nife, since nife is what it is a claim about, and a stranger
reading `how-this-is-built.md` should meet the numbers without leaving the tree. The collectors
would measure any repository run this way. Fully reversible.

## Fork 4, ruled A: the order of extraction

calef, 2026-10-12 (UTC), [relayed by the maintainer session](https://github.com/nifeos/nife/pull/1976#issuecomment-6117241317): *"A"*.
The edges are cut in place first, in one nife lane, and then class 3 moves as a unit. That lane is
milestone 889 (the tree's gates stop depending on the fleet tooling), in
[its block](0889-the-trees-gates-stop-depending-on-the-fleet-tooling.md).

Background. Edges 1 to 6 are why a move today would be a rewrite. Every option keeps class 1 where
it is.

| Option | Cost |
|---|---|
| A. Cut the edges in place first: lint's class-3 sections become their own entry point that the fleet's CI runs; the queued-push check becomes opt-in; the five workflows take their identity as an input. Then move class 3 as one unit. Generic class 2 rides 887 when 887 is built. | One lane in nife before any new repository. |
| B. Build 887's template first, then class 3 | Class 3's edges stay until 887 lands, and 887 is `needs_person: yes` |
| C. Move class 3 first, as it is | lint, pre-push and five workflows break or vendor it back |

Recommendation: A. It is reversible, needs no new repository, and makes a stranger's lint
independent of the merge queue on day one. It is also how tidy and triagebot already relate.

## BUGS

- The class of each file is one reading, not a ruling. The ambiguous rows are the ones a
  second reader would most likely move.
- Callers are found by path search, so a file named only in a comment counts as a reader. Every
  edge above was checked as a real invocation; the table columns were not.
- `helpers/prune_build_output.py` (#1974) is not on `main` and is not counted. It is class 3.
- Notes about OS features whose names sound like workflow (`notes/state-handoff.md`,
  `notes/delegation.md`, `notes/durable-delegation.md`) were read and left out.

## Index row

An inventory of nife's tooling in four classes, sorted by who needs each file, with calef's rulings
on where each class goes, taken before any of it moves out of the tree.
