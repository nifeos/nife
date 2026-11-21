---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1034
decision_dependencies: 151, 305
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1081. `capability_table`: what an outside maintainer would fix before it leaves

Raised by a read-only outside review of `crates/capability_table`, met as a stranger to this tree would
meet it on the day it becomes `nifeos/<crate>`. The review changed nothing; it lists what the move
lane and the lanes after it change.

The crate's review package is [`notes/crate-reviews/capability-table.md`](../../notes/crate-reviews/capability-table.md),
indexed in [`notes/crate-reviews.md`](../../notes/crate-reviews.md). This review treats as settled
whatever a ruling in the tree already covers. That is the crate name as ratified against `caps`
and `cap_space` (reopened below for one reason only, item D1, ruled `capability_table`), `CapabilityTable` as the exported
type and `MAXIMUM_SLOTS` as ratified. It also covers the five right bits and their positions (the
system call wire format, pinned by `abi::rights` and asserted equal in
`kernel/src/capability_object.rs`), the set-means-free word, the 64-slot ceiling and the
publishing hold. Where a later rule contradicts an earlier
ruling, the item says so.

The kernel is the crate's only consumer (`kernel/Cargo.toml` line 52; no other manifest in the tree
depends on it). Milestone 1034 (a runbook for moving a crate out of nife) owns the move itself; this block owns the
findings that should land inside that path, and the two findings against the move tool that the
lane will hit first.

Reuse: `script/crate-move`, the template `nifeos/crate-workflows` at its newest release and at head,
`helpers/falsify.py`, `helpers/standalone.py`, the template's naming helpers, cargo-mutants' recorded
rows, Kani, and the crate's own harness builder (`any_small_capability_table`). No new tool.

## Finding status

- P0-1, the manifest stands alone: Done, [#3888](https://github.com/nifeos/nife/pull/3888). The
  manifest carries its own license, lints, `rust-version` 1.85, `description` and `repository`, and
  the crate has a README and both license files.
- P0-2, the crate's text refers back into nife: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565),
  [#3656](https://github.com/nifeos/nife/pull/3656). Only the `Name:` blocks remain, and the move
  strips them.
- P1-3, the proved `derive` is not the delegation the kernel runs: Done. `Capability::delegate`
  holds the rule once, the kernel's `Delegation::derive` calls it, and a harness proves it
  ([#3656](https://github.com/nifeos/nife/pull/3656)). The two `REVOKE` arms in
  `kernel/src/system_call.rs` test `GRANT` inline with no harness, by the maintainer's provisional
  ruling ([#3888](https://github.com/nifeos/nife/pull/3888)). The reason sits beside them. A revoke
  mints nothing, `allows` has its proof in the crate, and a harness of the arm needs stubs of the
  current thread, its table and the frame revoke that the kernel proofs do not have.
- P1-4, a slot truncates on a 32-bit target: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565).
- P1-5, `len` means capacity: Done, [#3656](https://github.com/nifeos/nife/pull/3656). The name
  The ruling of 2026-11-19 (UTC) keeps the name `capacity`.
- P1-6, the move tool corrupts patches and skips `repository`: Done,
  [#3521](https://github.com/nifeos/nife/pull/3521).
- P1-7, two mutators outside every harness: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- P2-8, `Rights::ALL` is a literal: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- P2-9, abbreviated parameter names: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- P2-10, `Error` is thin for a stranger: Done, [#3565](https://github.com/nifeos/nife/pull/3565),
  [#3656](https://github.com/nifeos/nife/pull/3656). The ruling of 2026-11-19 (UTC) keeps `GrantRequired` and
  `SlotOccupied`.
- P2-11, the process-wide peak is unconditional: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565),
  [#3888](https://github.com/nifeos/nife/pull/3888). `highest_seen` and its static are behind the
  cargo feature `peak`, off by default, which the kernel turns on (the maintainer's provisional
  ruling). The feature's name joins the release name review.
- P2-12, no read-only walk, and `delete_matching` takes `Fn`: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565),
  [#3656](https://github.com/nifeos/nife/pull/3656).
- P2-13, the mutation record travels as unknown: Done,
  [#3888](https://github.com/nifeos/nife/pull/3888). The crate's `.github/mutants-exempts` lists the
  three survivors and the four timeouts with their reasons, and `.cargo/mutants.toml` excludes the
  Kani proofs.
- P2-14, one test pins the global exactly: Declined. No table has more than 64 slots, so no test can
  exceed the pinned `(64, 64)`, and a wider table cannot break the exact assertion.
- P2-15, notes beside the crate have drifted: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565),
  [#3656](https://github.com/nifeos/nife/pull/3656).
- P2-16, `Option<Capability<O>>` has no niche: Open. Optional; it changes the input domain of every
  rights harness, so a lane measures it before taking it.
- D1, the crate's name: Done (ruled 2026-11-16 UTC, option A),
  [#3674](https://github.com/nifeos/nife/pull/3674).
- The five names: Done (ruled 2026-11-19 UTC, option A). `delegate`, `GrantRequired`, `SlotOccupied`,
  `capacity` and `iter` stand as built.
- The move: Open (cleared 2026-11-21 UTC, option A). [#3888](https://github.com/nifeos/nife/pull/3888)
  readies the crate in the tree: `Reap`, `reap_decision` and `survey_includes` stay in nife, in
  `kernel/src/sched/region_reaping.rs` with their four proofs in `region_reaping/verification.rs`
  (the place is the lane's provisional choice). Next: create nifeos/capability_table from the dry
  run, then the pin pull request.

## Open decisions

None. The clearance below has its ruling.

### Clear the review, which creates nifeos/capability_table

Ruled 2026-11-21 (UTC), option A. The crate moves to nifeos/capability_table with `publish =
false`, no tag and no publish. It carries the table, the rights algebra and `Capability::delegate`.
`Reap`, `reap_decision` and `survey_includes`, with their Kani harnesses and falsification patches,
stay in nife, wired into `script/verify` and `script/falsifications`, with no change of behavior.

Background: a crate that leaves nife gets a review package under `notes/crate-reviews/`, which an
architect reads; clearing it is the go to create the crate's repository and move the crate there.
The package for this crate is `notes/crate-reviews/capability-table.md`. The crate's only consumer
is the kernel, which then depends on it at a pinned revision.

Recommendation: A.

- A. The architect clears the review package; the
  manifest, README, licenses and exemptions (P0-1, P2-13) come with the move.
- B. Land P0-1 and the `peak` feature (P2-11) in the tree first, so the package shows the crate as
  it will publish.

Door: one-way. The template's workflows and nife's pin act on a repository and a moved
crate from the first day; moving back is a second move.

Prior art (recalled, not read): the Rust project moved `libc` out of the compiler's tree into a
repository of its own, and the compiler consumes it back by version. Where nife differs: a review
package checks that the crate stands alone before it leaves, not after. Example:
`jh7110_entropy`, whose review the architect cleared on 2026-11-01 (UTC), lives in nifeos/jh7110_entropy,
and nife depends on it at a pinned revision.

What it blocks: the move, and so the first tag and the first publish.

## Goal

A stranger who clones the crate's repository can tell in one screen what it is for, and can build, test, prove and falsify it from the README alone. The property the README leads with (a capability passed on never carries more than its source held) holds by proof on the code the kernel runs, not on a neighbor of it. The repository's gates are green for the crate's own reasons on its first
pull request.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1081-capability-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1081-capability-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the move or the release (a repository name, a
published name, a published behavior, a layout, a license); everything else is two-way and the
maintainer decides it. A name proposed below is provisional unless a ruling under Decisions for the architect ratifies it.

### P0: blocks the move

The P0 items are in [the appendix](1081-capability-outside-review/blocks-the-move.md).

### P1: before the first green pull request in the new repository

The P1 items are in [the appendix](1081-capability-outside-review/before-the-first-green-pull-request.md).

### P2: later

The P2 items are in [the appendix](1081-capability-outside-review/later-items.md).

## Decisions for the architect

D1 and D2 have their rulings. Everything else above is two-way until the first release, and the
maintainer decides it, provisionally. The review clearance that creates the repository is under
[Open decisions](#open-decisions).

### D1. The crate's name is `capability_table`

Ruled 2026-11-16 (UTC), option A. The crate carries the name `capability_table` from before the move. The name
`capability` belongs on crates.io to an unrelated published crate (a skill-tree library for games,
version 0.1.0, about 520 downloads all time, last updated more than a year before this review).
A repository named `nifeos/capability` could never publish under its own name. A rename after the move
would be a second move: a new repository, a new pin, and a dead name in every record between.

The name is free on crates.io, it names the thing the crate is (a table of capabilities with their
rights), it reads at the use site (`capability_table::CapabilityTable`, `capability_table::Rights`),
and it keeps `capability` as the word the type names carry. The earlier refusal of `cap_space` was
that it named one of five exports and stuttered; `capability_table` names the central export, and the
stutter is the one `slotmap::SlotMap`, `slab::Slab` and `generational_table::Table` all accept.
The review refuses a `nife_` prefix, because a crate that leaves does not mention nife. It also
refuses `capability` as a repository name that can never publish, because that name closes the door
§151 (the goal of the repository split is independent release) opens. There is no
`#[doc(alias = "capability")]` on the crate: no old nife name carries an alias before the first
release.

Prior art. Fuchsia publishes its handle and rights types as `fuchsia-zircon`, a name for the system and not for "handle". seL4's term is `CSpace`, which this review already refused. The Rust crates for a slot structure take the structure's name (`slab`, `slotmap`, `thunderdome`, `generational_table`).
Rust's convention for a taken name is a qualifier, not a prefix (`cap-std`, `cap-primitives`).

Concrete example. The kernel's line is `capability_table = { version = "0.1.0",
git = "https://github.com/nifeos/capability_table", rev = "<tag commit>" }` once the repository
exists, and the two kernel files that name the crate (`kernel/src/capability_object.rs` and
`kernel/src/sched/region_reaping.rs`, 17 lines) say `capability_table::`; `capability_object.rs` keeps
its alias `pub(crate) type CapabilityTable = capability_table::CapabilityTable<Object,
CAPABILITY_TABLE_SLOTS>`.

Door: one-way (a repository name and a published crate name), and the ruling closes it.

### D2. The five names stand as built

Ruled 2026-11-19 (UTC), option A. `Capability::delegate`, `Error::GrantRequired`,
`Error::SlotOccupied`, `CapabilityTable::capacity` and `CapabilityTable::iter` stand as built.
`Capability::delegate` makes a copy of a capability with fewer rights and refuses without the
`GRANT` right (`Error::GrantRequired`) or when the copy would hold a right the source lacks. The
kernel calls that one method, so the proof covers the code the kernel runs. `capacity` is the number
of slots, as in std's `Vec::capacity`, and `len` stays out because in std it counts the elements
held. `iter` walks the occupied slots. `Error::SlotOccupied` answers an insert at a slot already in
use, apart from a full table. The ruling refuses `derive` as a rename of `delegate` and adds no `len`.
The first publish turns each name into a promise, and this ruling is the clearance for that
promise.

Door: one-way at the first publish.

## Done when

The checks are in [the appendix](1081-capability-outside-review/done-when.md).

## Follow-on

- **Outstanding.** The Open lines under [Finding status](#finding-status) and the clearance under
  [Open decisions](#open-decisions).

## BUGS

- Item 4 came from reading and from clippy; the review ran no 32-bit build, because the host has no 32-bit target and the review adds nothing to a machine.
- Ten of the fourteen falsification patches the review found were not replayed; the four that were
  each failed exactly their harness. The tree's own `script/verify` replays every patch on its
  cadence.
- The dry run ran with `--no-build` and with the template at its newest release. The template's
  head carries checks the release lacks (spelled-out words, unit symbols, passive voice, function
  length); the review ran the two naming ones by hand and they pass, and it did not run the other two.
- The review did not look for a review package on any branch; if one exists, its rulings win over
  the names here.
- Item 16 is optional, and the review measured it only on a 64-bit host.
- Item 3's kernel half and item 12's three kernel walks were checked with `cargo check` for the three
  kernel targets and `--features system_tests`, and the crate's tests and Kani harnesses ran; the
  kernel's QEMU suite is left to the queue.

## Index row

An outside review of `capability_table` before it leaves. Two items block the move (a manifest that leans on the workspace and a README that does not exist; text that cites nife on 69 lines). Five land before the first green pull request. Three are the proved `derive`, which is not the delegation the kernel runs, a 32-bit slot truncation, and `len` that means capacity. The other two are two defects in the move tool that corrupt all fourteen patches and skip the manifest step, and two mutators outside every harness. Nine items are for later. The crate is named `capability_table`, because another crate already holds the name `capability` on crates.io.
