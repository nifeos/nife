---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1073. `counter_frequency_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/counter_frequency_contract` as a stranger to this tree
would meet it: the crate's five tracked files, every caller in the tree, the review package, the
naming rules, the crate template and a local dry run of the move. It changes nothing; it lists
what the next lanes change. The recording lane assigns the number.

The review took its accepted baseline from
[the review package](../../notes/crate-reviews/counter-frequency-contract.md) and reopens none of
the decisions it settles: the crate name, the thirteen ratified names (`MAGIC`, `VERSION`,
`INTERFACE_ID`, `PAGE_CONTENT_BYTES`, `PAGE_VIRTUAL_ADDRESS`, `MINIMUM_PLAUSIBLE_HERTZ`,
`MAXIMUM_PLAUSIBLE_HERTZ`, `is_plausible`, `build_page`, `CounterFrequencyPage`, `new`, `hertz`
and the `hertz` and `virtual_address` parameters), the kept wire value `TIMEBAS1`, the dual
license, and the three edits the package assigns to the move lane (the README's "written for the
nife kernel" sentence, the three sibling-crate citations, and the machine and tree names in the
doc comments). The package's open question on `PAGE_VIRTUAL_ADDRESS` existing on aarch64 stays
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

Verified firsthand, in a fresh shallow clone of `nifeos/nife` at the merge of pull request 3461
and a clone of `nifeos/crate-workflows` at its head, on a copy of the crate standing alone:

- `cargo fmt --check` (the template's `rustfmt.toml`), `cargo test` (five unit tests, four
  documentation tests, the README's example among them) and `cargo doc` with broken links and
  warnings denied: clean on stable 1.99.1 and on 1.88, the manifest's `rust-version`.
- `cargo clippy --all-targets -- -D warnings`: red on stable 1.99.1 and on 1.88 (item 2), green on
  the nightly this tree pins. The template's `--cfg kani` clippy pass over its shim: green.
- The crate builds and passes clippy for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat`
  and `riscv64imac-unknown-none-elf`, apart from item 2, which is the same line on every target.
- Miri (`cargo miri test` on the pinned nightly): all nine tests pass.
- cargo-llvm-cov: 100% of regions, functions and lines in `src/lib.rs`.
- The template's helpers over the standalone copy: `standalone.py` fails only on the `Name:`
  blocks, which the move strips; `spelled_out`, `unit_symbols`, `register_names`, `contract_names`,
  `british_spelling`, `question_shape`, `service_model`, `abbreviations`, `readme_sections`,
  `readme_acronyms`, `inflated_wording`, `history_phrases`, `prose_figures`, `function_length`,
  `file_length` and `comment_blocks` report nothing; `prose_acronyms` and `passive_voice` report
  findings (item 8); `unsafe_scan` refuses a fuzz opt-in, correctly, for the four `unsafe` sites.
- `script/crate-move --dry-run` into an empty directory: the template at `v7` scaffolds, the host
  tests and the three bare-metal builds pass on 1.88, the pinned template's cheap checks pass, and
  the three findings it reports are the README's nife sentence (baseline) and two lines in the
  template's own scaffold (BUGS). The mutation baseline row it carries over reads "missed 0".
- Every caller: `kernel/src/user/loader.rs` (the writer and the mapper), the three
  `kernel/src/arch/*/timer.rs` (the band), `crates/user_mode_runtime/src/clock.rs` (the reader),
  `crates/process_builder/src/lib.rs` (the relay), five programs under `components/src/` (the
  `INTERFACES` declaration), `system_tests/src/user/counter_frequency_tests.rs`, and the `std`
  platform layer that `xtask/src/farm/generate.rs` writes from this crate's source.

Inferred from reading, not run: the behavior on a target whose `usize` is narrower than 64 bits
(item 5); whether the template's `proofs / kani` job passes on a crate with no harness (item 9);
why the pinned nightly's clippy accepts the line stable refuses (item 2).

Not run: a mutation census (outside its window; the carried row says none missed), a fuzz
campaign (the crate has no fuzz target and, with `unsafe` in shipped source, cannot opt in to the
public one), `cargo semver-checks` (no release to compare against), and the template's full
workflow set against a real repository.

## Prior art

How other systems hand a process the rate of its free-running counter. The details marked
unverified are from memory of those sources, not from reading them during this review.

| system | mechanism | rate or scale | update discipline | discovery of the page |
|---|---|---|---|---|
| Arm, aarch64 | `CNTFRQ_EL0`, an architected register firmware writes | hertz, 32 bits | none needed; constant by architecture | a register, no page |
| RISC-V | `/cpus/timebase-frequency` in the device tree; no CSR states it | hertz | the kernel reads it once | no architected way to reach a process; each kernel invents one |
| Linux vDSO | the `vvar` data page beside the vDSO | a multiplier and shift per clock source, not hertz (unverified in detail) | a sequence counter; readers retry across an update | `AT_SYSINFO_EHDR` in the auxiliary vector, not a fixed address |
| FreeBSD | the shared page's `vdso_timekeep` and `vdso_timehands` | a scale factor; a version field (`tk_ver`) and a generation per hand (unverified in detail) | a generation counter, seqlock style | the shared page's address through the auxiliary vector |
| OpenBSD | the `timekeep` page, read-only in every process | a version constant the kernel and libc compare (unverified in detail) | a generation counter | ELF auxiliary vector |
| Fuchsia | a vDSO call returning ticks per second (unverified) | hertz | constant for the boot | the vDSO |
| Rust crates (`quanta`, `fugit`, `x86`, `riscv`, `aarch64-cpu`) | `quanta` calibrates the counter in user space itself; `fugit` carries the rate as a const type parameter; the register crates read the counter and expose no rate on the two architectures without one | hertz, or a compile-time constant | none | not applicable |

Where this crate differs, on purpose: it carries the rate itself in hertz, not a scale pair, so
every reader divides the same way; it has one writer before any reader exists, so it needs no
sequence counter and a torn read is impossible by construction; the page sits at a fixed virtual
address every program compiles in, so there is no auxiliary vector to parse and a process with no
page faults rather than reading a wrong number; and an unknown rate is a zeroed frame that reads
as `None`, where the vDSO designs always have a number. What it costs: a rate that changes after
boot cannot be expressed (every design above shares that for an invariant counter), and the fixed
address becomes part of the contract's identity the moment a published binary compiles it in.

The concrete example the band exists for: a firmware that leaves `CNTFRQ_EL0` at its reset value
reports 0 or 1 hertz. A reader checking `> 0` believes 1 hertz and reports every second as a
billion. `is_plausible(1)` refuses it at the point it is learned, and the three kernel boot paths
assert through that one function.

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
   is read from Cargo's checkout and a crate still in `crates/` from there. One change serves
   every copied crate; it lands before the first of them leaves, and it is not this crate's alone.
   The generated copy also keeps the `#[cfg(doctest)] include_str!("../README.md")` item; it is
   harmless inside `std` because `doctest` is never set there, and a sentence in the generator's
   comment should say so.
   Door: two-way. Effort: M. Owner: a nife lane, shared with the other copied contract crates.

2. The template's clippy job is red on arrival.
   Finding: the module documentation writes `x86_64` without backticks, `clippy::doc_markdown`
   is `warn` in the manifest's `[lints.clippy]`, and the template runs clippy with `-D warnings`
   on stable. Stable 1.99.1 and 1.88 both refuse the crate; the nightly this tree pins accepts it,
   which is why nife's own gate never saw it (the reason the two differ is unverified).
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
   for the life of a process, so a counter whose rate changes after the page is written cannot be
   expressed, and whether a given counter is invariant is the writer's to establish. A reader of a
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
   returns the number as written and the band is the reader's to apply, add one unit test pinning
   `build_page(0)` to `Some(0)` so the behavior is explicit, and have `cntfrq_checked` in
   `user_mode_runtime` apply `is_plausible` before returning. The stricter alternative, `hertz`
   answering `None` outside the band, is compatible with a later release and the maintainer can
   take it then; the package already leaves this with the maintainer.
   Door: two-way. Effort: S (the reader change is in nife, not in the crate).

5. `new` truncates its address on a target narrower than 64 bits.
   Finding: `new(virtual_address: u64)` casts with `as *const u8`. On a 32-bit target both fixed
   addresses (`0x7000_0000_0000` and `0x38_0000_0000`) are above `u32::MAX`, so the cast yields
   address 0 and the reader dereferences a null page. No target the crate names is affected; the
   three bare-metal targets are all 64-bit. It is a hole in the claim, not a defect a current user
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
   table of fixed pages "should list it, and does not yet"; the map's own documentation now says
   that above 2 GiB only this page is assigned, so the sentence is stale, and it describes another
   crate's internals. (b) The same item's `Name:` block says `VA` is the abbreviation alias, but no
   `#[doc(alias)]` attribute exists, and the rule that a crate carries no alias for this tree's own
   earlier spelling before its first release means none should; this is the one place a later rule
   contradicts the baseline's text, and the move strips the block, so the fix is cosmetic on the
   nife side. (c) `build_page`'s example comment promises an "aligned buffer" while `new` requires
   only byte alignment; the two should agree, and byte alignment is the true requirement because
   the reader copies into a local array. (d) `build_page` and `hertz` say "the calibrated TSC
   rate" and `new`'s safety section says "every `x86_64` process" and opens a sentence in
   lowercase; the package's proposed items 1 and 2 cover these and they belong in the same lane.
   Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 102, 104 to 105, 119 to 120,
   124, 134, 170 to 171 and 190; `crates/address_space_map/src/lib.rs` lines 4 to 5.
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
   code span where it names a value, expand "control and status register (CSR)" at first use, and
   put the six README sentences into the active voice (the first is the baseline's "was written
   for the nife kernel" sentence, which the move lane rewrites anyway).
   Door: two-way. Effort: XS.

9. Nothing is machine-checked, and the manifest says a model checker proves the crate's harnesses.
   Finding: the README says so plainly, and the package leaves a harness to the maintainer. The
   reader's one decision is the magic compare, and the whole value of the crate is that a frame
   without the magic reads as `None`. A harness proving it over every 16-byte frame costs little
   and is the kind of claim a stranger wants checked before trusting the `unsafe` reader. The
   manifest's `check-cfg` comment says `cfg(kani)` is set by "the Kani model checker, which proves
   this crate's harnesses", which is not true until one exists. Whether the template's
   `proofs / kani` job is green on a crate with no harness is unverified; `falsify.py` reports
   "nothing to falsify" and exits 0.
   Evidence: `crates/counter_frequency_contract/README.md` lines 45 to 48; `Cargo.toml` lines
   20 to 21; `crates/counter_frequency_contract/src/lib.rs` lines 185 to 216.
   Change: two harnesses in a `verification` module behind `cfg(kani)`, each with a falsification
   patch the template replays red: `hertz` over an arbitrary `[u8; PAGE_CONTENT_BYTES]` answers
   `None` exactly when the first eight bytes differ from `MAGIC`, and otherwise the little-endian
   `u64` of the next eight; and `build_page(x).hertz() == Some(x)` for an arbitrary `x`. The
   README's "What is proven" section then says what the two prove and that no proof covers the
   caller's mapping promise, which the package already states. The pattern is
   `block_roster_contract`'s `verification` module and its `falsifications/` directory.
   Door: two-way. Effort: S.

### P2: later

10. The private offset constants abbreviate.
    Finding: `OFF_MAGIC` and `OFF_HERTZ` shorten "offset". The word is not on the refused list
    and the sibling page crates use the same prefix, so this is a family question rather than a
    defect; the template's abbreviations check is report-only.
    Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 88 to 89.
    Change: `MAGIC_OFFSET` and `HERTZ_OFFSET`, when the family decides, in every page crate at
    once.
    Door: two-way (private). Effort: XS.

11. The reader test in nife compares the user-space number with the kernel's and stays in nife.
    Finding: `system_tests/src/user/counter_frequency_tests.rs` drives the whole path through QEMU.
    It points from nife to the crate, which the move permits, and the crate's own tests already
    drive both halves of the page without a kernel. Nothing to move; the README's "Building and
    testing" section should say that the end-to-end test lives with the kernel that maps the page.
    Evidence: `system_tests/src/user/counter_frequency_tests.rs` lines 1 to 40.
    Change: one sentence in the README.
    Door: two-way. Effort: XS.

12. The pointer-bearing type derives `Debug`, which prints the address.
    Finding: harmless, and the address is a compile-time constant in every real use. A stranger
    may prefer a `Debug` that prints the rate or "unknown". Not worth a release on its own.
    Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 150 to 153.
    Change: none now; fold into the next API change if one comes.
    Door: two-way. Effort: XS.

## Decisions for the architect

One one-way door is open. Everything else above is two-way and the maintainer decides it.

### Freeze the contract's identity before it leaves

Background. Five programs under `components/src/` declare this interface by the pair
(`INTERFACE_ID`, `VERSION`) in their `INTERFACES` note, the kernel and every process compile in
`PAGE_VIRTUAL_ADDRESS`, and every reader compares `MAGIC`. Once one of those programs is built from
a pin outside nife, a change to any of these values has to move every pin in one image, which is
the cost milestone 1017 (the contract crates leave in this order) describes from the two reverts
it measured. The package lists ratifying `INTERFACE_ID = 9` as the architect's item; the sibling
contracts that left before this one each had their identity frozen as a set, and the fixed
addresses belong in this crate's set because a program acts on them exactly as it acts on the
magic.

Recommendation first: freeze the identity as it stands. `MAGIC` is `TIMEBAS1`, `VERSION` is 1,
`INTERFACE_ID` is 9, `PAGE_VIRTUAL_ADDRESS` is `0x0000_7000_0000_0000` on `x86_64` and
`0x0000_0038_0000_0000` on `riscv64`, and `PAGE_CONTENT_BYTES` is 16 with the magic at offset 0
and the little-endian rate at offset 8. Every value is already compiled into the kernel, the
runtime, the `std` platform layer and five programs; none of the items above needs any of them to
change; and a later layout break has a path already (a new magic and version together, which the
reader refuses by construction).

Prior art. Arm fixed the meaning of `CNTFRQ_EL0` in the architecture and never moved it. OpenBSD's
`timekeep` page carries a version constant that libc compares before trusting the layout, and a
mismatch means no page rather than a misread one (unverified in detail). FreeBSD's `vdso_timekeep`
carries `tk_ver` for the same reason (unverified in detail). The design here is the same shape:
the magic is the version, and the reader refuses what it does not recognize.

Concrete example. `components/src/free.rs` declares `(counter_frequency_contract::INTERFACE_ID,
counter_frequency_contract::VERSION)`. If `INTERFACE_ID` changed to 14 after `free` was built from
a pin, the image's interface check would refuse `free` until its pin moved, and the move would have
to carry `disk_surveyor`, `printenv`, `rm`, `slabtop` and `top` in the same pull request.

The alternative, left open, is to renumber or re-mint before the first move while every consumer
is still in this tree. Nothing in this review found a reason to.

## Done when

- `cargo xtask std-src` and the farm stamp read every copied contract crate's source from the
  checkout `cargo metadata` names, proven by a build of the `std` farm with one such crate pinned
  by `rev` (item 1).
- `cargo +stable clippy --all-targets -- -D warnings` and the template's `--cfg kani` clippy pass
  are green on the standalone copy, on 1.88 and on the current stable (item 2).
- The BUGS sections of the README and the module documentation describe only what the contract
  cannot express, and no sentence in the crate states what any kernel checks (item 3).
- `hertz`'s documentation and the README state that the band is the reader's to apply, a unit
  test pins `build_page(0)` to `Some(0)`, and `user_mode_runtime::cntfrq_checked` applies
  `is_plausible` (item 4).
- A `const` assertion refuses a target whose pointer cannot hold `PAGE_VIRTUAL_ADDRESS`, and
  `new`'s safety section says so (item 5).
- No file in the crate names a path, a crate or a machine outside the repository, apart from the
  cited register names; `script/crate-move --dry-run` reports no finding against the crate's own
  files (items 6 and 7 and the baseline's three edits).
- `prose_acronyms` and `passive_voice` from the template's head report nothing over the crate's
  README and source (item 8).
- Two Kani harnesses pass, each with a falsification patch that `proofs / falsify` replays red,
  and the README's "What is proven" section names them (item 9).
- The architect has frozen the identity set, recorded by its values in the review package.
- `cargo test`, `cargo clippy` on stable, `cargo doc` with warnings denied, `cargo kani` and the
  three bare-metal builds are green at the commit the move copies.

## BUGS

- Item 1 is shared by thirteen copied crates and is a nife change, not a change to this crate.
  Until it lands, no copied contract crate can leave without breaking the `std` build, and this
  milestone's dependency on milestone 1017 stands in for that ordering.
- `script/crate-move --dry-run` runs the pinned template's checks, not the template head's, and
  its notes say so. The dry run also runs no clippy pass, so item 2 was invisible to it, and its
  citation check did not report the manifest comment of item 6. Both are findings against
  `helpers/crate_move.py`, for a lane of its own.
- The dry run's two remaining findings (`.github/pull_request_template.md` line 2 and
  `CONTRIBUTING.md` line 23, plus four more "mentions nife" lines in `.githooks/commit-msg`,
  `deny.toml` and `docs/stranger-test.md`) are in the template's scaffold, not in this crate, and
  are the template's to fix.
- Item 5 came from reading, not from a build for a 32-bit target; the fix is cheap enough to land
  without one, and the three targets the crate names are all 64-bit.
- Whether `proofs / kani` is green on a crate with no `#[kani::proof]` is unverified; item 9
  removes the question.
- The kernel's writer caches the page's physical frame on first use; if `frequency_checked` is
  `None` at that moment, every later process reads the zeroed frame (`kernel/src/user/loader.rs`
  lines 159 to 190). That is kernel behavior the kernel's own BUGS should carry, outside this
  crate; it is noted here because the crate's documentation currently describes it.
- The mutation census was not re-run; the carried baseline row reads "missed 0".

## Index row

An outside review of `counter_frequency_contract` before it leaves this tree: two items that block
the move or its first green run (the `std` farm's path copy of the source, and a clippy failure on
stable), seven to land before the first tag (a stale kernel claim in the BUGS section, an
unapplied rate band in the readers, a pointer truncation on narrow targets, a manifest comment
into nife, four documentation fixes, the template's incoming prose gates, and two Kani harnesses),
three for later, and one one-way door for the architect: freezing the identity set as it stands.
