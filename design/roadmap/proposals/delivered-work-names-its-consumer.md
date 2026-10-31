---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# Delivered work names its consumer, and BUILT waits for its first use

Raised 2026-10-31 (UTC) by the lane `proposal/delivered-work-names-its-consumer`, at the
architect's request that day. This is a proposal for a rule, not a ruling. It mints no number,
edits no decision and turns nothing on in lint. The decision it asks for is unwritten until an
architect rules.

Reuse: the schema is `helpers/roadmap_block.py`'s, and the checks would sit in `script/roadmap
--check` beside `reuse_problems`, whose cut-over date is the precedent for exempting old blocks.
The measurement reads `Cargo.toml` files with Python's `tomllib` and the base image's own list.
This lane considered `cargo metadata` and did not take it: it resolves the whole dependency graph, vendored
crates included, to answer a question about path edges alone. `cargo-udeps` and `cargo-machete`
find dependencies a crate declares and never calls, which is the opposite direction. Linux's
`TRIM_UNUSED_KSYMS` drops unused exports from a build and reports nothing, so it has no part to
lend here.

## Recommendation

Adopt all four parts, staged so that nothing old fails on the day the rule turns on:

1. A new `consumer` key in every block raised on or after the ruling date, refused when empty.
2. A new `first_use` key that `BUILT` requires on those same blocks, citing a pull request, a test
   or a measured run in which the consumer called the work.
3. A review rule, not a gate: a large producer's first pull request carries its thinnest real
   caller.
4. A periodic audit, report-only, built on `helpers/consumer_census.py`, which this lane landed.

Older blocks are not back-filled by hand. The audit carries them instead, and each finding gets a
consumer or goes to deletion.

## Background

A milestone in this tree ends when its block reads `BUILT`. The status vocabulary defines that as
complete and proven by the gate on every supported architecture. Nothing in it asks whether
anything uses the result. A crate can land with no dependent, a program can ship in the base image
with nothing that starts it, and a contract can carry one side. Each still reads as delivered.

That gap has a cost beyond dead code. Work that nothing calls has met no real requirement, so its
interface is a guess. When a consumer finally arrives, it either bends to the guess or rewrites the
producer. The tree has paid this cost and recorded it. Milestone 945 found that `serial_driver`
shipped in the drivers package with nothing to start it; its last starter had gone with a tour step,
and the replacement driver beat it on every axis anyway. §311 (a contract is code, not prose) and
lint check 5's count of `#[path]` consumers are two narrower answers to the same failure.

The rule asks three things of a block and one of the tree. The block names who will use the work
before anyone builds it. It does not claim `BUILT` until that user has called it. A large producer
grows from its first real caller rather than ahead of one. And the tree, on a schedule, lists what
it delivered that nothing uses yet.

## What the tree does today

- `status: BUILT` requires a `built` date and a `## Follow-on` section. Neither names a user.
- `milestone_dependencies` names what a block waits on. Read backwards, it names a consumer, and it
  is the only machine-readable consumer edge the schema has. A `BUILT` block need not carry the
  dependency fields, so the edge exists only where a newer block depends on an older one.
- `Reuse:` binds blocks raised on or after 2026-10-05, and older blocks are exempt. The
  rule here would arrive the same way.
- Milestone 640 (a gate is not evidence until somebody has watched it fail) already demands a
  first use of one kind of producer: a gate.
- `helpers/orphan_rust.py` fails a Rust file no crate compiles. Nothing fails a crate, program or
  contract that nothing uses.

## The four parts, as proposed

### 1. Each block names its consumer

The key is `consumer`, and its value is one of four forms:

| form | example | the check resolves it against |
|---|---|---|
| `milestone N` | `consumer: milestone 976` | a block under `design/roadmap/` |
| `program <name>` | `consumer: program swish` | `packages/base-image.toml`, or a package's `programs` |
| `gate <path>` | `consumer: gate script/roadmap` | a file that exists |
| `none (<reason>)` | `consumer: none (a document split; its reader is a person)` | a reason of three words or more |

A block that carries neither a consumer nor `none` with a reason fails. A value names one consumer
or a comma-separated list of them, in the schema's flat form. `milestone N, part 4` names a part, and
§207's rule applies: a dependency on part of a milestone means that milestone wants splitting.

### 2. BUILT cites first use

A block raised on or after the ruling date may read `BUILT` only with a `first_use` key. It names a
pull request (`nifeos/nife#2797`), a test path or a bench path, and the check proves that it
resolves: the path exists, or the pull request number has the right form. The check cannot prove
that the cited run called the work. A reviewer reads that, as a reviewer reads a `Reuse:` line
today. Landing the code is not a first use. A unit test of the producer alone is not one either,
because the consumer did not call it.

A producer whose consumer is not yet built stays `PARTIAL`, with an `Outstanding.` bullet naming
the consumer it waits for. The status vocabulary already carries that meaning.

### 3. A large producer arrives with its thinnest real caller

When the producer is large, the first pull request includes the smallest real caller that
exercises it, and later pull requests grow the producer from what that caller needs. No gate could
hold a number for how large is large, so this part is a review rule in the developer-lane skill. The
`first_use` key makes it cheap to follow: the caller's test is the citation.

### 4. A periodic audit lists delivered work with no dependents

`helpers/consumer_census.py` prints the counts below. It reads the tree, never fails and runs in
about ten seconds. The proposal is to run it weekly beside `script/metrics`, post its lists, and
give each finding a disposition: a consumer named, or a removal proposed. A REMOVED status already
exists for the second.

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

Where nife differs. Linux and Rust gate one interface kind each, exports and library APIs. This rule
covers every milestone, so it needs a `none (<reason>)` escape that neither has. Kubernetes asks
for usage at graduation, as here, but leaves the evidence to prose; a `first_use` key makes it
something a script can resolve. No system found here back-fills the rule onto existing work, and
this proposal does not either.

## The example: milestones 695 and 697

Milestone 607 (nife is built and updated from packages) lists seven items. Item 6 is the trust
handoff: the loader hands the kernel two digests, the kernel checks the progenitor, and the
progenitor checks the base set. Milestones 695 and 697 carry it. The architect holds their code, as of
2026-10-31, until the updater shows what a kernel download costs per update.

Under this rule both blocks would carry `consumer: milestone 976`. Milestone 976 (an OS update is a
set of packages that lands through a slot) is the updater, and its step 3 reads the new manifest
through the loader. That step is the first thing that would call the handoff. The `consumer` key
records what the hold already says in prose: the producer waits for the consumer, and the consumer
decides what the producer must be.

When the work lands, `first_use` cites the pull request in which 976's updater staged a slot and
booted it through the two-digest handoff. A handoff that passes its own unit tests, with no update
having crossed it, stays `PARTIAL` under an `Outstanding.` bullet naming 976. Part 3 applies as
well. The handoff touches three boot protocols, so its first pull request would carry the thinnest
real caller, one update on one architecture, and the other two would follow from what that run
showed.

A second example came out of the measurement. Milestone 126 (`process-tools`) is `BUILT` and the
base image ships its `pmap`. No code in the tree names `pmap` to start it, the shell's program catalog does not
list it, and `crates/pmap`'s own `BUGS` section says nothing at the prompt grants it an address
space. The rule would have kept 126 `PARTIAL` with that consumer named, or kept `pmap` out of the
image.

## How the tree stands

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

- The crate side is already clean. The one finding, `allocation_cache`, reaches `std` through
  generated source (`cargo xtask std-src`), which no `Cargo.toml` names. The audit's crate half
  would find little today, and its value is in holding that.
- The program side is not. Six of the ten test-only programs are services no boot starts:
  `root_supervisor`, `spawner`, `sub_server_supervisor`, `swapper`, `fs_file_caretaker` and
  `network_time_client`. The system tests boot each, so a test proves each, and no image runs any of
  them. The other four (`slabtop`, `vmstat`, `disk_partitioner`, `disk_surveyor`) are commands a
  person types, and their consumer is that person; each would carry `none (an interactive command)`
  or name the shell.
- The 52 `BUILT` blocks with no candidate are mostly process work: lint gates, file splits,
  workflow fixes. Their consumer is a gate or a reader, which the `gate` and `none` forms carry.

The census reads a program as reached when code outside the tests spells its name. A program
started by a name built at run time would read as unused; none is, today.

## What back-filling would cost

Auto-derivation reaches far less than hoped. A strict default, a block's reverse
`milestone_dependencies`, gives a consumer to 91 of 493 `BUILT` blocks and 50 of 386 open ones. A
looser pass finds a candidate in most of the rest: 310 `BUILT` blocks name a `script/` path or a
workflow, 30 name a base-image program and 20 a crate. But a candidate is not a consumer. A block
that names `script/lint` may have built the check or merely run it, and only a reader can tell.

Back-filling every block therefore means a reader for about 740 blocks, with the 141 strict
defaults written by script. At the effort calibration in `notes/roadmap/effort.md` that is several
lanes of reading, each judgment cheap and the total not. It also writes 493 claims about finished
work that nobody will act on, since `BUILT` blocks have no lane left to steer.

So the recommendation is the `Reuse:` precedent. The keys bind blocks raised on or after the ruling
date. An open block gains `consumer` the next time a lane touches it, or by the strict default where
one exists. A `BUILT` block gains nothing, and the audit reaches it through the code instead: the
crate, program and contract lists name the unused output directly, whichever block built it.

## The plan

1. The architect rules on the questions below. A decision section records the rule.
2. An integrator promotes this proposal. The implementing lane adds `consumer` and `first_use` to
   `KEY` in `helpers/roadmap_block.py`, and `script/roadmap --check` gains `consumer_problems` and
   `first_use_problems`, each starting at a `CONSUMER_FROM` date as `reuse_problems` starts at
   `REUSE_FROM`. Each refusal names its fix, as `reuse_problems` does. `--selftest` gains fixtures
   that must fail: a new block with no consumer, a `none` with no reason, a `milestone N` that
   resolves to nothing, and a `BUILT` with no `first_use`.
3. `notes/roadmap.md` gains both keys in its frontmatter table, and the developer-lane skill gains
   part 3.
4. The census runs weekly, and its lists land somewhere a maintainer reads. Its first run opens the
   six boot-less services and `pmap` for disposition.
5. A contract column joins the census once the tree marks its contract crates in a form a script
   can read; today only the `_contract` suffix does, and that is a naming convention, not a field.

The census exists now and changes no gate. Nothing else here exists until the rulings.

## Open questions for the architect

One at a time, each with a recommendation. The first blocks the rest.

1. Does a block name its consumer in a frontmatter key that lint checks? Recommended: yes, a
   `consumer` key. The other options are a `Consumer:` body line, like `Reuse:`, or no record and
   the audit alone. The key resolves by script and joins the schema the dependency fields already
   use. The schema is the architect's, and this adds to it.
2. Which blocks must carry it? Recommended: blocks raised on or after the ruling date, with open
   blocks gaining it on their next edit. The alternative is a back-fill of about 740 hand reads,
   costed above. This recommendation is partly about effort, and it says so; the other part is
   that a claim about finished work steers nothing.
3. Is `first_use` a key, required with `BUILT`? Recommended: yes, a key, beside `built`. A body line
   would serve, but `built` is already a key and the two belong together.
4. Does a test count as first use? Recommended: a test counts when the consumer is what it runs,
   such as a system test that boots the updater through the handoff. A unit test of the producer
   alone does not.
5. Is part 3, the thinnest real caller, a gate or a review rule? Recommended: a review rule in the
   developer-lane skill. No gate can measure "large", and lanes would route around a number.
6. Where does the audit's output go, and who disposes of a finding? Recommended: a weekly report
   beside `script/metrics`, with each finding becoming a proposal under this directory, to name a
   consumer or to remove the work.
7. What happens to the six services no boot starts? Recommended: each names its consumer
   milestone, or goes to removal, decided one service at a time. This is the first audit's work,
   not this proposal's.

## Index row

Nothing in `BUILT` asks whether anything uses a milestone's result. This proposes a `consumer` key
on every new block and a `first_use` key that `BUILT` requires. A large producer arrives with its
thinnest real caller, and a weekly audit lists delivered work with no dependents. Today that audit
finds one unreached base-image program and six services that only tests boot.
