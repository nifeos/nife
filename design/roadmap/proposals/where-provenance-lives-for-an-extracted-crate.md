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

The lane for milestone 818 (the jh7110_entropy crate released on its own) raised this on
2026-10-31 (UTC). The consumer is every crate that leaves for its own repository after it, starting
with the next of the release-track set in milestone 857 (crates released on their own).

Reuse: none exists; the proposal is about where a record goes, and `script/names` and
`script/crate-standalone` already read the present home.

## Background

Until now a name's provenance sat in a `Name:` block beside the item in source: who ratified it,
when, and why. `script/crate-standalone` requires a block on every module and `pub` item before a
crate may leave, and requires each to read ratified. The shared template of milestone 887 (crate repositories share one CI template) does the
opposite once the crate is in its own repository: it refuses a `Name:` block, because a stranger's
repository carries no record of nife's rulings. So the lane for `jh7110_entropy` removed the blocks
from the crate's source in its move. The date and reason each name was ratified now live only in
git history and in the pull request that ratified them.

That leaves two questions with no owner. After a crate leaves, a reader of the crate cannot find
why a name is what it is. And the rule that a crate must carry blocks to leave, and carry none
once there, is stated in two places that do not mention each other.

## Recommendation

Keep provenance in nife, beside the milestone that moved the crate, and keep nothing in the crate.
The milestone's block gets one table: each public name, the date it was ratified and the pull
request. The crate's source and README carry no reference to nife. This keeps the template's
refusal true and gives the record one home that `script/names` can read. `script/crate-standalone`
then states that its blocks are removed in the move commit, not before.

## Prior art

Both entries are from memory and unchecked.

- Rust's own crates record naming reasons in RFCs and the issue tracker, not in the crate source.
- RustCrypto and the rust-embedded crates carry no record of who named a type; the reason is in the
  commit and pull request that introduced it.

## Where nife differs

nife ratifies names as a matter of rule (`design/naming.md`), so a name has an owner and a date
that a stranger's crate usually lacks. That record is worth keeping; it is only the place that is
in question.

## Alternatives

- Keep a `PROVENANCE.md` in the crate. This puts nife's rulings in a public crate repository and
  needs the template to allow it.
- Keep the blocks and change the template to allow them. A published crate then carries references
  a stranger cannot follow.
- Keep nothing. History holds it, at the cost of the question above.
