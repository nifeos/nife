---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A producer's tagged release moves each consumer's pinned revision

*The title and slug are provisional.*

Raised by the lane `lane/unpublished-crates-pin-by-revision-and-version` on 2026-11-01 (UTC),
recording the ruling of that day on how nife consumes a crate it does not publish. Journey 5 states
the form: a git dependency on `https://github.com/nifeos/<crate>` whose `rev` names the commit of
a tagged release, with the version requirement stated. §305 (git dependencies on `nifeos/*` only, pinned to a landed commit)
governs the pin. Nothing moves that pin today when a producer tags a new release, so every consumer
falls behind until someone edits it by hand.

Reuse: the tree's dependency bot (`.github/dependabot.yml`, three `cargo` entries) and the shared
crate template in `nifeos/crate-workflows`. It proposes no new dependency.

## The question

Can the dependency bot move a git `rev` to the commit of a producer's newest tagged release? Nobody
has measured it. The measurement is one producer, one tag and one consumer, with the bot's own run
log as the evidence.

- If it can, the work is its configuration, and a check that the new revision is a tagged
  release's commit whose manifest carries the version the consumer states.
- If it cannot, the producer's release workflow opens a bump pull request in each consumer, citing
  `nifeos/<crate>@<sha>` as §305 asks. The list of consumers has to come from somewhere a
  workflow can read, and choosing that place is part of the work.

## An option, priced and not recommended

A static-file registry: a sparse index that a web server hands out as plain files, which Cargo
reads through `registry = "<name>"` on the dependency and an index that `.cargo/config.toml`
names. A version requirement would then resolve without a git pin. What it would take is unmeasured: where the
index and the archives live, who signs them, how `deny.toml`'s `allow-registry` admits it, and
what a crate does about a dependency on another registry once it publishes to crates.io. This file
recommends nothing about it.

## BUGS

- This lane ran nothing. The bot's behavior on git dependencies is the open measurement.
