# P0: blocks the move

An appendix to [the milestone](../1092-glob-outside-review.md).

## P0: the name, the names, and what does not stand alone

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
caller, under the rename rules of `design/naming.md`. Decision 1 in the milestone records the ruling. Evidence:
the manifest's `name` line; the registry. One-way.
S for the rename, since the crate is small and has three dependents.

#### P0-2. Seven public names carry no provenance and are not ratified

`LeadingDot` (and its variants `Special` and `Ordinary`), `matches`, `matches_with`, `has_magic`,
`literal`, `match_steps` and `cost_bound` have no `Name:` block; `helpers/name_release.py --draft
byte_glob` reports "no block" for each, and `design/module-block-baseline.txt` lines 3146 to 3158 hold
them as baseline violators. The move strips `Name:` blocks, so whatever is not ratified in a review
package before the move leaves the tree as an unratified public API. The names themselves are mostly
right; Decision 2 in the milestone decides them, provisionally, with one change. Evidence:
`crates/byte_glob/src/lib.rs` lines 33, 76, 91, 102, 118, 140 and 236. Two-way before the first tag. S.

#### P0-3. The integration test links two crates that do not leave with it

`tests/fixture_agreement.rs` dev-depends on `filesystem_contract` and `filesystem_fixture`
(`Cargo.toml` lines 22 to 24), both sibling crates by path. The rule in milestone 1017 is that a crate leaves
when every crate it links, dev-dependencies included, has left or leaves with it; `standalone.py`
and the review packet both flag the two paths. The test checks that nife's staged fixtures equal the expansion of the patterns they stand for, which is a fact about the fixtures, not about the
matcher. Its own header says it lives here only because the fixture crate does not depend on the
matcher; a dev-dependency the other way creates no cycle and is the ordinary shape.

Change: move the file to `crates/grant_plan/tests/`, which already depends on `byte_glob` and leaves
after it, and delete the two dev-dependencies here. `filesystem_fixture` must not take it, since
that crate leaves first, as item 2 of milestone 1091 (`filesystem_fixture`: what an outside
maintainer would fix before it leaves) states. Two-way. S.

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
files and the changelog; the README is the review's concern. `readme_sections.py` requires `# byte_glob`,
a summary paragraph, `## EXAMPLES`, `## What is proven`, `## BUGS`, `## Minimum Rust version` and
`## License`, in that order. `standalone.py` fails without `## EXAMPLES` and `## BUGS`. The summary
must say in its first lines the three things a stranger will not guess. The first is bytes rather
than characters. The second is one name rather than a path (nothing treats `/` as special, so a `*`
handed a path crosses separators). The third is that a leading `.` needs a literal `.` by default. The corners table from
`notes/glob.md` is the crate's own section. Two-way. M, with P0-4.
