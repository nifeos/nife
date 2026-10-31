---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Where the provenance of an extracted crate's names lives

The lane for milestone 818 (the jh7110_entropy crate released on its own) raised this on 2026-10-31
(UTC). The consumer is every crate that leaves for its own repository after it, starting with the
next of the release-track set in milestone 857 (crates released on their own).

Reuse: none exists. This is about where a record goes, and `script/names` and
`script/crate-standalone` already read the present home.

## Background

Until now a name's provenance sat in a `Name:` block beside the item in source: who ratified it,
when, and why. `script/crate-standalone` requires a block on every module and `pub` item before a
crate may leave, and requires each to read ratified. The shared template of milestone 887 (crate
repositories share one CI template) does the opposite once the crate lives in its own repository.
Its check refuses a `Name:` block, because a stranger's repository carries no record of nife's
rulings.

So the lane for `jh7110_entropy` stripped the blocks from the crate's source in its move. The date
and reason for each name now live only in git history and in the pull request that ratified them.

That leaves two questions with no owner. After a crate leaves, a reader cannot find why a name is
what it is. And two places state the rule (blocks before leaving, none after) without mentioning
each other.

## Recommendation

Keep provenance in nife, beside the milestone that moves the crate, and keep nothing in the crate.
The milestone's block gets one table: each public name, the date of its ratification and the pull
request. The crate's source and README carry no reference to nife. This keeps the template's
refusal true and gives the record one home that `script/names` can read. `script/crate-standalone`
then says that the move commit strips the blocks, not an earlier one.

## Prior art

Both entries come from memory and nobody checked them.

- Rust's own crates record naming reasons in RFCs and the issue tracker, not in the crate source.
- RustCrypto and the rust-embedded crates carry no record of who named a type. The reason sits in
  the commit and pull request that introduced it.

## Where nife differs

nife ratifies names as a matter of rule (`design/naming.md`), so every name has an owner and a date
that a stranger's crate usually lacks. That record has value. Only its place is in question.

## Alternatives

- Keep a `PROVENANCE.md` in the crate. This puts nife's rulings in a public crate repository and
  needs the template to allow it.
- Keep the blocks and change the template to allow them. A published crate then carries references
  a stranger cannot follow.
- Keep nothing. History holds it, at the cost of the question above.
