# The contract crates leave in this order: the questions for the architect

[The proposal](../the-contract-crates-leave-in-this-order.md) holds the order and the policy. These
were the decisions the earlier rulings did not settle. Each went to the architect alone, in this
order. The architect ruled all five as recommended on 2026-11-01 (UTC); each section ends with its ruling.
Each opens with its background, then the recommendation, then the options.

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

Ruled 2026-11-01 (UTC): (a). Milestone 756 (`process-tools`, `core-tools` and `disk-tools` are the
first code to leave this repository)'s single cut gives way to package-by-package departure, each
package leaving as soon as its own crates have left, in the order `disk-tools`, `process-tools`,
`core-tools`. The `INTERFACES` note type moves into its own small leaf crate, and `block_roster`
stays the first consumed crate. 

This lane read `helpers/interface_declarations_check.py` against the move and did not run it.
The lint stays correct but fails until two things move with the crate. The new crate is linked and
in none of the check's three tables, so it needs an `INTERFACE_ID` or an `OWED` line. And
`named_crates` and the selftest hard-code `capability_manifest_note::` as the macro's crate, so a
program naming the macro by the new path would read as linking the old crate. Slots such as
`DOMAIN_SLOT` stay in `grant_plan`.

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

Ruled 2026-11-01 (UTC): (a), 22 repositories. A crate's repository appears when the crate moves and only once the
architect approves that exact repository; no lane creates one. The architect refused (b) and (c).

## 3. Does the architect hold on a semver break apply to contract crates?

The crate template labels a pull request `needs-architect` when `cargo-semver-checks` reports a break
against the last published version, and the hold keeps it red until a ruling. The architect kept that rule
for released crates. Contract crates break often: `grant_plan` changed 227 public items in four weeks,
and the direction of 2026-11-01 expects that to continue.

A hold on each break would put every such change in the architect's queue. The wire identity
(`INTERFACE_ID`, `VERSION`, a magic) is what other repositories and binaries act on; the compiler
catches a Rust-API break at every consumer's next pin bump.

Recommendation: (a). For contract crates, a Rust-API break must bump the Cargo minor, checked
mechanically with no hold, and a change to a wire identity value keeps the hold. This is the less
strict option, against the habit of choosing the stricter one on a close call. It is not close:
five identity changes in 90 days is a workable queue, and hundreds of API breaks a month is not.

- (a) Recommended, as above.
- (b) The hold applies to every break, as for released crates. The stricter option and the easier
  to relax later, at the cost of the architect's queue.
- (c) No hold at all; the version checks alone.

Ruled 2026-11-01 (UTC): (a). The hold on a `cargo-semver-checks` break applies to wire-identity
changes only (`INTERFACE_ID`, `VERSION`, a magic); a Rust-API break must bump the Cargo minor,
checked mechanically with no hold. Telling the two apart mechanically needs the 14 crates that lack
an `INTERFACE_ID` to get one first. The crate template change is a follow-on.

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

Ruled 2026-11-01 (UTC): (a). basalt refuses the image, and the bump pull request that moves a
contract moves every pin of it together. The architect refused (b) and (c).

## 5. May basalt run a trial on commits that have not merged?

The workspace rule says a pin never names an unmerged commit, and basalt refuses one. So basalt can
prove a breaking pair only after both halves merge. A trial run would be a manual workflow
that takes candidate commits, merged or not, builds and checks them as one image, and writes no pin
and no file.

Recommendation: (a), allow the trial, since it writes no pin. It proves a pair before either half
merges.

- (a) Recommended, as above.
- (b) No trial. basalt proves the pair only at its bump, after both halves merge, as today.

Ruled 2026-11-01 (UTC): (a). basalt may run a manual trial workflow on unmerged commits of
repositories in the organization, never on a fork's pull request and never on a self-hosted runner
for a fork. It writes no pin and no file. The pin rule stands: a pin never names an unmerged commit.

## 6. Where does `redoxfs` go?

Ruled 2026-11-01 (UTC), after the five above. The vendored engine (`vendor/redoxfs` with its pin
file, divergence patch series and upstream-status record), `redoxfs_server` and the host tool
`tools/redoxfs_host` leave as one repository holding all three. They leave after the contract crates
they link, and they consume those by a `rev` pin with the version stated, like every consumer. The
`script/vendor-verify` and `vendor-watch` checks move with them. Splitting the engine out later
stays possible. The architect refused the engine alone in its own repository, and refused staying
in nife until the end. Step 9 of the order places it.

Linked set, read from `cargo metadata` on 2026-11-01 (UTC): the server and the host tool reach 10 of
the 22 through normal dependencies (`address_space_map`, `capability_manifest_note`,
`domain_name_resolution_contract`, `domain_name_system`, `filesystem_contract`, `filesystem_fixture`,
`glob`, `globally_unique_identifier_partition_table`, `grant_plan`, `program_argument_contract`).
They also reach two crates outside the 22, `subtree_scope` and `memory_region_heap`, which leave
first (`memory_region_heap` only for the target build's `el0` feature, with `user_mode_runtime`
and `abi` behind it). `entropy_contract` is named in `mkfs.rs` prose and is not a dependency, so it
need not have left. Both manifests also patch `talc` to `vendor/talc`, a reference back into nife
that the ruling above does not place; the proposal's talc section records the maintainer's
provisional decision of 2026-11-01 (UTC). Unverified, not decided:

- Whether upstream `redoxfs` accepts AI submissions.

`talc`, decided by the maintainer on 2026-11-01 (UTC) and provisional (the proposal states it), with
the read-only checks of that day:

- Divergence: `vendor/talc` equals the crates.io 5.1.1 package (sha256 `53f8ad14fa1c`, upstream
  commit `15b2758a4538`) plus `vendor/talc.divergence.patch`, and differs in exactly two files.
  Two hunks in `src/base/mod.rs` rewrite the in-place grow fit check and the gap fit check
  (padding below an aligned base) so that wrapped arithmetic cannot pass, and
  `tests/wasm_top_of_memory.rs` is a new 75-line wasm regression test. The patch is
  SFBdragon/talc pull request 57, open, so no upstream revision carries it.
- Users: `allocation_cache`, `memory_region_heap` and `redoxfs_server` depend on `talc`. The root
  workspace, `jig`, `fuzz`, `redoxfs_server` and `tools/redoxfs_host` patch it to `vendor/talc`,
  and the `std-nife` heap overlay and the `xtask` farm generator name it. When `redoxfs` leaves,
  the other users stay on `vendor/talc` in nife until they leave.
- Upstream: no `CONTRIBUTING` file, and no README line about AI submissions or generated code, so
  the repository states no policy; issues and discussions are unverified. Its manifest requires
  `allocator-api2` 0.4, the newest, and `lock_api` 0.4, while 0.5.0 appeared on 2026-10-21, 11 days
  earlier. The newest release is 5.1.1 of 2026-09-09 and master holds one later documentation
  commit.
- Unverified: how the patch applies to a git dependency, which carries none. A rev that holds the
  fix, once pull request 57 lands, would remove the patch file.
- Whether the QEMU runners (254 path references in 64 files) can start the server from a pinned
  image and not from a path in the tree.

## 7. Do the contribute-disposition crates wait to move?

Ruled 2026-11-01 (UTC): no. The ruling of 2026-10-27 (contribute rows wait until a published crate
has users who ask upstream) covers the upstream offer only. The four contribute crates among the 22
(`globally_unique_identifier_partition_table` at step 2, `glob` and `domain_name_system` at step 5,
`calendar` at step 8) move in the order like the other 18. Each gets its own repository, tag and
version and a consumer pins it by `rev` with the version stated. Each waits for the architect's hand
review under amended §247 (the split begins with basalt holding nife, and `procps` moves first). Nothing is offered upstream, published or sent; that stays gated on the
2026-10-27 ruling, approval of each upstream submission and the AI-policy ruling (milestone 857,
fork 8). A crate later contributed has its repository as the source of the offer.
