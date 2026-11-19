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

- P0-1, another crate holds the name on crates.io: Done,
  [#3578](https://github.com/nifeos/nife/pull/3578).
- P0-2, seven public names unratified: Ruled (Decision 2, maintainer, provisional, 2026-11-14 UTC).
  Not built: `pub enum Dot` (`src/lib.rs` line 42) is not yet `LeadingDot`, in the crate, `pgrep`,
  `grant_plan` and the doctests; ratification is under Open decisions.
- P0-3, the integration test links staying crates: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566). The test is in `crates/grant_plan/tests/`, and
  the manifest has no dev-dependency.
- P0-4, the shipped text cites the tree: Open. Eighteen citation lines remain in `src/`; the README
  takes on `notes/glob.md`, `docs/proofs.md` takes on the proofs note, and the move strips the rest.
- P0-5, the manifest is not standalone: Open. Every field is present, but the `publish = false`
  comment cites this tree's allowlist and there is no `repository` field.
- P0-6, no README: Done, [#3566](https://github.com/nifeos/nife/pull/3566), with both license texts.
- P1-1, `naive` fails the question gate: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-2, falsification patch paths: Open. The six patches diff `a/crates/byte_glob/src/lib.rs` and
  the six records name tree paths; the move rewrites them, regenerates the hunks and replays each.
- P1-3, mutation testing would reach the inline proof module: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566). The repository's `.cargo/mutants.toml` comes
  with the move.
- P1-4, no `forbid(unsafe_code)`: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-5, the `/` caveat is missing: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-6, the module doc opens for this tree's reader: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-7, the length parameters lack a unit: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-1, the doc overstates the cost bound: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-2, `.*` matches `.` and `..`: Open. The matcher's doc and README state it; no listing in this
  tree offers either name (RedoxFS stores neither, `disk_tree` refuses both), and the note saying
  so in `grant_plan` is not written.
- P2-3, `match_steps` returns a tuple: Open. The doc states the promise; a named return
  (`Outcome`) or a recorded keep is two-way before release, so the maintainer rules provisionally.
- P2-4, no fuzz target: Open. A target with the naive matcher as oracle, under the opt-in rule for
  crates with no `unsafe`.
- P2-5, harnesses fix one dot rule: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-6, stale solver times: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-7, `Elem` is an abbreviation: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-8, no benchmark: Open. A `benches/` with the three adversarial shapes and one ordinary
  pattern.
- Decision 1, the registry name: Done, [#3578](https://github.com/nifeos/nife/pull/3578).
- Decision 2, the seven names: Ruled (maintainer, provisional, 2026-11-14 UTC). Not built: P0-2's
  rename.
- R1, the review package: Open. `notes/crate-reviews/` has no package for this crate; it follows
  P0-2's rename.
- R2, ratify the seven names and clear `nifeos/byte_glob`: Open. Under Open decisions.

## Open decisions

### Ratify `byte_glob`'s seven names and create `nifeos/byte_glob`

Background. `byte_glob` is the tree's glob matcher: it answers whether a pattern such as `*.txt`
matches one file name, over bytes, with a proved cost bound. The shell's expansion and `pgrep` call
it, and milestone 1017 moves it to its own repository at step 5. The move strips every `Name:`
block, so its seven public names (`matches`, `matches_with`, `has_magic`, `literal`,
`match_steps`, `cost_bound` and the dot-rule enum) leave the tree as whatever they are on that day,
ratified or not. A repository in the organization exists only once the architect clears its
review, and the clearance is the go to create it. The maintainer ruled the names provisionally on
2026-11-14 UTC: six as they are, and `Dot` becomes `LeadingDot` with no alias.

Recommendation: A.

- A. Clear the review once P0-2's rename and R1's package have landed: ratify the seven names with
  `LeadingDot`, and create `nifeos/byte_glob`. No tag and no publish follow from it.
- B. Clear now on the names as built, `Dot` included, and drop the rename. The enum's name then
  does not say which dot the policy is about.
- C. Hold the clearance until 1017 reaches step 5, and ratify then. Waiting loses nothing except
  that the package goes stale against the code.

Door: one-way. Once the repository exists and a consumer pins it, a name change is a breaking
change across repositories, and the repository's name is a public fact.

Prior art: fnmatch spells the policy `FNM_PERIOD`, rust-lang/glob `require_literal_leading_dot`,
bash `dotglob` and glibc's `glob(3)` `GLOB_PERIOD`; each names the position, as `LeadingDot` does.
This tree cleared `generational_table` and `block_roster_contract` the same way, a review package
first and the repository after, with no tag and no publish. A concrete example: with A,
`byte_glob::matches_with(b"*", b".config", LeadingDot::Ordinary)` is `true` and reads without the
doc, where `Dot::Ordinary` leaves the reader asking which dot.

Blocks: the move.

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

Decision 1 carries the architect's ruling and Decision 2 the maintainer's provisional one. Ratifying
the names and clearing the repository are under Open decisions. The baseline (scope, the default dot
rule, the algorithm) stays closed.

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
| `matches(pattern, name)` | fn, bool | a third-person verb phrase, as the question rule asks | ratify |
| `matches_with(pattern, name, dot)` | fn, bool | the same, `_with` for the policy argument, as std's `_with` | ratify |
| `has_magic(pattern)` | fn, bool | `has_` before a thing held; "magic" is the word Python's `glob` module and every shell manual use for an unquoted wildcard | ratify |
| `literal(pattern, out)` | fn, Option | a noun for the thing returned, the pattern's one literal name | ratify |
| `match_steps(pattern, name, dot)` | fn, (bool, usize) | the steps of a match; P2-3 may give it a named return later | ratify |
| `cost_bound(pattern_len, name_len)` | const fn | a bound on the cost; the parameters are P1-7 | ratify |
| `Dot { Special, Ordinary }` | enum | "dot" alone does not say which dot; the policy is about the first byte | `LeadingDot` |

`LeadingDot::Special` and `LeadingDot::Ordinary` read at a call site without the doc:
`matches_with(pattern, name, LeadingDot::Ordinary)`. Prior art spells the same policy
`FNM_PERIOD` (fnmatch), `require_literal_leading_dot` (rust-lang/glob), `dotglob` (bash) and
`GLOB_PERIOD` (glibc's `glob(3)`); every one of them names the position. The variants stay: they are
the two answers. The three call sites in the tree change with it
(`pgrep/src/lib.rs` lines 94 to 102, two mentions, and the doc tests).

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The six keep their spelling, and `Dot` becomes `LeadingDot` with
its variants unchanged and no alias. Ratifying them is the architect's, under Open decisions. P0-2
is the work.

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

- **Outstanding.** Every finding that Finding status above marks Open or Ruled, each with what
  remains, and each open decision under Open decisions. A lane checked each finding line against
  the crate and its pull requests on main.

## Index row

An outside reading of `byte_glob` before it leaves. The crate name is `byte_glob` (Decision 1), the
README, the manifest fields and the split proof module are in, and the test that linked two staying
crates lives in `grant_plan`. Before the move: `Dot` becomes `LeadingDot` (Decision 2), the shipped
text loses its citations of this tree, the manifest names its repository, the review package is
written, and the architect ratifies the seven names and clears `nifeos/byte_glob`. The falsification
paths are rewritten at the move; four P2 items follow. The matcher itself checks out. Publishing
stays held.
