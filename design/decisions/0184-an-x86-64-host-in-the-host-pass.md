---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-28
ratified_by: calef
---

# 184. Does the host test pass run on a second architecture, and at what cadence?

Ruled 2026-10-28 (UTC): Option 2, matrix leg — the host test pass runs on both architectures
(arm64 and x86_64) on every pull request. Option 1 dominated (same runner cost, less honest shape);
option 3 rejected because cost alone should not drive a day of latency. Unblocks milestone 403.

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 403 (an x86_64 host in the host pass)'s
`DECISION` gate naming no section. Milestone 288 fixed four instances of "a test that states a fact
about the author's laptop" in `crates/elf` and could not stop a fifth. *(Section number provisional
until the merge queue lands it.)*

## What is being decided

Whether this project pays runner concurrency, on every pull request, for a class of fault it has
met three times in two months. That is a standing cost rather than a one-off edit, which is what
routes it here. The edits themselves are minutes.

## Is the premise true

Checked 2026-09-19 in this worktree, and it is, with one clarification that prices two of the three
options lower than the proposal assumed. Every `runs-on:` in `.github/workflows/` is
`ubuntu-24.04-arm` (29 of them) except one: `verify.yml`'s `prove-kernel-x86_64`, which runs
`script/verify --only kernel` because CBMC needs a goto-binary for the host it runs on.

So no host test pass has ever run on x86_64, which is the claim, and the x86_64 runner image
is already in use for a different job, so the "we would be introducing a new runner" objection
does not apply.

*(Dated note, 2026-09-26, decisions-hygiene lane.)* The runner economics this section prices were
reversed on 2026-09-24. Milestone 587 (most CI jobs do not need an arm64 host), on calef's ruling
that day, moved fifteen jobs to `ubuntu-24.04` (commit `0695ab7d8`) because the arm64 pool was where
every run waited. In same-run pairs the x86_64 job waited less, often by an order of magnitude. So
option 1's cost, "one runner slot per pull request, competing with group builds", is now a slot in
the less contended pool. And one host pass already runs on x86_64 as a side effect: the `coverage`
job moved with the rest, and `script/coverage` runs `cargo llvm-cov --workspace`, the workspace's host
tests, on every pull request that is not documentation only. `build + test` stays on arm64, so the
full host pass (`script/test`'s) still has not run on x86_64 in CI. Whether coverage's leg is enough
to close the class is the question this note leaves to calef, and it narrows options 1 and 2.

## What is not covered

Every machine that has ever run this suite is aarch64: the development machine is Apple Silicon and
CI is `ubuntu-24.04-arm`. Milestone 288 made the class unrepresentable where it could
(`FOREIGN_MACHINES` is derived, a never-a-nife-machine number is checked in a `const`). The residue
is plain: `b.e_machine = 183` still compiles on every host, as does any other literal standing in
for a host-relative fact, and nothing in this repository would notice.

Three instances, each found by a stranger rather than by a gate:

- `xtask`'s host pass stopped compiling on x86_64 when three crates took `user_rt` dependencies, and
  *"nobody noticed, because CI moved to `ubuntu-24.04-arm` the same day"*. Found 2026-08-14 by
  milestone 117's first run on a clean x86_64 checkout.
- `crates/elf` failed 20 of 25 host tests on x86_64 from at least milestone 161 until milestone 288,
  with two milestones filing it as a proposal before one closed it.
- `fuzz/seeds/elf_parse/` seeded an empty corpus on x86_64, silently, because the note recording the
  host dependence was written before x86_64 was a target.

## What this tree already does in the analogous case

§19 makes parity a gate rather than an aspiration, and it is worth being precise about what it
reaches: it governs the *kernel capability* shipping on every supported ISA, proved by the same
suite. The host pass is the machine the tests are compiled and run on, which §19 does not name.
So this section widens §19's posture by analogy rather than being forced by it, and saying so keeps
the citation honest.

§74 is the closer analogue and it argues against a cadence. It decided, for audits, that event
triggers come first, a count-based trigger second, and the calendar only as a backstop, on the
reason that "eventually is the wrong word for an attack surface". A host-portability fault is not an
attack surface, which is the honest difference, but the three instances above each survived for
weeks, which is what a cadence would also permit.

`script/stranger-test` is the mechanism already aimed at this class, and its existing posture is
periodic rather than per-pull-request. It found instance one.

## The options

| | what | cost |
|---|---|---|
| **1** | a second CI job on `ubuntu-24.04` (x86_64) running only the host pass | the smallest thing that closes the class; one runner slot per pull request, competing with group builds for the same concurrency, which AGENTS.md already names as a real ceiling |
| **2** | a matrix leg: the host pass on both architectures | same runner cost, tidier shape, and the claim becomes symmetric rather than x86_64 being bolted on |
| **3** | a periodic run | cheapest, `script/stranger-test`'s existing posture, and it finds the fault a day late |

Option 3's "a day late" should be read against what actually happened. Three faults went
undetected for weeks, so a day late would have been an enormous improvement over the status quo,
and that is the measurement rather than a concession.

**Deliberately not proposed: a lint that greps for machine literals.** `git grep -w TODO`'s 82%
false-positive rate is this tree's own worked example of why, and a number is exactly the kind of
token a grep cannot tell from its legitimate uses, since `crates/elf` must name 183, 243 and 62
somewhere, and does.

## Recommendation

**2 if the runner minutes are affordable, 3 if they are not, and not 1.** Option 1 and option 2 cost
the same slot, and 2 buys a symmetric claim for it, so 1 is dominated: choosing 1 would be choosing
the less honest shape at the same price. Between 2 and 3 the trade is real concurrency against a
day's latency, and that is the judgment this section cannot make, because it is a standing cost on
a machine budget AGENTS.md already names as a ceiling.

## Would we still choose this if both options cost the same

No, and it must be said in those words: if runner concurrency were free, 2, without hesitation.
The entire case for 3 is cost. That is legitimate, and stating it lets calef weigh it as cost rather
than mistake it for judgment.

## How reversible, and who has acted on it

High. A CI file edit each way, nothing two programs agree on, and no name a stranger learns. The
irreversible part is the runner budget it consumes while it is in place, which is why the answer is
one sentence and the lane that follows it needs no further ruling.

## What is blocked until this is answered

Milestone 403. Nothing in the tree is incorrect meanwhile; the class is simply unpoliced.
