---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1073. `counter_frequency_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/counter_frequency_contract` as a stranger to this tree
would meet it. The review read the crate's five tracked files, every caller in the tree, the review
package, the naming rules and the crate template, and ran a local dry run of the move. It changes
nothing; it lists what the next lanes change.

The review took its accepted baseline from
[the review package](../../notes/crate-reviews/counter-frequency-contract.md) and reopens none of
the decisions it settles. Those are the crate name and the thirteen ratified names (`MAGIC`,
`VERSION`, `INTERFACE_ID`, `PAGE_CONTENT_BYTES`, `PAGE_VIRTUAL_ADDRESS`,
`MINIMUM_PLAUSIBLE_HERTZ`, `MAXIMUM_PLAUSIBLE_HERTZ`, `is_plausible`, `build_page`,
`CounterFrequencyPage`, `new`, `hertz` and the `hertz` and `virtual_address` parameters). They are
also the kept wire value `TIMEBAS1`, the dual license, and the three edits the package assigns to
the move lane: the README's "written for the nife kernel" sentence, the three sibling-crate
citations, and the machine and tree names in the doc comments. The package's open question on `PAGE_VIRTUAL_ADDRESS` existing on aarch64 stays
with the maintainer and is not reopened. Where a later rule contradicts the baseline, the item
says so; one does (item 7, the `#[doc(alias)]` sentence).

Reuse: the template `nifeos/crate-workflows` and its helpers, `script/crate-move --dry-run`,
`script/crate-standalone`, Kani, cargo-llvm-cov, Miri and the sibling contract crates' tests as
patterns. No new tool.

## Goal

A stranger who finds `counter_frequency_contract` in its own repository can tell in one screen
what the page is, build and test it from the README alone, and trust every claim the documentation
makes about what is checked and by whom. The repository's gates are green on the day the crate
arrives, for the crate's own reasons, and nife's own build keeps working the day the crate leaves.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1073-counter-frequency-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the prior art appendix](1073-counter-frequency-contract-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the move (a published name, a wire value, a
layout, a fixed address, a dependency); everything else is two-way and the maintainer decides it.

### P0: blocks the move or the first green run

1. nife's `std` build reads this crate's source by path, and loses it the day the crate leaves.
   Finding: `xtask/src/farm/generate.rs` copies `crates/counter_frequency_contract/src/lib.rs`
   verbatim into the `std` platform layer as `sys/pal/nife/counterfreqproto.rs`, and
   `xtask/src/farm/stamp.rs` hashes the same path to decide when the farm rebuilds. Both are
   `root.join("crates/...")`. After the move the path does not exist, `cargo xtask std-src` fails,
   and no `std` program builds. The crate's review package does not list this; the packages for
   `clock_contract` and `entropy_contract` do, and `notes/crate-move.md` records it as a known
   gap for thirteen crates of milestone 1017 (the contract crates leave in this order).
   Evidence: `xtask/src/farm/generate.rs` line 142; `xtask/src/farm/stamp.rs` line 70;
   `notes/crate-move.md` lines 295 to 301; `patches/std-nife/overlay/std/src/sys/pal/nife/mod.rs`
   lines 65 to 72.
   Change: both files resolve each copied crate's source directory through `cargo metadata`
   (the `manifest_path` of the package the workspace depends on), so a crate pinned by `rev`
   comes from Cargo's checkout and a crate still in `crates/` comes from there. One change serves
   every copied crate. It lands before the first of them leaves, and it is not this crate's alone.
   The generated copy also keeps the `#[cfg(doctest)] include_str!("../README.md")` item; it is
   harmless inside `std` because `doctest` is never set there, and a sentence in the generator's
   comment should say so.
   Door: two-way. Effort: M. Owner: a nife lane, shared with the other copied contract crates.

2. The template's clippy job is red on arrival.
   Finding: the module documentation writes `x86_64` without backticks, `clippy::doc_markdown`
   is `warn` in the manifest's `[lints.clippy]`, and the template runs clippy with `-D warnings`
   on stable. Stable 1.99.1 and 1.88 both refuse the crate. The nightly this tree pins accepts it,
   which is why nife's own gate never saw it (nobody verified why the two differ).
   Evidence: `crates/counter_frequency_contract/src/lib.rs` line 9; `Cargo.toml` lines 25 to 31.
   Change: backticks around `x86_64` on that line, and a `cargo +stable clippy -- -D warnings`
   pass over the standalone copy before the move, which `script/crate-move --dry-run` should run
   (BUGS).
   Door: two-way. Effort: XS.

### P1: before the first tag

3. The crate's BUGS section states a kernel fact that is no longer true.
   Finding: the module documentation and the README both say the invariant time stamp counter bit
   (`CPUID.80000007H:EDX[8]`) is not checked. The kernel's `x86_64` ISA discovery documents the
   opposite: it refuses a part that does not advertise an invariant TSC and warns under a
   hypervisor. A stranger reading the contract learns a wrong fact about a kernel the crate will no
   longer be able to see. The deeper problem is that a contract's BUGS section describes one
   kernel's behavior at all.
   Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 9 to 11; `README.md` lines 55
   to 57; `kernel/src/arch/x86_64/isa.rs` lines 4 and 10; `kernel/src/arch/x86_64/timer.rs` line 5.
   Change: rewrite the three BUGS entries in the contract's own terms. The page carries one number
   for the life of a process, so the contract cannot express a counter whose rate changes after the
   page is written, and the writer establishes whether a given counter is invariant. A reader of a
   process with no mapping at the fixed address faults, by design. Drop the sentence about QEMU's
   code generator; it is a fact about one writer's environment.
   Door: two-way. Effort: XS.

4. `hertz` returns whatever number the page carries, and no reader in the tree applies the band.
   Finding: `hertz` answers `Some(n)` for any page with the magic, so `build_page(0)` reads back
   as `Some(0)` and `build_page(u64::MAX)` as `Some(u64::MAX)`. The review package proposes to
   keep that and document that the reader calls `is_plausible`. The readers do not:
   `user_mode_runtime::cntfrq_checked` returns `page.hertz()` directly on both architectures, and
   `process_builder` relays that number into a child's page unchecked. In nife this is safe only
   because the kernel asserts the band before `build_page`; a third-party writer has no such
   assertion, and a `Some(0)` becomes a division by zero in any reader that computes seconds.
   Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 208 to 216;
   `crates/user_mode_runtime/src/clock.rs` lines 82 to 94 and 137 to 147;
   `crates/process_builder/src/lib.rs` line 335; `kernel/src/arch/x86_64/timer.rs` lines 257 to
   267.
   Change, within the baseline: say at `hertz` and in the README's contract list that `hertz`
   returns the number as written and the band is the reader's to apply. Add one unit test pinning
   `build_page(0)` to `Some(0)` so the behavior is explicit. Have `cntfrq_checked` in
   `user_mode_runtime` apply `is_plausible` before returning. The stricter alternative, `hertz`
   answering `None` outside the band, is compatible with a later release and the maintainer can
   take it then; the package already leaves this with the maintainer.
   Door: two-way. Effort: S (the reader change is in nife, not in the crate).

5. `new` truncates its address on a target narrower than 64 bits.
   Finding: `new(virtual_address: u64)` casts with `as *const u8`. On a 32-bit target both fixed
   addresses (`0x7000_0000_0000` and `0x38_0000_0000`) are above `u32::MAX`, so the cast yields
   address 0 and the reader dereferences a null page. This leaves every target the crate names
   unaffected; the three bare-metal targets are all 64-bit. It is a hole in the claim, not a defect a current user
   can reach.
   Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 106 to 122 and 175 to 179.
   Change: a `const` assertion that `PAGE_VIRTUAL_ADDRESS <= usize::MAX as u64`, so a narrow
   target fails to compile instead of reading address 0, and one sentence in `new`'s safety
   section saying the address must be representable as a pointer on the target. Keeping `u64` as
   the parameter type is right: the kernel's mapper and the fixed constant are `u64`, and a
   `usize` parameter would move the cast to every caller.
   Door: two-way. Effort: XS.

6. The manifest's `publish` comment points into nife.
   Finding: the `publish = false` line cites `crates/publish-allowlist.txt`, a file the new
   repository will not hold. The dry run's citation check did not report it (BUGS).
   Evidence: `crates/counter_frequency_contract/Cargo.toml` line 11.
   Change: one sentence a stranger can act on, in the shape the `generational_table` review
   settled: the line stays until the release that publishes, which this crate's own release
   milestone owns under the publishing hold.
   Door: two-way. Effort: XS.

7. Documentation a stranger reads, beyond the three edits the package already assigns.
   Finding, four parts. (a) The `PAGE_VIRTUAL_ADDRESS` documentation says the address-space map's
   table of fixed pages "should list it, and does not yet". The map's own documentation now says
   that above 2 GiB only this page has an assignment, so the sentence is stale, and it describes
   another crate's internals. (b) The same item's `Name:` block says `VA` is the abbreviation alias,
   but no `#[doc(alias)]` attribute exists. The rule that a crate carries no alias for this tree's
   own earlier spelling before its first release means none should. This is the one place a later
   rule contradicts the baseline's text, and the move strips the block, so the fix is cosmetic on
   the nife side. (c) `build_page`'s example comment promises an "aligned buffer" while `new` requires
   only byte alignment; the two should agree, and byte alignment is the true requirement because
   the reader copies into a local array. (d) `build_page` and `hertz` say "the calibrated TSC
   rate" and `new`'s safety section says "every `x86_64` process" and opens a sentence in
   lowercase; the package's proposed items 1 and 2 cover these and they belong in the same lane.
   Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 102, 104 to 105, 119 to 120,
   124, 134, 170 to 171 and 190; `crates/address_space_layout/src/lib.rs` lines 4 to 5.
   Change: delete the stale sentence and the alias sentences, say "byte-aligned" in the example,
   and apply the package's items 1 and 2 in the same pull request.
   Door: two-way. Effort: S.

8. The template's next release refuses the prose.
   Finding: at the template's head, `check / standalone` runs `prose_acronyms` over the README and
   every doc comment at ceiling zero, and `passive_voice` over the README at two per thousand
   words. Over the standalone copy, `prose_acronyms` refuses `GHz` (README line 23, `src/lib.rs`
   line 56), `MHz` and `CSR` (`src/lib.rs` line 50), and `passive_voice` refuses the README (six
   passive constructions in 392 words, which allows none). The pinned template release `v7` does
   not run either check, so the dry run passes today; the weekly `lag` job turns red when the
   next release lands, Dependabot opens the bump, and the bump is red until the prose changes.
   Evidence: `crate-workflows/.github/workflows/check.yml` lines 255 to 286;
   `crates/counter_frequency_contract/README.md` lines 3, 8, 19, 23, 56 and 59;
   `crates/counter_frequency_contract/src/lib.rs` lines 50 and 56.
   Change: write "100 kilohertz" and "100 gigahertz" in prose and keep the symbol only inside a
   code span where it names a value. Expand "control and status register (CSR)" at first use. Put
   the six README sentences into the active voice; the first is the baseline's "was written for the
   nife kernel" sentence, which the move lane rewrites anyway.
   Door: two-way. Effort: XS.

9. Nothing is machine-checked, and the manifest says a model checker proves the crate's harnesses.
   Finding: the README says so plainly, and the package leaves a harness to the maintainer. The
   reader's one decision is the magic compare, and the whole value of the crate is that a frame
   without the magic reads as `None`. A harness proving it over every 16-byte frame costs little
   and is the kind of claim a stranger wants checked before trusting the `unsafe` reader. The
   manifest's `check-cfg` comment says "the Kani model checker, which proves this crate's harnesses"
   sets `cfg(kani)`, which is not true until a harness exists. Nobody verified whether the
   template's `proofs / kani` job is green on a crate with no harness; `falsify.py` reports
   "nothing to falsify" and exits 0.
   Evidence: `crates/counter_frequency_contract/README.md` lines 45 to 48; `Cargo.toml` lines
   20 to 21; `crates/counter_frequency_contract/src/lib.rs` lines 185 to 216.
   Change: two harnesses in a `verification` module behind `cfg(kani)`, each with a falsification
   patch the template replays red. The first takes `hertz` over an arbitrary
   `[u8; PAGE_CONTENT_BYTES]` and asserts `None` exactly when the first eight bytes differ from
   `MAGIC`, and otherwise the little-endian `u64` of the next eight. The second asserts
   `build_page(x).hertz() == Some(x)` for an arbitrary `x`. The
   README's "What is proven" section then says what the two prove and that no proof covers the
   caller's mapping promise, which the package already states. The pattern is
   `block_roster_contract`'s `verification` module and its `falsifications/` directory.
   Door: two-way. Effort: S.

### P2: later

The P2 items are in [the appendix](1073-counter-frequency-contract-outside-review/later-items.md).

## Decisions for the architect

The architect froze the identity: magic `TIMEBAS1`, version 1, interface 9 (ruled 2026-11-11 UTC,
#3516).
The decisions still open are under "Open decisions" below.

### How the page crates spell their offset constants (item 10)

Ruled 2026-11-14 (UTC) for the whole family: the byte position of a page field or region-layout
field is named `<FIELD>_OFFSET_BYTES`, with the field name spelled out under `design/naming.md`.
This crate's constants are `MAGIC_OFFSET_BYTES` and `HERTZ_OFFSET_BYTES`. A constant that counts
words says `_OFFSET_WORDS`. No old spelling is kept as an alias.

## Done when

The checks are in [the done-when appendix](1073-counter-frequency-contract-outside-review/done-when.md).

## BUGS

- `script/crate-move --dry-run` runs the pinned template's checks, not the template head's, and
  its notes say so. The dry run also runs no clippy pass, so item 2 was invisible to it, and its
  citation check did not report the manifest comment of item 6. The same gap hid two more refusals
  that the template head and stable clippy made over the staged copy: a README with no badge row,
  and a bare `VisionFive` that nife's own `clippy.toml` accepts and the standalone copy does not.
  Both are fixed in the crate. All of these are findings against `helpers/crate_move.py`, for a
  lane of its own.
- The dry run's three remaining findings (`.github/pull_request_template.md` line 2,
  `CONTRIBUTING.md` line 23, and five "mentions nife" lines in `.githooks/commit-msg`, `deny.toml`,
  `docs/stranger-test.md` and `rustfmt.toml`) are in the pinned `v7` scaffold, not in this crate.
  The template head's scaffold names nife nowhere, so a move at the next template release clears
  them.
- Item 5 came from reading, not from a build for a 32-bit target; the fix is cheap enough to land
  without one, and the three targets the crate names are all 64-bit.
- The kernel's writer caches the page's physical frame on first use. If the rate has not resolved
  at that moment, the frame stays zeroed for the machine's lifetime, and every process reads the
  rate as unknown (`timebase_page_phys` in `kernel/src/user/loader.rs`). The crate's contract does
  not cover when the writer runs.

## Finding status

- Item 1, the `std` farm reads the source by path: Done,
  [#3514](https://github.com/nifeos/nife/pull/3514).
- Item 2, clippy red on stable: Done, [#3513](https://github.com/nifeos/nife/pull/3513).
- Item 3, BUGS in the contract's own terms: Done, [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 4, the band is the reader's to apply: Done, [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 5, pointer-width assertion: Done, [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 6, manifest `publish` comment: Done, [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 7, documentation a stranger reads: Done, [#3556](https://github.com/nifeos/nife/pull/3556),
  with the review package's three move edits.
- Item 8, the template's prose gates: Done, [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 9, two Kani harnesses with falsification patches: Done,
  [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 10, private offset constants: Done, [#3571](https://github.com/nifeos/nife/pull/3571).
- Item 11, README says where the end-to-end test lives: Done,
  [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 12, `Debug` prints the address: Declined. No change until the next change to the crate's
  public interface ([the appendix](1073-counter-frequency-contract-outside-review/later-items.md)).
- The frozen identity: Done, [#3516](https://github.com/nifeos/nife/pull/3516).
- The mutation census after the harnesses: Done,
  [run 43876176886](https://github.com/nifeos/nife/actions/runs/43876176886): 23 mutants tested,
  0 missed.
- The kernel's zeroed frame: Done, [#3283](https://github.com/nifeos/nife/pull/3283): the kernel
  states the outcome at `timebase_page_phys` (BUGS below).
- The move dry run's gaps: Open. `helpers/crate_move.py` runs no clippy pass and does not report a
  manifest comment that cites a nife path (BUGS below); a lane of its own fixes both.
- The review package: Done, [#3615](https://github.com/nifeos/nife/pull/3615),
  [#3626](https://github.com/nifeos/nife/pull/3626) and
  [#3635](https://github.com/nifeos/nife/pull/3635); it states that it is ready.
- Clearance of the review package: Open. calef's, under "Open decisions" below.
- The move to its own repository: Open. A lane moves the crate once the review has its clearance.
- A pre-release tag: Open. calef's, under "Open decisions" below.
- Publishing: Open. A contract crate never publishes to crates.io; the tag is its release.

## Open decisions

### Clearing the review package

Background. Before a crate leaves this tree, calef reads its code and its review package and
clears it; the clearing is also the approval to create its repository. The package for this crate
is [the review package](../../notes/crate-reviews/counter-frequency-contract.md). It states the
crate, its thirteen ratified names, its frozen identity (`TIMEBAS1`, version 1, interface 9) and
what the move changes, and it states that it is ready. Lanes have built every finding above that
the crate itself can fix. The crate is a contract: the kernel writes a 16-byte page and every
process reads it, so what leaves is an agreement between two programs.

Recommendation: A.

- A. Clear the review, which approves creating nifeos/counter_frequency_contract and moving the
  crate there.
- B. Send it back, naming what to change first.
- C. Hold it until milestone 1017's order reaches the crate.

Door: one-way. A public repository is an outward step: once it exists, strangers fetch and pin it,
and the crate's name and page layout become what they rely on.

Prior art. Debian's archive takes no new source package until a person in the `ftpmaster` team
reviews it by hand in the NEW queue (recalled, not read here). nife differs in that the person who clears the
crate is the architect who ratified its names, and a contract crate leaves with its wire identity
already frozen. Example: `block_roster_contract` had its clearance on 2026-11-09 (UTC) and now
lives at nifeos/block_roster_contract, which nife pins at the commit of a pre-release tag.

Blocks: the move, and through it the tag.

### The pre-release tag

Background. After the move, nife depends on the crate by a git commit. A tag such as
`v0.1.0-pre.1` names that commit as a release candidate, with `Cargo.toml` carrying the same
version, so nife pins a tag and not an arbitrary commit. An approval of 2026-11-11 (UTC) gave seven
cleared crates this tag, each after its move check passed and its release dry run was green. That
approval named those crates, and this crate is not among them. A contract crate never publishes,
so for it the tag is the release.

Recommendation: A.

- A. Approve now: the tag `v0.1.0-pre.1` once the move lane finds the repository matches nife's
  copy and the release dry run is green, under the same two conditions as the seven.
- B. Rule it when the move lands, as its own question then.
- C. No tag: nife pins a commit of the repository's `main`.

Door: one-way. Others fetch and pin a pushed tag at once, and a tag that moves breaks every pin
that named it; nobody can reuse the version it burns for another commit.

Prior art. Semantic Versioning 2.0.0 marks a candidate with a pre-release identifier
(`0.1.0-pre.1`), and Cargo never selects a pre-release for a requirement that does not name one.
nife differs in that the tag is never published: it exists only so nife and basalt pin a named
commit. Example: nife pins `block_roster_contract` by `rev` at the commit of its `v0.1.0-pre.2`
tag, with `version = "0.1.0-pre.2"`.

Blocks: the tag, and with it nife's pin by tag; not the move.

## Follow-on

- **Recorded.** Item 10, the spelling of the page crates' offset constants: ruled, see the decisions above.
- **Outstanding.** The move itself, which waits on the architect's clearing of the review, and
  the move dry run's two gaps in `helpers/crate_move.py`. The finding status above says what each
  needs.
- **Recorded.** Item 12 (`Debug` prints the address) is `design/roadmap/1073-counter-frequency-contract-outside-review/later-items.md`
  item 12: no change until the next API change.

## Index row

An outside review of `counter_frequency_contract` before it leaves this tree. Items 1 to 9 and 11
have landed. They cover the `std` farm's source lookup, the stable clippy failure and the BUGS
entries in the contract's own terms. They also cover the band the reader applies, the pointer-width
assertion, the manifest comment and the documentation fixes. The rest are the template's prose
gates, two Kani harnesses, and a README sentence on where the end-to-end test lives. Item 10, the private offset constants, follows the family rule. The identity does not change, and
the crate waits for the architect to clear its review.
