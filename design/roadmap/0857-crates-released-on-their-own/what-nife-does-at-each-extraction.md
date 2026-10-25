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
- The crate's name is ratified (Fork 5), and so is every name a stranger meets in it (ruled
  2026-10-25 UTC). The record is `<crate, hyphenated>-names.md` beside this file, and once it
  carries a dated `Ratified:` line, `script/names` reads the crate's names there and its `Name:`
  blocks leave its shipped files. `helpers/name_release.py` holds the format. Then
  `script/crate-standalone` exits 0.

## At each extraction

- The path dependency becomes a crates.io release, pinned with its checksum in `Cargo.lock`
  (Fork 2), and `crates/<name>` is deleted in the same pull request.
- The proofs, the falsification sweep, the fuzz targets and the coverage floor stop covering that
  code in nife. From then on Fork 4's release gate in the crate repository is relied on, and nife
  checks only that the version it pins was published by that gate.
- Falsification records that patch the crate move with it, rewritten from `-p3` to `-p1`.
- Citations into `crates/<name>/src` repoint to the crate repository at the pinned tag, by symbol.
- The dependency rule of the architect-label workflow fires once, on the first crates.io adoption,
  and is left to fire; that pull request is the ruling that nife depends on its own release.
- `cargo-deny` applies to the crate as to any dependency.
- The name leaves `script/names`, which tracks names in this tree; its record stays beside this
  file.

## What the metrics do

nife treats a released crate as an external dependency (calef's preference), so the tree's code
charts drop at each extraction. The drop is real and is not a regression. Two things keep it from
reading as one:

- `script/metrics` adds the milestone 623 (a bullet under the chart explains a cliff) week-note
  bullet itself when a `crates/<name>` directory
  disappears and `Cargo.lock` gains that name from crates.io in the same week.
- Prose that quotes tree totals says the figure counts the nife tree: `notes/how-this-is-built.md`,
  and the agent instructions' "two hundred thousand lines".
