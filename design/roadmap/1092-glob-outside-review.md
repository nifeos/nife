---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1092. `byte_glob`: what an outside maintainer would fix before the crate leaves

Raised by a read-only outside review of the crate then at `crates/glob` (now `crates/byte_glob`) at `nifeos/nife@cf1328af6`, read the way a
maintainer meeting it for the first time would. The review changed nothing and opened nothing; every
item below is work for a lane. No review package exists for this crate yet, so the baseline the review
took as settled is the crate's own record: [notes/glob.md](../../notes/glob.md),
[notes/glob-scope.md](../../notes/glob-scope.md), [notes/glob/proofs.md](../../notes/glob/proofs.md)
and [notes/verification/glob-proofs.md](../../notes/verification/glob-proofs.md). The four scope
decisions those record are `**` out, qualifiers out, POSIX classes out and a leading dot special by
default. They stay closed here. So do the one-backtrack algorithm, the three-byte proof domain and
the ratified crate name `byte_glob`. The exception is where a later rule or a fact outside the tree bears
on them, and the text says so where it does. Milestone 1017 (the contract crates leave in this order) places the crate
at step 5 and owns the move; this block owns the findings that land inside that path.

Reuse: the crate's own tests and six harnesses, Kani 0.68.0 and the six falsification patches, the
shared crate workflows and their helpers (`standalone.py`, `question_shape.py`,
`readme_sections.py`, `falsify.py`), `script/crate-review-packet`, `helpers/name_release.py`,
cargo-mutants, cargo-fuzz. No new tool.

## Goal

A stranger who finds this crate in its own repository can tell in one screen what it matches, what it
refuses to match, and why `matches(b"*", b".config")` is false. They can build it, run its tests and
its proofs from the README alone, and apply one falsification patch. The template's `check` and
`proofs` workflows pass on the move commit for the crate's own reasons. The public names are final
before the move strips their provenance, and the crate has a name it can one day publish under.

Publishing stays held. Nothing here lifts the hold on crates.io publication, and a green gate is not
permission to publish.

## What the review covered

What the review covered is in [the appendix](1092-glob-outside-review/what-was-checked.md).

## Findings, prioritized

P0 blocks the move and the creation of the repository. P1 lands before the first tag in that
repository. P2 follows it. A door is one-way where someone outside the tree acts on the outcome: a
published name, the public API, a layout, a dependency. Everything else is two-way, and the
maintainer decides it.

### P0: the name, the names, and what does not stand alone

#### P0-1. The crate name was taken on crates.io

`glob` is rust-lang/glob, version 0.3.4, over 700 million downloads, the most widely used glob
crate in the ecosystem. The ratification of the crate name as a standard term, which it is, did not weigh the registry. The tree already knows the other crate: the survey behind
milestone 784 (reuse what the tree wrote) lists `glob` 0.3.4 as prior art for this very crate.

Why it matters now rather than at publication: the move creates the repository with the crate's name, consumers pin it by that name, and the template's release path reads the registry.
`helpers/publish_mode.py` decides the publishing door by whether a crate of that name exists on
crates.io. For `glob` it exists. Even a dry run would report trusted publishing to a crate this
organization does not own, and a real run would fail against it. A dependent that needs both this
crate and rust-lang's cannot name them both without a `package =` rename. The hold on publishing
makes this latent, not absent: a name chosen for a repository and then changed for publication is two
names for one thing.

Change: the crate is `byte_glob` in the tree, in the directory, the package, the library and every
caller, under the rename rules of `design/naming.md`. Decision 1 below records the ruling. Evidence:
the manifest's `name` line; the registry. One-way.
S for the rename, since the crate is small and has three dependents.

#### P0-2. Seven public names carry no provenance and are not ratified

`Dot` (and its variants `Special` and `Ordinary`), `matches`, `matches_with`, `has_magic`,
`literal`, `match_steps` and `cost_bound` have no `Name:` block; `helpers/name_release.py --draft
glob` reports "no block" for each, and `design/module-block-baseline.txt` lines 3146 to 3158 hold
them as baseline violators. The move strips `Name:` blocks, so whatever is not ratified in a review
package before the move leaves the tree as an unratified public API. The names themselves are mostly
right; Decision 2 below lists them with one recommended change. Evidence: `crates/byte_glob/src/lib.rs`
lines 33, 76, 91, 102, 118, 140 and 236. One-way (public API). S, once ruled.

#### P0-3. The integration test links two crates that do not leave with it

`tests/fixture_agreement.rs` dev-depends on `filesystem_contract` and `filesystem_fixture`
(`Cargo.toml` lines 22 to 24), both sibling crates by path. The rule in milestone 1017 is that a crate leaves
when every crate it links, dev-dependencies included, has left or leaves with it; `standalone.py`
and the review packet both flag the two paths. The test checks that nife's staged fixtures equal the expansion of the patterns they stand for, which is a fact about the fixtures, not about the
matcher. Its own header says it lives here only because the fixture crate does not depend on the
matcher; a dev-dependency the other way creates no cycle and is the ordinary shape.

Change: move the file to `crates/filesystem_fixture/tests/` (or `crates/grant_plan/tests/`, which
already depends on `byte_glob`) with `byte_glob` as a dev-dependency, and delete the two dev-dependencies here.
Two-way. S.

#### P0-4. The shipped text cites the tree it is leaving

`standalone.py` counts 18 lines with a path into the nife tree, 3 section citations, 2 milestone
citations, 1 provenance block and 1 architect's name. They sit in `Cargo.toml` (lines 7 to 13) and
`src/lib.rs` (lines 4 to 5, 8, 24, 101, 155, 235, 252, 614, 627, 678, 736, 758, 802, 879, and the
six `Falsification:` records). `notes/glob.md`, `notes/glob-scope.md`, `notes/glob/proofs.md` and
`notes/undefined-behavior.md` appear as citations where a stranger will have none of them. The template's
`check / standalone` job fails on every one.

Change: the README carries what `notes/glob.md` carries today (the surface, the four scope
decisions, the algorithm and the corners table), `docs/proofs.md` in the repository carries
`notes/glob/proofs.md`, and each citation in the source becomes a sentence or a link into those. The
`Name:` block and the architect's name leave with the move. `notes/glob-grant.md` stays in nife; it
is about the layer above. Two-way. M.

#### P0-5. The manifest is not a standalone manifest

`Cargo.toml` inherits `license` and `lints` from the workspace, has no `rust-version`, no
`description`, `repository`, `readme`, `keywords`, `categories` or `include`, and the comment on
`publish = false` sends the reader to `crates/publish-allowlist.txt`, a file the repository will not
have. `standalone.py` fails on the inheritance and the missing `rust-version`; the weekly `msrv` job
has nothing to build without it. Compare `nifeos/generational_table`'s manifest, which has every
field.

Change: add `license = "MIT OR Apache-2.0"` and `rust-version` (edition 2024 needs 1.85; the review
index uses 1.88 across the first crates). Add `[lints.rust]` with `unexpected_cfgs` for `cfg(kani)`,
`unsafe_op_in_unsafe_fn` and `missing_docs`, and the clippy lints the other crates carry. Add the
metadata fields and an `include` list. Add a `publish = false` comment in the crate's own words (the
hold). Two-way; a ruling covers the publish flag itself and it stays. S.

#### P0-6. There is no README

The crate has no README, no `LICENSE-*` files and no changelog. The scaffold supplies the license
files and the changelog; the README is the review's concern. `readme_sections.py` requires `# glob`,
a summary paragraph, `## EXAMPLES`, `## What is proven`, `## BUGS`, `## Minimum Rust version` and
`## License`, in that order. `standalone.py` fails without `## EXAMPLES` and `## BUGS`. The summary
must say in its first lines the three things a stranger will not guess. The first is bytes rather
than characters. The second is one name rather than a path (nothing treats `/` as special, so a `*`
handed a path crosses separators). The third is that a leading `.` needs a literal `.` by default. The corners table from
`notes/glob.md` is the crate's own section. Two-way. M, with P0-4.

### P1: before the first tag

#### P1-1. One test function fails the template's question gate

`fn naive(pattern, name, j, i) -> bool` (`src/lib.rs` line 580) returns a `bool` and is not named like a
question. `check.yml` runs `question_shape` at a ceiling of 0, so the first pull request in the new
repository is red. Change: `naive_matches` or `exhaustive_matches`. Two-way. S.

#### P1-2. The falsification patches and their records are written for the tree's paths

All six patches diff `a/crates/byte_glob/src/lib.rs`, and the six `Falsification: replayable` records
name `crates/byte_glob/falsifications/<name>.patch`. The template's `falsify.py` accepts only
`falsifications/<module.path>.<harness>.patch` (its `CONVENTIONAL` regex) and applies with `git apply
-p1` at the crate root, so each would fail as "no longer applies", which the gate counts as a failure
and never a skip. The hunk headers are also about 150 lines stale; they apply at an offset today
and a rewrite is the moment to regenerate them. Whether the move lane's tooling already rewrites
paths stays unverified. Change: rewrite the six paths and the six records at the move, regenerate the
hunks, and replay each once. Two-way. S.

#### P1-3. The inline proof module will be mutated in the new repository

`#[cfg(kani)] mod verification` is inline in `src/lib.rs` (line 881). In nife, `.cargo/mutants.toml`
excludes `verification::` by regex; that file does not travel. cargo-mutants in the repository will
mutate harness bodies that no host test can reach, and the weekly mutation gate will fail on them, as
it did for `e1000e`. Change: split the file as the naming page prescribes for `file_allocation_table`
(`src/tests.rs` for the host tests, `src/verification.rs` for the proofs, `src/lib.rs` for the
matcher) and exclude `src/verification.rs` by glob in the repository's `.cargo/mutants.toml`, as
`generational_table` does. The split also takes the 1,047-line file to about 370 lines of shipped
code. Two-way. S.

#### P1-4. The crate does not forbid `unsafe`

`src/lib.rs` has no `unsafe` and does not say so. `#![forbid(unsafe_code)]` makes the claim one the
compiler keeps, and it is the shortcut the template's fuzz opt-in scan accepts. Change: add it under
`#![no_std]` (line 26). Two-way. S.

#### P1-5. The one caveat the record says is in the crate docs is not there

`notes/glob.md` and `notes/glob-scope.md` both say that the matcher's blindness to `/` is "written
down instead, in the crate docs where a reader meets the function". `src/lib.rs` never mentions a
separator or a path. A caller who passes `matches(b"*.txt", b"dir/notes.txt")` gets `true` and
nothing warned them. Change: one sentence in the module doc and one in `matches`, and the README
line in P0-6. Two-way. S.

#### P1-6. The module doc opens for nife's reader, not the crate's

The first paragraph of `src/lib.rs` explains that one matcher "serves both the shell's expansion and
its grant" and that the authors wrote it "because it is on the verification path (§46 (thin primitives or whole subsystems; we write everything in between) ...)". A stranger
needs the first paragraph to say what the crate is: an `fnmatch(3)`-style matcher over bytes for one
file name, with no allocation, no `unsafe`, a cost bound computable in advance, and six Kani proofs.
The grant argument belongs in the README's "How it differs" section, in the crate's own words.
Change: rewrite the two opening paragraphs; keep the syntax table. Two-way, with P0-4. S.

#### P1-7. The names of the two length parameters do not carry their unit

`cost_bound(pattern_len: usize, name_len: usize)` (`src/lib.rs` line 236). The naming page says a length
carries its unit (`MAXIMUM_NAME_BYTES`, never `MAXIMUM_NAME_LEN`) and that the rules apply to
parameters; the rules accept `len` only as the standard library's method spelling, which is why
`spelled_out` passes it. Change: `pattern_bytes` and `name_bytes`. A parameter name is not part of the
API's compatibility surface, so this is two-way, and it sits here rather than under P2 only
because it is the one naming item the gates do not catch. S.

### P2: later

The P2 items are in [the appendix](1092-glob-outside-review/later-items.md).

## Decisions for the architect

Only one-way doors. The scope decisions, the default dot rule and the algorithm are the baseline and
are not reopened.

### Decision 1. The registry name of the crate (P0-1)

Ruled 2026-11-14 (UTC): the crate is named `byte_glob`, in code and package alike. `crates/glob` is
`crates/byte_glob`, the package and the library are `byte_glob`, and every caller says
`byte_glob::matches`. The old name carries no `doc(alias)`. The `Name:` block is ratified for
`byte_glob`. The reasoning that led here follows.

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

Recommendation: ratify six as they are, and rename `Dot` to `LeadingDot`.

Background. The move strips provenance, so these gain ratification in the review package or never. The
names, with the reviewer's reading against `design/naming.md`:

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
the two answers and nothing shorter says them. The three call sites in the tree change with it
(`pgrep/src/lib.rs` lines 94 to 102, two mentions, and the doc tests).

## Prior art

The comparison with nearby work is in [the appendix](1092-glob-outside-review/prior-art.md).

## Done when

The checks are in [the appendix](1092-glob-outside-review/done-when.md).

## BUGS

- The review ran two of the six harnesses and replayed one of the six patches. The rest rests on
  the crate's record and the tree's own gates.
- The reviewer did not measure whether the move lane's tooling rewrites patch paths (P1-2); if it
  does, that item is a check rather than work.
- Whether any enumeration in nife yields `.` or `..` stays unverified (P2-2).
- The P2 order is the reviewer's judgment of what a first user asks about; nobody outside has asked
  yet.

## Index row

An outside reading of `byte_glob` before it leaves. Six items block the move. Another crate holds the crate name on crates.io (Decision 1 rules it). Seven public names lack ratification (Decision 2). A test links two
staying crates. Shipped text carries nife citations. The manifest inherits from the workspace. There
is no README. Seven go before the first tag: a test name the question gate refuses, patch paths, the
inline proof module, `forbid(unsafe_code)`, the missing `/` caveat, the module doc's opening, and
two parameter names. Eight follow. The matcher itself checks out: tests, clippy, docs,
three bare-metal builds and the two harnesses run are clean, every patch applies, and the recorded mutation score is complete.
Publishing stays held.
