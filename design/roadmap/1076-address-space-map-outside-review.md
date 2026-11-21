---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 956, 1017
decision_dependencies: 171, 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1076. `address_space_layout`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/address_space_layout` at nife commit `137639a3`, as a
stranger meeting the crate in a repository of its own would. It changes nothing; it lists what the
move lane and the lanes after it change.

The crate's review package, `notes/crate-reviews/address-space-layout.md`, holds the settled
baseline. This block treats as settled only what a ruling elsewhere already fixed. That is the interface identity
(`INTERFACE_ID` 2 and `VERSION` 1, from milestone 956 (the linked interfaces carry checkable
versions)), the ratified `MAXIMUM_STACK_PAGES` and the band numbers themselves (§171 (where a
program image starts), option D). It also includes the placement of this crate at step 3 of
milestone 1017 (the contract crates leave in this order), after the linker-script test moves. Every other name in the crate is
provisional by its own `Name:` block or carries none.

Reuse: the template `nifeos/crate-workflows` and its scaffold, Kani, cargo-mutants, the template's
name helpers, and the move shape `jh7110_entropy` and `file_allocation_table` already took. No new
tool.

## Goal

A stranger who finds the crate in its own repository can tell in one screen what it is for. The stranger can build, test and prove it from the README alone. The stranger can place a page with the compiler checking the whole window and not only its first page. The repository's gates are green for the crate's own reasons.
nife consumes it by `rev` with every pin moving together, and loses no check it runs today.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1076-address-space-map-outside-review/what-was-checked.md).

## The items

Effort: S is half a day for a lane or less, M a day or two, L more. A door is one-way when somebody
outside the repository acts on it after the first tag (a public name, a published behavior, a layout, an identity, a dependency). Everything else is two-way, and the maintainer decides it.
Line numbers are from `crates/address_space_layout/src/lib.rs` at `137639a3` unless a path says
otherwise.

### P0: blocks the move

1. The host test reads a file outside the crate.
   Finding: `the_user_linker_script_links_at_the_image_base` does
   `include_str!("../../user_mode_runtime/link.ld")`, so the crate's test target compiles only
   inside nife's tree. Standing alone it fails at compile time before any test runs.
   Evidence: `src/lib.rs` line 358; the standalone build above. Milestone 1017's blocker table names this
   row with the same resolution.
   Change: invert the test. `user_mode_runtime` stays in nife and already depends on the crate, so
   the test moves there and parses its own `link.ld` against `address_space_layout::IMAGE_BASE_VIRTUAL_ADDRESS`. The
   `IMAGE` band's doc comment (line 111) stops promising "a host test here holds the two to the
   same number" and says where the test lives.
   Door: two-way. Effort: S.

2. The falsification records and patches carry nife's tree paths.
   Finding: every harness record reads
   `` `crates/address_space_layout/falsifications/<harness>.patch` `` where the template requires
   `` `falsifications/<module>.<harness>.patch` ``, and every patch's diff header names
   `a/crates/address_space_layout/src/lib.rs`, which `git apply -p1` cannot resolve in the crate's own
   repository. The template's `falsify.py --check` reports "replayable with no patch path" for all
   nine, and the replay would fail every patch as stale.
   Evidence: `src/verification.rs` lines 53, 74, 99, 110, 123, 133, 146, 156 and 171; the nine files under
   `crates/address_space_layout/falsifications/`; the `--check` run above.
   Change: in the move commit, rewrite the nine record lines and the eighteen path lines (two per
   patch header) to crate-relative form, and run the template's `falsify.py` once without `--check`, so each patch visibly turns exactly its own harness red. The patch headers' "Expected to fail"
   paragraphs need no change.
   Door: two-way. Effort: S.

3. The manifest does not stand alone.
   Finding: `publish = false` with a comment pointing at `crates/publish-allowlist.txt`;
   `license.workspace = true`; `[lints] workspace = true`; no `rust-version`; no `description`,
   `repository`, `readme`, `keywords` or `categories`. The template's `standalone.py` refuses
   workspace inheritance, a missing `rust-version` and `publish = false` on a crate that means to
   release. The crate's `Cargo.toml` comment cites milestone 206 (A program image has under 896 KiB, and the failure names an overlap rather than a size) and §171 as nife paths.
   Evidence: `crates/address_space_layout/Cargo.toml` lines 5 to 6, 8 to 13 and 15 to 16.
   Change: `license = "MIT OR Apache-2.0"`, `rust-version = "1.87"` (the lowest the 1.85 failure allows; the template's weekly `msrv` job will confirm the exact floor), and the three template lints inline (`unsafe_op_in_unsafe_fn`, `missing_docs`, `unexpected_cfgs` naming `cfg(kani)`), a one-line
   `description`, `repository` and `readme`. Keep `publish = false` with a one-sentence comment a
   stranger can act on ("stays until the release that publishes"), since a separate ruling holds publication and this block does not reopen it.
   Door: two-way, apart from publication, which is already ruled. Effort: S.

### P2: later

The P2 items are in [the appendix](1076-address-space-map-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1076-address-space-map-outside-review/before-the-tag.md).

## What the move needs in nife

Twenty-seven manifests depend on the crate by `path`, all but `jig/Cargo.toml` with
`version = "0.1.0"`; `page_tables` as a development dependency. Under §305 (git dependencies on `nifeos/*` only, pinned to a
landed commit) each becomes
`address_space_layout = { version = "0.1.0", git = "https://github.com/nifeos/address_space_layout", rev = "<landed commit>" }`,
the form `components/Cargo.toml` already uses for `jh7110_entropy` and `e1000e`. Milestone 1017 rules
one revision per contract per image, so all twenty-seven move in one pull request, and recommends a
root `[workspace.dependencies]` entry so the next bump touches one line. For `memory_region_heap`
(`crates/memory_region_heap/Cargo.toml` line 21), whose default heap base is the map's `HEAP.start`,
the same line applies. `cargo xtask std-src` also copies `src/lib.rs` into std, and it finds the file through `cargo metadata`, so it reads Cargo's git checkout once the pins land. The crate leaves `crates/publish-allowlist.txt`'s rule by gaining
its row, and the root `Cargo.toml`'s member list drops it.

## Prior art

The comparison with nearby work is in [the appendix](1076-address-space-map-outside-review/prior-art.md).

## Decisions for the architect

The architect ruled the crate's name and cleared the review package, which ratifies the names of
items 7, 9, 15 and 17, the whole-window check `window` among them.

### The crate's name

Decision N1, option A, 2026-11-14 (UTC): the crate is `address_space_layout`, in code and as the
package name, with no `package =` rename and no `doc(alias)` of the earlier spelling. The rule in
force: nife's vocabulary rule sets `map` aside where the kernel uses it as a verb, and "layout" is
the word the crate already uses for itself. This tree carries the rename, and the crate's `Name:`
block says ratified.

### A whole-window check, and its name

Background. The three band functions check one page (item 9). The block channel is an 8 MiB window
in `SERVICE_WINDOWS`; the scratch window in `process_builder` is 64 MiB in `RUNTIME_WINDOWS`. Both
are correct today because somebody counted. The crate's BUGS says the check cannot see a window run into the next band. `Band::holds(start, end)` is the public whole-range check, so the fix
is a `const fn` that asserts it and returns the start, so the compiler checks a window's size where its address sits. A new public function is a name, and after the first tag it is permanent.

Recommendation: one function, not three:
`pub const fn window(band: Band, start_virtual_address: u64, bytes: u64) -> u64`, asserting both
arguments page aligned and `band.holds(start, start + bytes)`, so a caller writes
`window(SERVICE_WINDOWS, 0x5000_0000, 8 << 20)`. The three existing functions stay as the one-page
form (or become one-line wrappers over `window` with `PAGE_BYTES`). The alternative, three siblings
(`pair_window`, `runtime_window_span`, `service_window_span`), triples the surface and repeats the
copy-and-paste shape whose slip one falsification patch already demonstrates (`service_window`
checking `HEAP`). Keeping the one-page functions means no caller changes.

Prior art: `vm-memory` checks a region's range against its neighbors once, as data, when the code builds the memory. nife checks at the use site, at compile time, which is the design choice §171 made.

Concrete example: `crates/process_builder/src/lib.rs` lines 51 to 54 builds `SCRATCH_WINDOW` from two `runtime_window` calls, one for the start and one for the end. With `window` it is one call that also proves the 64 MiB between them lies in the band.

Decided by the maintainer on 2026-11-14 (UTC), and the name ratified by the review clearance of
2026-11-21 (UTC). The crate gains one function,
`pub const fn window(band: Band, start_virtual_address: u64, bytes: u64) -> u64`, which asserts
both arguments page aligned and `band.holds(start, start + bytes)` and returns the start. The three
one-page functions become one-line wrappers over it with `PAGE_BYTES`, and their callers do not
change. Item 9 is the work.

### Clearing the review package

Background. Before a crate leaves this tree, calef reads its code and its review package and
clears it; the clearing is also the approval to create its repository. This crate fixes where every
program's image, stack, heap and service windows sit in its address space, and the kernel, the
program builder and the runtime all read it. Its review package lists every public name with its
state, including the provisional names of items 7, 9, 15 and 17. This decision is the clearing of
that package.

Decision, option A, 2026-11-21 (UTC): the architect cleared the review. That approves creating
`nifeos/address_space_layout` and moving the crate with `publish = false`, no tag and no publish.
The clearance ratifies the names of items 7, 9, 15 and 17 as the package proposed them, `window`
included; `VERSION` and `INTERFACE_ID` stay provisional.

- A. Clear the review once the package lands, which approves creating the repository and moving
  the crate.
- B. Clear it with named changes to the provisional names, which land in the move pull request.
- C. Send the package back, naming what to change first.

Door: one-way. A public repository is an outward step, and after the first tag every band and name
in the crate is what an outside caller builds against.

Prior art. FreeBSD fixes its user address space in machine headers (`USRSTACK`, `SHAREDPAGE`),
kept in the kernel's own tree. nife differs in that the layout leaves
the kernel's tree as a crate of its own, pinned by commit, with the compiler checking each window
against its band. Example: `block_roster_contract` had its clearance on 2026-11-09 (UTC) and now
lives in a repository of its own, which nife pins at the commit of a pre-release tag.

## Done when

The checks are in [the appendix](1076-address-space-map-outside-review/done-when.md).

## Finding status

- Item 1, the host test reads a file outside the crate: Done,
  [#3659](https://github.com/nifeos/nife/pull/3659).
- Item 2, falsification records carry nife's tree paths: Open until the move. `helpers/crate_move.py`
  rewrites the eleven records and regenerates every patch in crate-relative form; on the staged copy
  every patch applies with no offset, and each turns exactly its own harness red. [#3886](https://github.com/nifeos/nife/pull/3886) refreshed the
  patches against the renamed source.
- Item 3, the manifest does not stand alone: Done, [#3659](https://github.com/nifeos/nife/pull/3659).
- Item 4, documentation addressed to a reader inside nife: Done,
  [#3659](https://github.com/nifeos/nife/pull/3659).
- Item 5, no README, licenses, changelog or scaffold: Open.
  [#3659](https://github.com/nifeos/nife/pull/3659) added the README and [#3886](https://github.com/nifeos/nife/pull/3886) the license files;
  `CHANGELOG.md`, `_typos.toml`, caller workflows, `.github` files and the recorded stranger run
  come with the move.
- Item 6, parameter and helper names the template refuses: Done,
  [#3659](https://github.com/nifeos/nife/pull/3659).
- Item 7 and decision 2, the address and length constants: Done, [#3886](https://github.com/nifeos/nife/pull/3886). `PAGE_BYTES` and the
  `_VIRTUAL_ADDRESS` stem on the five address constants and the three `ImagePlacement` fields,
  ratified and renamed at every call site.
- Item 8 and decision N1, the crate's name: Done, [#3593](https://github.com/nifeos/nife/pull/3593).
- Item 9 and decision 3, a whole-window check: Done, [#3659](https://github.com/nifeos/nife/pull/3659).
  The review clearance of 2026-11-21 (UTC) ratifies the name `window`.
- Item 10, `const` assertions under Kani's toolchain: Done,
  [#3659](https://github.com/nifeos/nife/pull/3659).
- Item 11, the surviving mutant's exemption: Done, [#3886](https://github.com/nifeos/nife/pull/3886). The crate carries its own
  `.github/mutants-exempts` line and a `.cargo/mutants.toml` that leaves the `cfg(kani)` module
  out, and the template's mutation gate passes on the staged copy.
- Item 12, the layout is the same in every process: Done,
  [#3659](https://github.com/nifeos/nife/pull/3659).
- Item 13, the template's `register_names.py` misreads harness names: Open. The template still
  reads `address` in a function name as an address, so the crate's `check / standalone` is red on
  its first run until nifeos/crate-workflows fixes it.
- Item 14, `BANDS` fixes the band count in its type: Done,
  [#3659](https://github.com/nifeos/nife/pull/3659).
- Item 15, a caller can build `Band` inverted: Done, [#3886](https://github.com/nifeos/nife/pull/3886). `Band::new` asserts `start < end` and
  page alignment at both ends; the fields stay public.
- Item 16, `#![forbid(unsafe_code)]`: Done, [#3659](https://github.com/nifeos/nife/pull/3659).
- Item 17, nife's checks that walk `crates/` lose this crate: Open. [#3886](https://github.com/nifeos/nife/pull/3886) made
  `LOW_HALF_SPLIT_SHIFTS` public and `page_tables` tests its three `SPLIT_SHIFT`s against it; the
  declarations check still has to read the pinned checkout, in the pins pull request.
- Item 18, milestone 1017's stale blocker row: Open. The row that says the `link.ld` test must move
  is stale, and the lane that owns milestone 1017 drops it.
- nife's pins: Open. All twenty-seven manifests move to one `rev` in one pull request after the
  move. `cargo xtask std-src` reads the crate from Cargo's git checkout when the manifests name a
  git dependency; a dry run against a local git repository of the staged crate copied that
  checkout's `src/lib.rs`.
- The move: Open. The architect cleared the review; the repository and the first push follow this tree's
  preparation.
- Publishing: Open. No first publish happens until the last outside-review milestone closes
  (ruled 2026-11-18 UTC).

## Open decisions

None.

## Follow-on

- **Done.** The crate's own name, `address_space_layout`. Items 1, 3, 4, 6, 8, 9, 10, 12, 14 and 16,
  and the README of item 5, are done in this tree: the crate has `window` and a
  manifest that stands alone.
- **Done.** The review package, [notes/crate-reviews/address-space-layout.md](../../notes/crate-reviews/address-space-layout.md),
  lists every public name with its state.
- **Done.** The clearance, and items 7, 11 and 15, with item 17's `page_tables` test.
- **Outstanding.** The repository and the move (items 2 and 5's scaffold), the pins and item 17's
  declarations check, and item 13 in the template; item 18 is left to 1017.

## BUGS

- Item 13 is the template's to fix, and until it lands the crate's `check / standalone` is red on
  the first run for a reason no change here removes.
- The 1.88 floor is the lowest installed toolchain that builds the crate (1.85 fails on
  `is_multiple_of`); no 1.86 or 1.87 toolchain was installed, and the template's `msrv` job is the
  check.
- The template's prose, README and history gates have not run on the crate's README, because they
  run in the crate's own repository; the move runs them.

## Index row

An outside review of `address_space_layout` before it leaves nife found three findings that stop a standalone build (a test reading a file outside the crate, falsification records in nife's path form, a manifest that inherits from the workspace). It found nine to land before the first tag and six for later. The crate's own name has its ruling and its rename. The architect cleared the review package, and the constants carry their ratified names, the whole-window check `window` among them.
