# How stable the interface is

*Milestone 610 (the interface's stability is measured weekly), its number provisional. calef,
2026-09-27 (UTC): "What I want are good metrics for capturing the stability of our interface."
calef, 2026-10-16 (UTC): the headline is the boundary a program outside the tree is written
against, not every public item in the tree. The numbers are on [the metrics
page](project-metrics.md#interface-stability), per crate and per package in [the
appendix](project-metrics/interface-stability.md), and produced by
`helpers/interface_stability.py`. The note's name, the program's, the three CSVs' and every
column's are provisional.*

## What the interface is

What a program that is not the kernel is written against, and what would break it if it changed.
The headline measures the boundary, two parts:

- The syscall surface: every public constant in `abi`, the `SYS_*` numbers, the `objtype::*`
  object types, and the method numbers and argument or result encodings in the object modules.
  The values of `abi`'s enums (the `Error::*` codes) count too, because a program matches
  them by number exactly as it does a constant.
  A new syscall number or a method outside the capability model is a design fork, so this
  surface moves by decision.
- The wire formats: a contract crate's public constant named `*VERSION*`, `*MAGIC*` or
  `*REVISION*` is a format identity, and a value change is a format bump.

Two more surfaces are read weekly and reported beside the headline, never in it (calef,
2026-10-16 UTC):

- The contract crates' remaining public API, item level, in `interface_contract_api_*`. The
  fixture extraction and the file-split campaign land here.
- The std ABI: every crate the farm's `std-src` copies into the standard library, item level, in
  `interface_std_abi_*`.

A program built from packages is written against stable `std` and the kernel boundary, not
against a contract crate's internals, so those two are coupling to watch, like co-change, not
instability of the interface. The contract set and the copy set both live in code (`CONTRACTS`
in the helper, the farm module it reads), so this note cannot drift from either.

## What each column counts

One row per ISO week in `notes/project-metrics/interface-stability.csv`. Each week's first-parent
commit (the same commit `script/metrics` picks) is compared with the previous week's.

| column | counts |
|---|---|
| `interface_breaking` | boundary moves: `abi` constants and enum variants (the `Error::*` codes) renumbered, removed, or renamed with the number kept. Format identity value changes count too |
| `interface_additions` | `abi` constants and enum variants added |
| `interface_std_abi_breaking`, `_additions` | item-level churn in the std copy set; reported, not thresholded |
| `interface_contract_api_breaking`, `_additions` | item-level churn in the contract crates beyond the boundary; reported, not thresholded |
| `interface_syscall_changed` | `abi` constants and enum variants removed or renumbered with no successor |
| `interface_syscall_renamed` | `abi` constants renamed with the number kept |
| `interface_syscall_added` | `abi` constants and enum variants added |
| `interface_format_bumps` | a format identity's value changed |
| `interface_format_amendments` | dated entries in a format's own amendment ledger |
| `interface_contract_commits`, `_crossing`, `_cochange_pct` | co-change, below |
| `interface_cochange_pct_excluding_mass` | the same without mass mechanical commits |
| `interface_crates_unmeasured` | crates rustdoc could not document on either side |
| `interface_package_commits`, `_crossing`, `_cochange_pct` | per-package co-change, below, each commit once |

An item is anything a consumer names. That is a function, a constant or a type, and each struct
field, enum variant, trait item, inherent method, and trait a type implements. "Changed" means its
signature, type, field order, `repr`, discriminant or constant value moved. A parameter's name and
a constant's spelling (`1 << 2` for `4`) are not part of it. A crate rename is one event, not one
per item, because it is one decision and every consumer fixes it with one line.

**How it is read.** rustdoc's JSON output, on the pinned nightly, for
`aarch64-unknown-none-softfloat`, over a `git archive` of each week's tree in `target/`. The helper
checks nothing out. The toolchain is always HEAD's, so every week's JSON has one format. A constant
rustdoc cannot evaluate (it writes `_` for `*b"CRKR0002"`) is read from the source its span points
at, which is how the one format bump in the history was found. Old crate names follow git's rename
detection to today's name, preferring the renamed `src/lib.rs` over the renamed `Cargo.toml`,
because git paired `uheap`'s manifest with `line_editor`'s in the 2026-08 naming sweep.

**The re-export rule.** A `pub use` is the item it names. The walk follows rustdoc's re-export to
the target and records the target's fingerprint under the re-export's local name, because the
path a consumer writes is the re-export; format 61 carries the local name and the target's id in
`inner.use`. Moving a definition behind `pub use`, which is what a §282 (when a file is split,
the parent becomes an index) split does, moves no surface. A foreign re-export contributes the
name without a shape.

**The syscall rename rule.** `endpoint::SEND` becoming `rendezvous::SEND` at the same value breaks
every caller's source and no binary. A removed `abi` constant is a rename when exactly one added
constant has its value and keeps either its leaf name or its module; a module another constant
already moved unambiguously resolves the rest. Anything less certain stays a change.

**Co-change** reuses the method of the measurement behind #1389's 76%, as the maintainer forwarded
it on 2026-09-26: merges excluded, documentation excluded from both sides, and a commit crosses when
it touches any non-documentation path outside the contract directories. A commit is mass mechanical
when it touches more than 40 files, spans more than 8 top-level areas, or its subject reads like a
rename or a migration. Over #1389's 60-day window with today's directory names this reads 78.6% of
224 commits, against that measurement's 78.8% of 226. The weekly series also counts commits to a
contract's older directory names, which raises the window to 378 commits and moves the share to
78.3%.

**Past cells are carried.** `--update` writes the current week and any week the CSV lacks, and
`script/metrics` never recomputes these cells. A week's old source that compiles on today's nightly
may not compile on next month's, and a past number must not change for that reason. `--backfill`
restates every week from the per-week surface caches, rebuilding any week whose cache is missing
or does not cover every crate in the interface.

## Co-change per package

§235 (the OS is built and updated from packages, and the tree divides by what releases together)
sizes a home by what changes together, and no package moves before this measurement exists. A package is
what §235 and `packages/` mean by one: a `packages/*.package.toml` file, which claims crate
directories, program sources and paths. Each Cargo package has a row too, because a crate
released on its own (milestone 857 (crates released on their own)) leaves as one.

The method is the contracts' co-change with each unit in the contracts' place. A commit touches a
unit when one of its non-documentation paths belongs to it. It crosses when another of those paths
does not, and it touches another when that path belongs to a different unit of the same kind. A
path that `packages/homes.toml` claims, such as `script/`, crosses but is no other package. The
weekly cells count each commit once: commits touching any package, and those crossing one.

`notes/project-metrics/interface-stability-packages.csv` holds one row per unit for each week a commit
touched it. `kind` is `package` or `crate`, then `commits`, `commits_crossing`,
`commits_touching_another`, and the first two again without mass mechanical commits. The appendix
lists every package with the partner it most often shares a commit with, and the release-track
crates. These cells need no build, so every run restates them from git under today's declarations.

The first reading, 2026W29 to 2026W43: 34% of commits touching a package crossed one over the last
four weeks, down from 65% in 2026W29. With mass changes excluded, only `kernel` crosses on fewer
than half its commits. The commonest pair is `kernel` and `contracts`, in 295 commits.

## The amendment ledger

An in-place change to a released format version cannot be derived: the layout may move with no
public item changing, and a public item may change with no byte on the wire moving. So it is
recorded at the format, AGENTS.md's rung 3, in the crate's own module documentation:

```text
//! # Amendments
//!
//! - 2026-09-27: version 1's field order changed in place, because ...
```

One list item per amendment, opening with its UTC date. The heading and the shape are this lane's,
provisional. No crate carries one yet. `capability_manifest_note`'s version 1, amended in place on 2026-09-27
on a branch not yet merged, is the first that should.

## First reading, and the trend

The series restated 2026-10-16 under the boundary headline and the re-export rule. Over the last
four weeks (2026W39 to 2026W42): 10 boundary moves, 49 syscall additions, no format bump, and 74%
of 297 contract commits crossing another division. The same four weeks read 674 breaking changes
under the previous headline, 246 of them the filesystem test fixture moving to its own crate and
176 of them three crate-root index splits that kept every consumer path.

| weeks | boundary breaking | syscall additions | what moved |
|---|---:|---:|---|
| W29 to W30 | 0 | 38 | the first weeks |
| W31 to W33 | 1 | 5 | |
| W34 to W36 | 29 | 34 | the naming sweeps: `endpoint` to `rendezvous`, `untyped` to `memory_region`, the `_proto` crates |
| W37 to W39 | 0 | 17 | |
| W40 to W42 | 10 | 33 | `irq` to `interrupt` and three register spellings spelled out |

The one format bump in the history is `crickerfs`' magic going from `CRKR0001` to `CRKR0002` on
2026-08-01. Contract API churn peaked at 158 items in 2026W41, the week the crate-root split
campaign crossed the contract crates, and reads 17 in 2026W42.

## What "stable" would mean

A threshold proposal only. The threshold is calef's call.

Over a trailing four weeks, `interface_breaking` is zero: no syscall number renumbered or
removed, no number-kept rename, no format changed in place without a bump. Additions are growth,
not instability, and they are the leading indicator: the boundary has never broken much, and it is
still gaining about a dozen syscall constants a week. Contract API churn, std copy churn and
co-change are reported and not thresholded; §151 (the goal of the repository split is independent
release and third-party programs) wants them low before a split.

By this proposal no four-week window in the history has been stable. The closest, 2026W36 to
2026W39, reads 2.

## Why rustdoc JSON and not a semver tool

rustdoc's JSON is on the pinned nightly and adds no dependency. `cargo-semver-checks` and
`cargo-public-api` both read the same JSON. What they add is a catalog of rules for whether a
change needs a new major version (from memory, not verified here). This series asks a narrower
question, whether the public surface moved, so neither is needed. Both also pin a rustdoc JSON
format version, which would tie the tool's version to the nightly pin. That argues against taking
either, and taking one is a §46 (thin primitives or whole subsystems) decision for calef.

## BUGS

- A semantic change behind an unchanged signature is invisible: a function that now returns a
  different value, a message whose meaning moved, a method whose rights check changed.
- A wire layout can change with no public item changing (an encoder's field order in a function
  body), and only the ledger sees that, if someone writes it. Nothing checks that a ledger entry
  exists.
- Only public items rustdoc documents are read. `#[doc(hidden)]` items, anything behind a
  `cfg(target_arch)` for riscv64 or x86_64, and auto traits (`Send`, `Sync`, `Unpin`) lost through
  a field change are not seen. Blanket impls are skipped on purpose.
- A rename reads as one removal and one addition unless the old name survives as a re-export, in
  which case it reads as nothing, which is honest: the consumer's paths did not move.
- A re-export retargeted to a different item of the same fingerprint reads as no change; the
  fingerprint cannot tell two same-shaped types apart.
- Items that reach a consumer only through a glob re-export (`pub use module::*`) are unseen: the
  glob is keyed by its source text and not expanded. No measured crate re-exports this way today;
  `domain_name_resolution_contract` and `user_mode_runtime` do, and are not measured.
- The syscall rename rule overcounts on purpose: `CSPACE_SLOTS` becoming `CAPABILITY_TABLE_SLOTS`
  in 2026W35 had no unique successor, so it counts as removed.
- A macro's fingerprint is its source text, so an edit to its body is a change even when its
  expansion is not.
- The first week counts every item as an addition, because there is no week before it.
- Membership is today's list applied to every past week, like every restated series on the metrics
  page. A contract crate deleted before today (`multicast_dns_protocol`, `smb_proto`) was never a
  member, so neither its life nor its deletion is counted.
- The first-half co-change finding in the maintainer's measurement (87.8% against 76.5%) used
  today's directory names only; this series' early weeks include older names and read differently.
- Per-package co-change places a past path by today's declarations, following crate directory
  renames only. A path that has since gone, or moved outside `crates/`, belongs to no unit,
  so a commit touching only such paths counts for nothing, and one touching them beside a unit crosses.
- A workspace file (`Cargo.lock`, the root `Cargo.toml`) is outside every unit, so adding a
  dependency crosses even when no other code moved. After a split the same commit would still
  touch two repositories' lock files, so it stays counted.
- A crate's figure rests on a few dozen commits at most, and a lane editing several release-track
  crates at once makes them co-change with each other by schedule, not by coupling.
- The contracts' walk counts a documentation file inside a contract directory (a crate's
  `README.md`) as a contract commit, while the method excludes documentation from both sides and
  the per-package walk does. A commit touching only such a file counts there and not here.
- `--update` recomputes only the current week and missing weeks. A ledger entry dated in a past
  week reaches the series only on a `--backfill`.
