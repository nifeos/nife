# What was already on the record

<!-- writing-standards: exception. Marked 2026-10-25 (UTC) by lane crates-stand-alone. Reason: this text moved verbatim out of milestone 857's block, where the passive baseline held it, to keep that block under its word cap; rewording a record while moving it would change what it says. -->

An appendix of [milestone 857 (crates released on their
own)](../0857-crates-released-on-their-own.md): the earlier rulings the forks had to respect.

Milestone 39 (repository structure for a loosely-coupled OS) answered a narrower form of this on
2026-07-31. It found that `cargo publish -p` works from a workspace, so publishing needs no split. It
set a trigger: publish a crate only after it has an in-tree consumer, because the first caller finds
the API's shape wrong. And it framed publication as evidence for the verified-Rust claim, not as a
product line. Three things have moved since:

- calef now wants release as a goal. §151 (the goal of the repository split is independent release)
  already says a piece earns separation when it can ship on its own schedule.
- The trigger is met. Every one of the 30 crates classed A or B has at least one consumer in this
  tree, counted from the `Cargo.toml` files.
- §46 (thin primitives or whole subsystems) says take first outside the kernel and the proven
  crates. That rule cuts both ways here. Where a maintained crate exists, it argues
  for offering our proofs to it rather than publishing a rival.

And two records bound Fork 1. Under §235 (the OS is built and updated from packages, and its
source divides by purpose), milestone 610 (the interface's stability is measured weekly)'s
per-package co-change column informs how packages group and holds no move. Under §316 (a program
package leaves as one repository, with the crates only it uses), a crate gets a repository of its
own only when it would publish on its own merits or more than one repository consumes it; any
other crate leaves inside its package's repository.
