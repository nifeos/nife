---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1092. `byte_glob`: what an outside maintainer would fix before the crate leaves

Raised by a read-only outside review of the crate then at `crates/glob` (now `crates/byte_glob`), read the way a
maintainer meeting it for the first time would. The review changed nothing and opened nothing; every
item below is work for a lane. No review package exists for this crate yet, so the baseline the review
took as settled is the crate's own record: [notes/glob.md](../../notes/glob.md),
[notes/glob-scope.md](../../notes/glob-scope.md), [notes/glob/proofs.md](../../notes/glob/proofs.md)
and [notes/verification/glob-proofs.md](../../notes/verification/glob-proofs.md). The four scope
decisions those record are `**` out, qualifiers out, POSIX classes out and a leading dot special by
default. They stay closed here. So do the one-backtrack algorithm, the three-byte proof domain and
the ratified crate name `byte_glob`. A later rule or an outside fact reopens one only where the text says so. Milestone 1017 (the contract crates leave in this order) places the crate
at step 5 and owns the move; this block owns the findings that land inside that path.

Reuse: the crate's own tests, harnesses and falsification patches, the shared crate workflows and
their helpers, `script/crate-review-packet`, `helpers/name_release.py`, cargo-mutants and cargo-fuzz.
No new tool.

## Finding status

One line per finding, in the review's order: P0 in
[the blocks-the-move appendix](1092-glob-outside-review/blocks-the-move.md), P1 in
[the before-the-tag appendix](1092-glob-outside-review/before-the-tag.md), P2 in
[the later-items appendix](1092-glob-outside-review/later-items.md), then the decisions and the
release steps.

- P0-1, the crate name was taken on crates.io: Done, [#3578](https://github.com/nifeos/nife/pull/3578).
  The crate is `byte_glob` in code and package.
- P0-2, seven public names unratified: Done, [#3790](https://github.com/nifeos/nife/pull/3790) and
  [#3832](https://github.com/nifeos/nife/pull/3832). `Dot` is `LeadingDot` and `cost_bound` is
  `step_bound`, with no alias, and the review package ratified the seven.
- P0-3, the integration test links staying crates: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566). The test is in `crates/grant_plan/tests/`, and
  the matcher's manifest has no dev-dependency.
- P0-4, the shipped text cites the tree: Done, [#3836](https://github.com/nifeos/nife/pull/3836).
  Every citation became a sentence. The README carries the scope note, and its "What is proven"
  section with `src/verification.rs` carries the proofs note.
- P0-5, the manifest is not standalone: Done, [#3836](https://github.com/nifeos/nife/pull/3836).
  The manifest carries its own license, lints, `rust-version`, metadata and `include`, and the
  `publish = false` comment is in the crate's own words.
- P0-6, no README: Done, [#3566](https://github.com/nifeos/nife/pull/3566), with both license texts.
- P1-1, `naive` fails the question gate: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
  It is `naive_matches`.
- P1-2, the falsification patches name the tree's paths: Done,
  [#3836](https://github.com/nifeos/nife/pull/3836). The six patches diff `a/src/lib.rs` and the
  six records name `falsifications/...`; the `proofs` workflow is green on the repository's main.
- P1-3, mutation testing would reach the inline proof module: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566) split the file, and
  [nifeos/byte_glob#1](https://github.com/nifeos/byte_glob/pull/1) excludes `src/verification.rs`
  in the repository's `.cargo/mutants.toml`.
- P1-4, no `forbid(unsafe_code)`: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-5, the `/` caveat is missing: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-6, the module doc opens for this tree's reader: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-7, the length parameters lack a unit: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
  They are `pattern_bytes` and `name_bytes`.
- P2-1, the doc overstates the cost bound: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-2, `.*` matches `.` and `..`: Done, [#3962](https://github.com/nifeos/nife/pull/3962). The
  matcher's doc and README state it, and `grant_plan`'s `Expander::offer` says that no listing it
  sees carries either name, with a test that a listing which did makes `.*` refuse.
- P2-3, `match_steps` returns a tuple: Waiting on
  [nifeos/byte_glob#2](https://github.com/nifeos/byte_glob/pull/2), which returns a
  `#[non_exhaustive]` `Outcome { matched, steps }`, the name provisional.
- P2-4, no fuzz target: Waiting on [nifeos/byte_glob#2](https://github.com/nifeos/byte_glob/pull/2),
  a target against a separately written reference matcher, opted in to the weekly run.
- P2-5, two harnesses fix one dot rule: Will not fix in full, ruled 2026-11-20 by the maintainer as
  a two-way call in [nifeos/byte_glob#1](https://github.com/nifeos/byte_glob/pull/1).
  [#3566](https://github.com/nifeos/nife/pull/3566) gave both harnesses a symbolic dot rule and
  `any_lengths()`. The cost harness keeps it. The no-magic harness is stated under
  `LeadingDot::Ordinary` again, because the symbolic rule took 567 seconds of the 20-minute proofs
  budget, and the dot-rule harness proves the policy on its own.
- P2-6, stale solver times: Done, [#3566](https://github.com/nifeos/nife/pull/3566). The shipped
  text gives no per-harness seconds.
- P2-7, `Elem` is an abbreviation: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-8, no benchmark: Waiting on [nifeos/byte_glob#2](https://github.com/nifeos/byte_glob/pull/2),
  a standard-library `benches/matching.rs` over the three blowup shapes and one ordinary pattern.
- Decision 1, the registry name: Done, [#3578](https://github.com/nifeos/nife/pull/3578).
- Decision 2, the seven names: Done, [#3790](https://github.com/nifeos/nife/pull/3790) and
  [#3832](https://github.com/nifeos/nife/pull/3832).
- Decision 3, ratify the seven names and create `nifeos/byte_glob`: Done,
  [#3836](https://github.com/nifeos/nife/pull/3836). The repository exists with `publish = false`
  and no tag, and nife pins it by commit.
- R1, the review package: Done, [#3790](https://github.com/nifeos/nife/pull/3790), at
  [notes/crate-reviews/byte-glob.md](../../notes/crate-reviews/byte-glob.md).
- R2, ratify the seven names and clear `nifeos/byte_glob`: Done,
  [#3832](https://github.com/nifeos/nife/pull/3832).

## Open decisions

None.

## Goal

A stranger who finds this crate in its own repository can tell in one screen what it matches, what it
refuses to match, and why `matches(b"*", b".config")` is false. They can build it, run its tests and
its proofs from the README alone, and apply one falsification patch. The template's `check` and
`proofs` workflows pass on the move commit for the crate's own reasons. The public names are final
before the move strips their provenance, and the crate has a name it can one day publish under.

Publishing stays held.

## What the review covered

See [the appendix](1092-glob-outside-review/what-was-checked.md).

## Findings, prioritized

P0 blocks the move and the creation of the repository. P1 lands before the first tag there. P2
follows it. A door is one-way where someone outside the tree acts on the outcome (a published name,
the public API, a layout, a dependency); the maintainer decides the rest.

### P0: the name, the names, and what does not stand alone

The P0 items are in [the appendix](1092-glob-outside-review/blocks-the-move.md).

### P1: before the first tag

The P1 items are in [the appendix](1092-glob-outside-review/before-the-tag.md).

### P2: later

The P2 items are in [the appendix](1092-glob-outside-review/later-items.md).

## Decisions for the architect

Decisions 1 to 3 carry the architect's rulings. The baseline (scope, the default dot rule, the algorithm) stays closed.

### Decision 1. The registry name of the crate (P0-1)

Ruled 2026-11-14 (UTC): the crate is named `byte_glob`, in code and package alike. `crates/glob` is
`crates/byte_glob`, the package and the library are `byte_glob`, and every caller says
`byte_glob::matches`. The old name carries no `doc(alias)`. The `Name:` block is ratified for
`byte_glob`.

Background. The crate is a glob matcher, and `glob` is the right word for it. On crates.io that word
belongs to rust-lang's crate, which matches `str` patterns against paths, supports `**`, and has
over 700 million downloads. A crate name is the one name nobody can change after a publish, and the move creates the repository with whatever name the crate then has. The in-tree ratification of
`glob` stood as a description; the question was the qualifier.

The naming page's rule for crates: one word where one will do, an underscore where the name qualifies
a thing. One word will not do, so the qualifier should say what distinguishes this matcher from the
one everyone knows. Three facts do: it matches bytes rather than characters; it matches one file name
rather than a path; its cost has a bound and a proof. The prior art qualifies the same way:
`globset` (a set of globs), `glob-match` (the operation), `wildmatch` (the syntax subset).

Options, each verified free on crates.io:

- `byte_glob`: names the one property every other Rust glob crate lacks and the crate's own doc
  leads with ("Bytes, as `fnmatch(3)` in the C locale"). Reads at a call site as
  `byte_glob::matches(pattern, name)`. Ruled.
- `name_glob` or `filename_glob`: names the scope (one name, no path). `name_glob` is terse and
  slightly ambiguous ("a glob of names"); `filename_glob` is clear and long.
- `component_glob`: names the scope in path vocabulary a Unix reader knows and others do not.
- `shell_glob`: names the syntax's origin, which is also what rust-lang's crate implements.
- `fnmatch`: the C function's name, free, and a standard term; refused by the naming page because it
  is an abbreviation ("filename match") and a crate takes a noun.

Concrete example. With the ruling, `swish`'s manifest line becomes
`byte_glob = { version = "0.1.0", git = ..., rev = ... }` and `words.rs` reads
`byte_glob::has_magic(token)`. Without a change, the template's release dry run for this crate would
print the trusted-publishing door for a crate owned by rust-lang, and the registry would refuse a real run.

### Decision 2. The seven public names (P0-2)

Background. The move strips provenance, so these gain ratification in the review package or never. The
reviewer's reading of each against `design/naming.md`:

| name | kind | reading | recommendation |
|---|---|---|---|
| `matches(pattern, name)` | fn, bool | a third-person verb phrase, as the question rule asks | ratified |
| `matches_with(pattern, name, dot)` | fn, bool | the same, `_with` for the policy argument, as std's `_with` | ratified |
| `has_magic(pattern)` | fn, bool | `has_` before a thing held; "magic" is the word Python's `glob` module and every shell manual use for an unquoted wildcard | ratified |
| `literal(pattern, out)` | fn, Option | a noun for the thing returned, the pattern's one literal name | ratified |
| `match_steps(pattern, name, dot)` | fn, (bool, usize) | the steps of a match; P2-3 may give it a named return later | ratified |
| `cost_bound(pattern_len, name_len)` | const fn | a bound on the steps; the parameters are P1-7 | `step_bound` |
| `Dot { Special, Ordinary }` | enum | "dot" alone does not say which dot; the policy is about the first byte | `LeadingDot` |

`LeadingDot::Special` and `LeadingDot::Ordinary` read at a call site without the doc:
`matches_with(pattern, name, LeadingDot::Ordinary)`. Prior art spells the same policy
`FNM_PERIOD` (fnmatch), `require_literal_leading_dot` (rust-lang/glob), `dotglob` (bash) and
`GLOB_PERIOD` (glibc's `glob(3)`); every one of them names the position. The variants stay: they are
the two answers. The three call sites in the tree change with it
(`pgrep/src/lib.rs` lines 94 to 102, two mentions, and the doc tests).

Ruled 2026-11-20 (UTC): `matches`, `matches_with`, `has_magic`, `literal`, `match_steps` and
`LeadingDot` (variants `Special` and `Ordinary`) stand as built, and `cost_bound` becomes
`step_bound`, with no alias for either old spelling. `has_` and `is_` are for a `bool` only, and
`literal` is a noun for an `Option` value. `step_bound` sits beside `match_steps` and states the
unit. The tuple return of `match_steps` stays the maintainer's provisional call, a two-way door.

### Decision 3. Ratify the seven names and create `nifeos/byte_glob`

Ruled 2026-11-20 (UTC), option A: the architect cleared the review package. The repository
`nifeos/byte_glob` has the go, with `publish = false`, no tag and no publish. The move
(milestone 1017, step 5) starts once the prove shard is green and the `step_bound` rename has
landed. The package is [notes/crate-reviews/byte-glob.md](../../notes/crate-reviews/byte-glob.md).

Background. The move strips every `Name:` block, so the names leave the tree as whatever they are
on that day. A repository exists in the organization only once the architect clears its review. The
alternatives were clearing before the rename and holding until step 5.

Prior art: fnmatch spells the policy `FNM_PERIOD`, rust-lang/glob `require_literal_leading_dot`,
bash `dotglob` and glibc's `glob(3)` `GLOB_PERIOD`; each names the position, as `LeadingDot` does.
This tree cleared `generational_table` and `block_roster_contract` the same way, a review package
first and the repository after. A concrete example:
`byte_glob::matches_with(b"*", b".config", LeadingDot::Ordinary)` is `true` and reads without the
doc, where `Dot::Ordinary` leaves the reader asking which dot.

## Prior art

The comparison with nearby work is in [the appendix](1092-glob-outside-review/prior-art.md).

## Done when

The checks are in [the appendix](1092-glob-outside-review/done-when.md).

## BUGS

- The review ran two of the six harnesses and replayed one of the six patches. The rest rests on
  the crate's record and the tree's own gates.
- The P2 order is the reviewer's judgment of what a first user asks about; nobody outside has asked
  yet.

## Follow-on

- **Recorded.** The crate lives at `nifeos/byte_glob` with `publish = false` and no tag, and nife
  pins it by commit. The stranger test is a separate lane's. The module names `tests` and
  `verification` ship provisional.
- **Outstanding.** Every finding that Finding status above marks Waiting, each with what it waits
  on.

## Index row

An outside reading of `byte_glob` before it leaves. The crate name is `byte_glob` (Decision 1), the
README, the manifest fields and the split proof module are in, and the test that linked two staying
crates lives in `grant_plan`. `Dot` is `LeadingDot` and `cost_bound` is `step_bound` (Decision 2), and the architect cleared the review package and
ratified the seven names (Decision 3). The move is done: the crate lives at
`nifeos/byte_glob`, and nife pins it by commit. Finding status names what the later items wait on. The matcher itself checks out. Publishing
stays held.
