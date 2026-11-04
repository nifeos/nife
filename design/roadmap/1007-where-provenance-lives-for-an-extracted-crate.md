---
status: BUILT
built: 2026-11-04
raised: 2026-10-31
promoted_from: where-provenance-lives-for-an-extracted-crate
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1007. Where the provenance of an extracted crate's names lives

The lane for milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own) raised this on 2026-10-31
(UTC). The consumer is every crate that leaves for its own repository after it, starting with the
next of the release-track set in milestone 857 (crates released on their own).

Reuse: none exists. This is about where a record goes, and `script/names` and
`script/crate-standalone` read the present home.

## Decision

Ruled 2026-11-04 (UTC). Once a crate leaves nife it is treated like a third-party crate. Every
public name is ratified before the move, so nife keeps no provenance record for it: no table in
nife, no `PROVENANCE.md` in the crate, and no `Name:` blocks kept. Git history and the pull
requests that ratified the names are the only record.

The move commit strips the `Name:` blocks from the crate, and nothing replaces them. The crate's
source and README carry no reference to nife, which keeps the refusal in the shared template of
milestone 887 (crate repositories share one CI template) true.
`script/crate-standalone` states this in its header.

## Background

Until a crate leaves, a name's provenance sits in a `Name:` block beside the item in source: who
ratified it, when, and why. `script/crate-standalone` requires a block on every module and `pub`
item before a crate may leave, and requires each to read ratified. The template of milestone 887
refuses a `Name:` block once the crate lives in its own repository, because a stranger's repository
carries no record of nife's rulings.

## Prior art

Both entries come from memory and nobody checked them.

- Rust's own crates record naming reasons in RFCs and the issue tracker, not in the crate source.
- RustCrypto and the rust-embedded crates carry no record of who named a type. The reason sits in
  the commit and pull request that introduced it.

## Where nife differs

nife ratifies names as a matter of rule (`design/naming.md`), so every name has an owner and a date
that a stranger's crate usually lacks. That ratification happens before the move, in a pull request,
and that pull request is the record.

## Alternatives not taken

- A `PROVENANCE.md` in the crate would put nife's rulings in a public crate repository and need the
  template to allow it.
- Keeping the blocks would leave references a stranger cannot follow.
- A table in the milestone that moves the crate would keep a second record of what the ratifying
  pull request already holds.

## Follow-on

- **None.**

## Index row

A crate that leaves for its own repository carries no naming record, and nife keeps none for it: git history and the ratifying pull requests are the only record.
