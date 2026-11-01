# The contract crates leave in this order: the questions for the architect

[The proposal](../the-contract-crates-leave-in-this-order.md) holds the order and the policy. These
are the decisions the rulings do not settle. Each goes to the architect alone, in this order, and
the next waits for the answer to the one before. Each opens with its background, then the
recommendation, then the options.

## 1. Does the first cut wait for the crates it links, and leave one package at a time?

The first cut moves three program packages (`disk-tools`, `process-tools`, `core-tools`) into
repositories of their own. Their 15 programs link 22 library crates that stay in nife today. The
direction of 2026-11-01 (UTC) says nothing that leaves may reference back into nife, so a program
cannot leave while a crate it links is still here.

The packages do not need the same crates. Measured on 2026-11-01, `disk-tools` reaches 12 of the 22,
`process-tools` 13 and `core-tools` 15. Only `disk-tools` avoids `grant_plan`, the crate with the
most churn and the most crossings of all 22. The version-declaring note that pull request #3022 adds
pulls `grant_plan` into every program, because the note's type lives there.

Recommendation: (a). The cut waits for its crates and leaves package by package, `disk-tools`
first, then `process-tools`, then `core-tools`. The note's type moves to a leaf crate of its own, so
`disk-tools` need not wait for `grant_plan`.

- (a) Recommended, as above. `disk-tools` becomes the first consumer outside nife, of the
  `block_roster` pilot.
- (b) The three packages leave together once all 18 crates they reach with the note have left. One
  event, but the latest possible.
- (c) The packages leave now and pin nife for the crates they link. Refused by the direction of
  2026-11-01: it is a reference back.

If the answer is no, milestone 756's single cut stands and waits for all 18 crates.

## 2. One repository per contract crate, or a few grouped repositories?

The split's rule for a program package is one repository per package (§316 (one repository per
package leaves the tree)). For a library crate released on its own it is one repository per crate,
on a shared workflow template (milestone 857 (crates released on their own, and proofs offered to
the projects that already exist)). The 22 contract crates belong to four packages: `contracts`
holds 17, `runtime` 2, `network` 2 and `fixtures` 1.

The measurement says grouping buys almost nothing. Of 348 focused commits in 30 days that touched
the 22, 244 also changed code outside them, and only 5 changed two of them while staying inside.
So the number of two-repository changes is about the same at either grain. What differs is
overhead: 22 repositories each carry the template, its rulesets and its CI, and each crate can carry
its own version.

Recommendation: (a), one repository per crate, named after the crate, on the crate template.

- (a) Recommended. Each contract versions on its own, which independent release (§151 (the goal of
  the repository split is independent release)) asks for, and which an alternative implementation
  needs.
- (b) One repository per package (`nifeos/contracts` and the rest). Fewer repositories; one version
  line for 17 unrelated contracts.
- (c) A repository per layer. Fuchsia tried layers and folded them back in 2019.

## 3. Does the architect hold on a semver break apply to contract crates?

The crate template labels a pull request `needs-architect` when `cargo-semver-checks` reports a break
against the last published version, and the hold keeps it red until a ruling. That rule was kept for
released crates. Contract crates break often: `grant_plan` changed 227 public items in four weeks,
and the direction of 2026-11-01 expects that to continue.

A hold on each break would put every such change in the architect's queue. The wire identity
(`INTERFACE_ID`, `VERSION`, a magic) is what other repositories and binaries act on; a Rust-API
break is caught by the compiler at every consumer's next pin bump.

Recommendation: (a). For contract crates, a Rust-API break must bump the Cargo minor, checked
mechanically with no hold, and a change to a wire identity value keeps the hold. This is the less
strict option, against the habit of choosing the stricter one on a close call. It is not close:
five identity changes in 90 days is a workable queue, and hundreds of API breaks a month is not.

- (a) Recommended, as above.
- (b) The hold applies to every break, as for released crates. The stricter option and the easier
  to relax later, at the cost of the architect's queue.
- (c) No hold at all; the version checks alone.

## 4. Does basalt refuse an image whose components pin different revisions of one contract?

Once a contract lives in its own repository, nife and a program repository each pin it by commit.
They can pin different commits. basalt could build the image as each component pinned it, could
override every pin with its own, or could refuse the mismatch.

Recommendation: (a), refuse. The image is then a set in which every pair agrees by construction,
and the pull request that moves one contract moves every pin of it.

- (a) Recommended: refuse, and the bump moves all pins together.
- (b) Override every component's pin with basalt's. One revision per image, but a combination no
  component's own CI ever built.
- (c) Build each as pinned. Two copies of a contract may then link into one image, and a shared
  page's two sides may disagree.

## 5. May basalt run a trial on commits that have not merged?

The workspace rule says a pin never names an unmerged commit, and basalt refuses one. That is why a
breaking pair can be proven only after both halves merge. A trial run would be a manual workflow
that takes candidate commits, merged or not, builds and checks them as one image, and writes no pin
and no file.

Recommendation: (a), allow the trial, since it writes no pin. It proves a pair before either half
merges.

- (a) Recommended, as above.
- (b) No trial. The pair is proven only at the basalt bump, after both halves merge, as today.
