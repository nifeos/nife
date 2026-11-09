---
status: NOT-STARTED
raised: 2026-11-09
milestone_dependencies: 1017, 1034
decision_dependencies: 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1059. `block_roster_contract`: the outside review's findings land before its first release

Raised by the lane `milestone/1059-block-roster-contract-outside-review`. A reviewer with no prior
exposure to this project read the crate's repository at `436d4ce` (`main`, after its pull request
#1), against [the review package](../../notes/crate-reviews/README.md) for `block_roster_contract`.
Pull request #2 of the repository then renamed the crate to `block_roster_contract` (`47ac5a9`,
now `main`), and this block cites files and symbols at that commit. This block proposes work and
changes nothing in that repository. Every item below is work for a lane there.

The identity stays as frozen: `INTERFACE_ID` 5, `VERSION` 1, magic `blkrosv1`. Nothing below asks
to change it. The names the review package ratifies, and the decisions it records, stay closed
here. This crate is the contract pilot of milestone 1017 (the contract crates leave in this order),
which owns the order of extraction and the reasons for it; milestone 1034 (a runbook for moving a
crate out of nife) owns the move itself. This block owns the findings that land after the move and
before a first release.

Reuse: the crate's own Kani harnesses and falsification patches, `cargo-mutants`, and the shared
crate workflows at a pinned revision do every check this block names. The one new file is a
`cargo-mutants` configuration of two lines, in the shape of the one this tree already carries. No
new tool.

## Goal

The crate is small, free of dependencies, safe Rust, and its three proofs hold. What stands between
it and a first release is housekeeping the shared workflows already measure: the weekly run is red
twice over, the template pin predates the mutation stack, and the shipped text still carries the
wording of the tree it left. The three API changes ruled below land before any tag, while they cost
two call sites and no version bump.

Publishing stays held. No crate from this organization publishes to crates.io until the
maintainers have extracted several crates and a review has decided whether any of them combine.
Nothing in this block lifts that hold, and no tag is ruled: a tag is a number others compare
against, so placing one is a separate question for the architect.

## What the review covered

The reviewer ran these firsthand at `436d4ce`, on a Linux host with stable Rust 1.99.1, Kani
0.68.0 (the template's `.kani-version`) and `cargo-mutants` 27.1.0:

| check | result |
|---|---|
| `cargo test` | 10 unit tests and 2 doctests pass; one doctest is the README example |
| `cargo build --lib` for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` | all three build |
| `cargo clippy --all-targets`; `cargo doc` with warnings as errors | no warning |
| `cargo kani` | 3 of 3 harnesses verify |
| the falsification patches, each applied to a copy | each turns its own harness red (3 of 3) |
| `cargo mutants` | 59 mutants: 34 caught, 21 missed, 4 unviable; the 21 are one in `capacity_of` and 20 inside the `cfg(kani)` module |
| `cargo llvm-cov` | 98.5 percent of lines, 100 percent of functions |
| `cargo deny check`, `cargo machete`, `typos .` | clean (deny notes an unmatched `Zlib` allowance, a warning only) |
| `cargo package --list` | 38 files, 56.8 KiB, 19.8 KiB compressed |
| `cargo +1.85 test --ignore-rust-version` | passes |
| the weekly workflow on `main` | red: `stranger cadence` (no `Last run:` date) and `mutation` (survivors not exempted) |
| repository settings | private vulnerability reporting on; the `crate-maintainers` team and the `main` and `tags` rulesets present |

Rechecked at `47ac5a9` for this block: the manifest, the workflow pins, `.github/mutants-exempts`,
`docs/stranger-test.md`, the README and the symbols each item names in `src/lib.rs`. The rename
moved none of them. The latest weekly run is still the one on `436d4ce`; `check` and `proofs` pass
on `47ac5a9`.

Not run: the stranger test proper, on a machine with no target preinstalled (the targets were
installed here, which hides one README gap in P1-7). Also Miri (the weekly job runs it green),
`cargo-semver-checks` (no tag exists) and the template's workflows locally; the CI results above
come from the repository's runs. The reviewer pushed, tagged, branched and opened nothing.

Effort sizes: S is under an hour, M a few hours, L a day or more.

## Findings, prioritized

P0 blocks any release. P1 lands before the first release. P2 follows it, in
[the appendix](1059-block-roster-contract-outside-review/after-the-first-release.md). A door is
one-way where something outside the repository acts on it: a public signature, a name, the page
layout or the license. The maintainers decide every two-way item provisionally.

### P0: the two red weekly jobs

#### P0-1. The stranger test has never run

`docs/stranger-test.md` reads `Last run: none`. The release workflow at the pinned template refuses
a release until a run is on record, and the weekly `stranger cadence` job fails closed on the same
line. Change: a reader with no project context clones fresh and follows the README alone (P1-7
lists what it will trip on), fixes the README where it misled, and writes the date. Two-way. S.

#### P0-2. The weekly mutation gate fails on survivors nobody has triaged

Twenty of the 21 survivors sit inside `mod verification` in `src/lib.rs`, which compiles only under
`cfg(kani)`, so no host test can kill them; their verdict measures the harness, not the crate. The
twenty-first swaps `<` for `<=` in `capacity_of`, and it is equivalent: at `len == HEADER_BYTES`
both branches return 0. Change: a `.cargo/mutants.toml` with `exclude_re = ["verification::"]` and a
comment saying why, an entry for the `capacity_of` survivor in `.github/mutants-exempts` with that
reason, and the census comment carried there deleted, since the first weekly run has answered it.
Two-way. S. The template's scaffold ships no `cargo-mutants` configuration, so every crate with an
inline Kani module meets this on its first weekly run; that is work for the template, not here.

### P1: before the first release

#### P1-1. The CI wiring predates the template's mutation stack

Every caller pins `nifeos/crate-workflows` at `cd1b484`. That commit is not behind release `v7`, so
the lag check passes, but `main` of the template is 33 commits ahead and carries `mutation.yml`,
`check / mutation-diff` and the mutation requirement of the release gate. The repository has no
`.github/workflows/mutation.yml` caller. Change, once the template cuts its next release: bump
every pin, add the `mutation.yml` caller with the per-repository cron line the scaffold now has,
then apply the settings so `check / mutation-diff` becomes required only after a run of it exists.
A required check that never reports holds every pull request. Inferred, not verified: the callers
carry no `# vN` comment after the commit, and whether Dependabot opens a bump for a bare commit is
unconfirmed; the bump should add the comment. Two-way. S.

#### P1-2. `Roster::read` tells the three refusals apart

`Roster::read` returns `None` for a short page, a wrong magic and a count past the page. Those are
three different bugs (a mapping bug, a version or wiring mismatch, a writer bug), and the program
that reads the page reports "nobody wired me a roster" and "this machine has no disks"
differently on purpose. A reader cannot say which refusal it met without parsing the page itself.

Ruled 2026-11-09. `Roster::read` returns `Result<Roster, ReadError>`. `ReadError` has one variant
per refusal, `TooShort`, `NotARoster` and `CountBeyondPage`, and implements `Display` and
`core::error::Error`. The harnesses keep their shape, with `is_ok()` in place of `is_some()`. The
README's sentence on what `read` returns changes with it. The two call sites in this organization,
`disk_surveyor` and the disk system test, change one `match` or `expect` each. One-way (public
signature). S.

#### P1-3. The reader ignores the capacity word

`read` checks neither half of what `write` guarantees about bytes 12 to 16: that the count is at
most the capacity word, and that the capacity word is `capacity_of` the page, clamped. The review
package carries this as its first open question. A page whose count exceeds its own capacity word
came from a broken writer, and a page whose capacity word exceeds what the slice holds is a mapping
shorter than the page. Change: `read` refuses both, through a new `ReadError` variant or two (names
provisional, minted in the pull request), with a host test at each boundary, and the crate header
and the README say what the reader checks. The `read` harness stays total, and the
write-then-read harness still holds, since `write` satisfies both. Two-way: it changes a refusal
and no signature. S.

#### P1-4. `TooManyDevices` implements `Display` and `core::error::Error`

It derives `Debug` alone, so `?` into the error type of a caller is awkward. `core::error::Error`
is stable at the manifest's `rust-version`. Additive, two-way. S.

#### P1-5. Nothing enforces the absence of `unsafe`

The crate has no `unsafe` block, and the README and `SECURITY.md` lean on that, but the crate root
carries `#![no_std]` alone. Change: `#![forbid(unsafe_code)]`. The template's fuzz opt-in (P2-4)
reads the same attribute. Two-way. S.

#### P1-6. The two accepted name findings of the review package

Ruled 2026-11-09. The parameter of `Roster::get` becomes `index`, the word its doc comment already
uses, in place of `i`. The three transport constants, `TRANSPORT_VIRTIO_MEMORY_MAPPED_IO`,
`TRANSPORT_VIRTIO_PCI` and `TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS`, carry `#[doc(alias)]` for
`virtio-mmio`, `virtio-pci` and `nvme`, the spellings a reader searches for. These are aliases of
the outside terms, not of an earlier spelling of this crate. One-way (names). S.

#### P1-7. The shipped text, for a stranger

Each finding is in the file and symbol named:

- The doc of `write` says "The kernel is the only caller". The README says "one party", and a host
  test or an image builder is a caller too. Say "the writer".
- The doc of `MAGIC` carries a dated `# Amendments` entry and asks for one per break. History goes
  in `CHANGELOG.md`, whose `0.1.0` line should state the identity values and that the magic carries
  the version digits.
- The comment on the `publish = false` line of `Cargo.toml` names `crates/publish-allowlist.txt`, a
  path from another repository. Say why the crate is unpublished in words that hold here: the
  publishing hold above.
- The README's "Building and testing" section builds for a bare-metal target without saying that
  `rustup target add <target>` comes first. The stranger run of P0-1 will trip here.
- The README's layout table lists the capacity word, and nothing says the reader ignores it (or,
  after P1-3, checks it).
- The README says "The identity never changes" beside "A layout break bumps both". Say that the id
  never changes and the version changes only with the layout.
- How a program obtains the page, and to whom it declares `INTERFACE_ID`, lie outside the crate.
  One README sentence saying so stops a stranger looking for an API that is not here.

Two-way. S.

#### P1-8. `Device` sorts and hashes, and `devices()` knows its length

`Device` derives neither `Hash` nor `Ord`, so a listing cannot sort or deduplicate without a
wrapper. `Roster::devices` is a `filter_map` over `get`, which loses `ExactSizeIterator` though the
count is known, and `get` keeps `try_into().ok()?` paths the proofs show unreachable.

Ruled 2026-11-09. `Device` derives `Hash`, `PartialOrd` and `Ord`. `Roster::devices` becomes an
exact-size, infallible iterator built on `chunks_exact(ENTRY_BYTES).take(count)`, and the
unreachable `try_into().ok()?` paths in `get` go. One-way once published (public API). S. The order
a derived `Ord` sorts by is still open; see the decisions below.

## Decisions this block needs

Ruled 2026-11-09, and closed here:

1. `Roster::read` returns `Result<Roster, ReadError>` with the variants `TooShort`, `NotARoster`
   and `CountBeyondPage`; `ReadError` implements `Display` and `core::error::Error`; the harnesses
   use `is_ok()` (P1-2).
2. `Roster::get` takes `index`, and the three transport constants carry the aliases `virtio-mmio`,
   `virtio-pci` and `nvme` (P1-6).
3. `Device` derives `Hash`, `Ord` and `PartialOrd`, and `Roster::devices` is an exact-size,
   infallible iterator over `chunks_exact(ENTRY_BYTES).take(count)` (P1-8).
4. The crate is `block_roster_contract`, since [design/naming.md](../naming.md) ends a wire-contract
   crate in `_contract`.

For the architect, one-way, not yet ruled:

1. Which field a derived `Ord` on `Device` compares first (P1-8). A derive compares in declaration
   order, and the struct declares `ordinal` before `transport`, so as written a listing sorts by
   ordinal across buses. Recommendation: declare `transport` first, so a listing sorts by bus and
   then by ordinal, the order a person reads. The page layout does not fix the field order, since
   `write` and `get` name each field. Once published, the sort order is part of the API.
2. Any tag. Not ruled, and asked separately when the weekly gates pass.

The maintainers' provisional calls: every two-way item above and in the appendix, and the names of
the capacity-word refusals in P1-3, which go through the naming review before any tag.

The publishing hold stands.

## Prior art

How NVMe, virtio, Linux, UEFI and the Rust block-device crates answer "which block devices exist",
and what this crate takes or refuses from each, are in
[the prior art appendix](1059-block-roster-contract-outside-review/prior-art.md).

## Done when

The checks are in [the done-when appendix](1059-block-roster-contract-outside-review/done-when.md).
None needs hardware or QEMU.

## BUGS

- The review read the template's checks through the repository's CI results, not locally, so a
  check the template adds before a lane starts is missing from the list.
- P1-1 waits on a template release; until then the lag check passes at the current pin, and nothing
  in this repository can move it.
- P2-7 needs a template change first, so this block cannot close it alone.
- The P2 order is the judgment of the reviewer on what a user asks for first. Nobody outside has
  asked for anything yet.

## Index row

An outside reading of `block_roster_contract`: two red weekly jobs block the first release (no
stranger run; mutation survivors inside the Kani module). Eight items go before it: the CI pin and
the mutation caller, a `Result` from `Roster::read` (ruled), a checked capacity word, `Display` on
`TooManyDevices`, `forbid(unsafe_code)`, `index` and three aliases (ruled), the shipped text, and
`Hash` and `Ord` on `Device` with an exact-size `devices()` (ruled). Eight follow it. The sort order
of `Device` and any tag are open; publishing stays held.
