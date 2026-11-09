# What nife does at each extraction

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md).
None of it is built yet. Each item is built with the first extraction, milestone 813 (a proven FAT crate,
released on its own), and every later extraction runs the same list.

## Before the first extraction

- The lint that fails a `[patch]` reaching `main` exists. Fork 2 ruled it; nothing builds it yet,
  and the first extraction is the first moment a `[patch]` can be committed by mistake.
- The tree-wide citation lint accepts a citation into a crate repository at a tag, by symbol, in
  the form §277 (cite a heading or a symbol, never a line number) allows. Until it does, repointing
  the citations below fails `script/lint`.
- Ratify the crate's name (Fork 5) and every name a stranger meets in it (ruled 2026-10-25 UTC). This tree keeps no record of a third-party crate's names (ruled
  2026-10-26 UTC), so the `Name:` blocks in the crate's source are the record until it breaks
  out. Ratification is a pull request: its body carries the table `python3 helpers/name_release.py
  --draft <crate>` prints, and it sets each block to `ratified <date> (#PR)`. Then
  `script/crate-standalone` exits 0, and the change that breaks the crate out strips the blocks
  from the outgoing copy.

## At each extraction

- The architect clears the crate: condition 4 of §247 (the split begins with basalt holding nife), the
  hand code review, passes. That clearance also approves creating the crate's repository under nifeos
  (the architect ruled it on 2026-11-01 UTC). No standing approval covers the order table, so a lane
  creates a crate repository only after the architect clears that crate.
- A person grants the crate's repository the organization secrets `NIFE_SMELTER_APP_ID` and
  the smelter App's private key beside it, using an `admin:org` token. A lane never takes this step.
- The path dependency becomes a git dependency, `{ version, git, rev }`, where `rev` is the commit
  of the crate's first tag and `version` is the version that tag's manifest carries, as the main
  page rules. `crates/<name>` is deleted in the same pull request. The line becomes a plain
  version requirement, with its checksum in `Cargo.lock` (Fork 2), only in a later pull request,
  after the crate's first publication.
- The proofs, the falsification sweep and the coverage floor stop covering that code in nife. From
  then on Fork 4's release gate in the crate repository is relied on, and nife checks only that the
  commit it pins carries a tag that gate made. Where the fuzz targets go is a
  [milestone 1043 (a moved crate fuzzed in its own repository)](../1043-a-moved-crate-is-fuzzed-in-its-own-repository.md).
- Falsification records that patch the crate move with it, rewritten from `-p3` to `-p1`.
- Citations into `crates/<name>/src` repoint to the crate repository at the pinned tag, by symbol.
- The dependency rule of the architect-label workflow fires on the pull request that adds the git
  dependency, and is left to fire; that pull request is the ruling that nife depends on the crate
  repository.
- `cargo-deny` applies to the crate as to any dependency.
- A person holding an `admin:org` token runs `script/grant-org-secrets <crate>`, which adds the new
  repository to the selected-repositories list of the two organization secrets with that
  visibility, `NIFE_SMELTER_PRIVATE_KEY` and `SETTINGS_TOKEN`, and checks that both lists name it.
  `NIFE_SMELTER_APP_ID` is open to every repository and needs no grant. Both secrets use the
  visibility "selected repositories", never "all repositories" (ruled 2026-11-01 UTC): the key
  bypasses the crate tag rulesets, so it stays out of nife and basalt, where lanes push branches.
  No agent handles the key value; the grant moves only the list. Do it before the first release
  tag job runs, since that job cannot authenticate without it.
- The name leaves `script/names`, which tracks names in this tree, and no record of it is kept.

## What the metrics do

nife treats a released crate as an external dependency (calef's preference), so the tree's code
charts drop at each extraction. The drop is real and is not a regression. Two things keep it from
reading as one:

- `script/metrics` adds the milestone 623 (a bullet under the chart explains a cliff) week-note
  bullet itself when a `crates/<name>` directory
  disappears and `Cargo.lock` gains that name from a `nifeos` git source in the same week.
- Prose that quotes tree totals says the figure counts the nife tree: `notes/how-this-is-built.md`,
  and the agent instructions' "two hundred thousand lines".
