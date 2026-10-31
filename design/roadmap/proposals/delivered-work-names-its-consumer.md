---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# Delivered work names what drives it

Raised 2026-10-31 (UTC) by the lane `proposal/delivered-work-names-its-consumer`, at the
architect's request that day, and narrowed the same day by the architect's ruling. It mints no
number, edits no decision and turns nothing on in lint.

Reuse: the measurement reads `Cargo.toml` files with Python's `tomllib` and the base image's own
list. This lane considered `cargo metadata` and did not take it: it resolves the whole dependency
graph, vendored crates included, to answer a question about path edges alone. `cargo-udeps` and
`cargo-machete` find dependencies a crate declares and never calls, which is the opposite
direction. Linux's `TRIM_UNUSED_KSYMS` drops unused exports from a build and reports nothing, so it
has no part to lend here.

## What is in force

Ruled 2026-10-31 (UTC): a question, a report and a trigger. No new field and no lint check.

1. **A question at promotion and in review.** What drives this work now, and what does it save?
   A lane brief carries the question, and so does the review of the lane's pull request. The answer
   is a sentence in the brief or the pull request body. "Nothing drives it yet" is an allowed
   answer and is a reason to hold the work, not to refuse it. "Where the question lives" below says where it goes.
2. **A report-only census.** `helpers/consumer_census.py` counts what the tree delivered that
   nothing yet uses. It exits 0 whatever it finds and sits in no gate. The maintainer runs
   it by hand on the first Monday of each month and reads the lists. No report-only slot exists
   for it today (the scheduled audits in `script/audits` cover other lenses, and
   `script/metrics` publishes a fixed set of columns), so this proposal wires none. The maintainer reads a finding as a
   question to the milestone's owner, never as a failure.
3. **Revisit on evidence.** If a case appears where a lane abandoned built work, or gave it the wrong
   shape, because it had no caller, that case is the argument for a `consumer` field on new
   blocks. Until one appears, there is no field.

## Why this much and no more

- **Most delivered work has a user.** The survey of 2026-10-31 found 46 of 57 base-image programs
  reached from code outside the tests and 91 `BUILT` blocks listed as a dependency of another
  block. It found no orphan crate. The one finding, `allocation_cache`, reaches `std` through
  generated source. The tree does not have a delivered-work problem that a new rule would fix.
- **A count of unused things cannot tell "not yet consumed" from "built with no reason".** A
  program waiting for its users is not waste. `pmap` is the example the census cannot decide: no
  code starts it, the shell catalog does not list it, and `crates/pmap`'s `BUGS` section says
  nothing at the prompt grants it an address space. That reads as unused, and it may be a tool
  built ahead of the grant that will make it useful. Only a person who knows the plan can say
  which.
- **The one real case came from the question.** Milestones 695 and 697 carry item 6 of
  milestone 607 (nife is built and updated from packages), which is the trust handoff. A person asked what
  uses the handoff and what it saves, which is under 1 MB per update in release builds, and
  that stopped boot-path code before it landed. No field was needed.

## Where the question lives

The question belongs to the developer-lane skill, `notes/skills/developer-lane/SKILL.md`, which a
lane brief names and which the maintainer reads when briefing and gating. A lane may edit that
file, and this pull request adds a short section there: "What drives the work". It holds the
question, the allowed answers and the rule that a lane brief and a pull request body each carry the
answer. `AGENTS.md` is not edited here, because a lane may not edit it. An architect who wants the question in `AGENTS.md` as well can add this beside its skill links:

> Before a lane starts, and again at review of its pull request, ask what drives the work now
> and what it saves. The developer-lane skill holds the question.

## Measurement

Measured on 2026-10-31 (UTC) at `d65f34578` with `python3 helpers/consumer_census.py --list`.

| what | count |
|---|---|
| roadmap blocks | 957 |
| `BUILT` | 493 |
| `BUILT`, named in another block's `milestone_dependencies` | 91 (47 of them by a `BUILT` block) |
| `BUILT`, any looser candidate (that, a prose citation from another block, or a `Milestone N.` follow-on) | 441 |
| `BUILT`, no candidate at all | 52 |
| `NOT-STARTED`, `PARTIAL` or `IN-PROGRESS` | 386 (50 named in another block's dependencies) |
| packages outside `vendor/` and `patches/` | 135 (124 with a library, 18 with a program) |
| library-only crates nothing depends on | 1, `allocation_cache`, a false positive |
| library-only crates only tests depend on | 0 |
| base-image programs | 57 |
| reached from code outside the tests | 46 |
| reached only from tests | 10 |
| named by nothing | 1, `pmap` |

What the numbers say:

- The crate side is clean. The one finding, `allocation_cache`, reaches `std` through generated
  source (`cargo xtask std-src`), which no `Cargo.toml` names.
- On the program side, six of the ten test-only programs are services no boot starts:
  `root_supervisor`, `spawner`, `sub_server_supervisor`, `swapper`, `fs_file_caretaker` and
  `network_time_client`. The system tests boot each, so a test proves each, and no image runs any of
  them. The other four (`slabtop`, `vmstat`, `disk_partitioner`, `disk_surveyor`) are commands a
  person types, and their consumer is that person.
- The 52 `BUILT` blocks with no candidate are mostly process work: lint gates, file splits and
  workflow fixes, whose reader is a gate or a person.

The census reads a program as reached when code outside the tests spells its name. A program
started by a name built at run time would read as unused; none is, today. A block's candidate
consumer from prose is a candidate and not a consumer: a block that names `script/lint` may have
built the check or merely run it, and only a reader can tell.

## Prior art

Four systems, each checked against its own text on 2026-10-31 (UTC):

- Linux. A maintainer refuses an export with no in-tree user. Greg Kroah-Hartman stated the rule on
  the kernel list on 2025-07-11, refusing an export for out-of-tree tracing code
  ([lkml.iu.edu](https://lkml.iu.edu/hypermail/linux/kernel/2507.1/07107.html)). The
  `TRIM_UNUSED_KSYMS` build option drops exports nothing in the configured build uses
  ([its help text](https://cateee.net/lkddb/web-lkddb/TRIM_UNUSED_KSYMS.html)). Verified. A 2008
  patch to schedule unused exports for removal drew a refusal, so removal stays a maintainer's call
  rather than a schedule; that part comes from a search summary, and this lane did not verify it.
- Rust's standard library. Every unstable API needs a tracking issue before its pull request merges.
  Stabilization asks for an experience report with "concrete usecases of users", tested for their
  needs, and a final comment period on that issue
  ([feature lifecycle](https://std-dev-guide.rust-lang.org/development/feature-lifecycle.html),
  [stabilization](https://std-dev-guide.rust-lang.org/development/stabilization.html)). Verified.
- Kubernetes. The enhancement template's graduation criteria ask, for general availability, for
  "N examples of real-world usage" and "N installs", with the numbers left to each proposal
  ([KEP template](https://github.com/kubernetes/enhancements/blob/master/keps/NNNN-kep-template/README.md)).
  Verified.
- LLVM. A new target needs a maintainer and an active community, and the policy uses those rules to
  remove unmaintained targets. It prefers incremental development, each change standing alone or
  part of a planned series ([developer policy](https://llvm.org/docs/DeveloperPolicy.html)).
  Verified. It sets no requirement that new code have a user.

Where nife differs. Linux and Rust each gate one interface kind, exports and library APIs, and
the gate is a maintainer's judgment at review. Here the reviewer asks the same question for every kind of work, and no script judges the answer. Kubernetes asks for usage at graduation
and leaves the evidence to prose, as this does.

## Alternatives

A `consumer` field on new blocks, with a `first_use` field that `BUILT` would require and a lint
check on both, came up, and the architect did not adopt it. It would have bound only blocks
raised after the ruling, since back-filling about 740 existing blocks costs hand reads that steer
nothing. It would have added two keys to the schema, and a lane with nothing to say would have
filled each with `none (<reason>)`. The survey shows no abandoned or misshapen work for it to
prevent, and the one case that mattered, milestones 695 and 697, came from asking. Item 3 above
says what evidence would reopen it. A weekly audit as a rule did not pass either: the counts above
move too slowly to need one.

## Milestones 695 and 697

Both blocks carry item 6 of milestone 607. The architect holds their code as a plain hold: no
boot-path code until milestone 976 part 4 measures a kernel download. Milestone 976 (an OS update
is a set of packages that lands through a slot) is the updater, and its step 3 reads the new
manifest through the loader. The hold stands on that measurement. It is not a rule of this
proposal, and the blocks need no field to state it.

## Index row

Nothing in `BUILT` asks whether anything uses a milestone's result. This proposal puts one
question, what drives the work now and what it saves, into the lane brief and the review, and runs
a report-only census of unused delivered work monthly. A field on new blocks is not adopted unless
a case of work abandoned for want of a caller appears.
