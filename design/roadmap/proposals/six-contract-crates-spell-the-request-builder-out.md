---
status: PROPOSED
raised: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# Six crates spell their request builder `req`

Raised by lane/record-open-follow-ups. Pull request #3480 (`entropy_contract` ready for review) renamed its own `req` to `request` and named these six in its "Left for other lanes" section.

Reuse: `entropy_contract`'s `request`, as renamed by #3480, and the crate review's rename pass. No new tool.

Name: provisional. `request` is the spelling `entropy_contract` took on 2026-11-11 (UTC), as a maintainer's provisional name.

## The problem

`req` abbreviates "request". Six crates still define a `req` function that packs an operation and its operands into the request word. They are `byte_sink_contract` (`src/lib.rs`), `clock_contract` (`src/lib.rs`, an associated function), `credential_contract` (`src/lib.rs`), `filesystem_contract` (a `req` in each of `src/lib.rs`, `src/fs.rs` and `src/blk.rs`), `graphics_contract` (`src/lib.rs`) and `line_editor` (`src/proto.rs`). design/naming.md says a name this tree authors spells its words out. The renames are two-way doors, since the crates are unpublished and only this tree links them.

## The work

Each crate renames `req` at its own extraction review, in the review's rename pass. The callers and a `Name:` block change in the same commit, with no `doc(alias)` of the old spelling. No lane renames them ahead of the review. The `names` check of `script/crate-standalone <crate>` passes once the rename is done.

## For the maintainer

Roadmap blocks 0372 (an entropy device that answers with zeros) and 0818 (the `jh7110_entropy` crate released on its own) still name `bringup_failure` and `entropy_contract::want`. Pull request #3480 renamed them to `bring_up_failure` and `requested_bytes`. A developer lane does not edit `design/`, so those two words wait for the maintainer.
