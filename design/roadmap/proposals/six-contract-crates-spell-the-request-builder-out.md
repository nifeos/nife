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

Raised by lane/record-open-follow-ups, from the "Left for other lanes" section of pull request #3480 (`entropy_contract` ready for review), which renamed its own `req` to `request` and named these six.

Name: provisional. `request` is the spelling `entropy_contract` took on 2026-11-11 (UTC), as a maintainer's provisional name.

## The problem

`req` abbreviates "request". Six crates still define a `req` function that packs an operation and its operands into the request word: `byte_sink_contract` (`src/lib.rs`), `clock_contract` (`src/lib.rs`, an associated function), `credential_contract` (`src/lib.rs`), `filesystem_contract` (a `req` in each of `src/lib.rs`, `src/fs.rs` and `src/blk.rs`), `graphics_contract` (`src/lib.rs`) and `line_editor` (`src/proto.rs`). design/naming.md says a name this tree authors spells its words out. The names are two-way doors because the crates are unpublished and only this tree links them.

## The work

Each crate renames `req` at its own extraction review, in the review's rename pass, with the callers and a `Name:` block in the same commit and no `doc(alias)` of the old spelling. No lane renames them ahead of the review. `script/crate-standalone <crate>` `names` check passes once it is done.

## For the maintainer

Roadmap blocks 0372 (an entropy device that answers with zeros) and 0818 (the `jh7110_entropy` crate released on its own) still name `bringup_failure` and `entropy_contract::want`, which #3480 renamed to `bring_up_failure` and `requested_bytes`. A developer lane does not edit `design/`, so those two words wait for the maintainer.
