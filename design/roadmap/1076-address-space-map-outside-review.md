---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 956, 1017
decision_dependencies: 171, 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
## Goal

A stranger who finds the crate in its own repository can tell in one screen what it is for. The stranger can build, test and prove it from the README alone. The stranger can place a page with the compiler checking the whole window and not only its first page. The repository's gates are green for the crate's own reasons.
nife consumes it by `rev` with every pin moving together, and loses no check it runs today.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1076-address-space-map-outside-review/what-was-checked.md).

## The items

Effort: S is half a day for a lane or less, M a day or two, L more. A door is one-way when somebody
outside the repository acts on it after the first tag (a public name, a published behavior, a layout, an identity, a dependency). Everything else is two-way, and the maintainer decides it.
Line numbers are from `crates/address_space_map/src/lib.rs` at `137639a3` unless a path says
otherwise.

### P0: blocks the move

1. The host test reads a file outside the crate.
   Finding: `the_user_linker_script_links_at_the_image_base` does
   `include_str!("../../user_mode_runtime/link.ld")`, so the crate's test target compiles only
   inside nife's tree. Standing alone it fails at compile time before any test runs.
   Evidence: `src/lib.rs:358`; the standalone build above. Milestone 1017's blocker table names this
   row with the same resolution.
   Change: invert the test. `user_mode_runtime` stays in nife and already depends on the crate, so
   the test moves there and parses its own `link.ld` against `address_space_map::IMAGE_BASE`. The
   `IMAGE` band's doc comment (line 111) stops promising "a host test here holds the two to the
   same number" and says where the test lives.
   Door: two-way. Effort: S.

2. The falsification records and patches carry nife's tree paths.
   Finding: every harness record reads
   `` `crates/address_space_map/falsifications/<harness>.patch` `` where the template requires
   `` `falsifications/<module>.<harness>.patch` ``, and every patch's diff header names
   `a/crates/address_space_map/src/lib.rs`, which `git apply -p1` cannot resolve in the crate's own
   repository. The template's `falsify.py --check` reports "replayable with no patch path" for all
   nine, and the replay would fail every patch as stale.
   Evidence: `src/verification.rs:53, 74, 99, 110, 123, 133, 146, 156, 171`; the nine files under
   `crates/address_space_map/falsifications/`; the `--check` run above.
   Change: in the move commit, rewrite the nine record lines and the eighteen path lines (two per
   patch header) to crate-relative form, and run the template's `falsify.py` once without `--check`, so each patch visibly turns exactly its own harness red. The patch headers' "Expected to fail"
   paragraphs need no change.
   Door: two-way. Effort: S.

3. The manifest does not stand alone.
   Finding: `publish = false` with a comment pointing at `crates/publish-allowlist.txt`;
   `license.workspace = true`; `[lints] workspace = true`; no `rust-version`; no `description`,
   `repository`, `readme`, `keywords` or `categories`. The template's `standalone.py` refuses
   workspace inheritance, a missing `rust-version` and `publish = false` on a crate that means to
   release. The crate's `Cargo.toml` comment cites milestone 206 and §171 as nife paths.
   Evidence: `crates/address_space_map/Cargo.toml:5-6, 8-13, 15-16`.
   Change: `license = "MIT OR Apache-2.0"`, `rust-version = "1.87"` (the lowest the 1.85 failure allows; the template's weekly `msrv` job will confirm the exact floor), and the three template lints inline (`unsafe_op_in_unsafe_fn`, `missing_docs`, `unexpected_cfgs` naming `cfg(kani)`), a one-line
   `description`, `repository` and `readme`. Keep `publish = false` with a one-sentence comment a
   stranger can act on ("stays until the release that publishes"), since a separate ruling holds publication and this block does not reopen it.
   Door: two-way, apart from publication, which is already ruled. Effort: S.

### P2: later

The P2 items are in [the appendix](1076-address-space-map-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1076-address-space-map-outside-review/before-the-tag.md).

## What the move needs in nife

Twenty-six manifests depend on the crate by `path` with `version = "0.1.0"`; `jig/Cargo.toml` and
`xtask/Cargo.toml` by path alone. Under §305 (git dependencies on `nifeos/*` only, pinned to a
landed commit) each becomes
`address_space_map = { version = "0.1.0", git = "https://github.com/nifeos/address_space_map", rev = "<landed commit>" }`,
the form `components/Cargo.toml` already uses for `jh7110_entropy` and `e1000e`. Milestone 1017 rules
one revision per contract per image, so all twenty-six move in one pull request, and recommends a
root `[workspace.dependencies]` entry so the next bump touches one line. For `memory_region_heap`
(`crates/memory_region_heap/Cargo.toml:21`), whose default heap base is the map's `HEAP.start`,
the same line applies. `cargo xtask std-src` also generates its core into std. Whether that generator resolves a git dependency where it resolved a path one stays unverified and deserves one dry run before the pull request. The crate leaves `crates/publish-allowlist.txt`'s rule by gaining
its row, and the root `Cargo.toml`'s member list drops it.

## Prior art

The comparison with nearby work is in [the appendix](1076-address-space-map-outside-review/prior-art.md).

## Decisions for the architect

Three one-way doors. Each is a public name that becomes the repository's, crates.io's or every
caller's at the first tag.

### 1. The crate's name

Background. The crate is a fixed partition of a process's low half into eight bands, with the
functions that check a page against them. Its name uses `map`, which nife's vocabulary rule sets
aside because the kernel's `map` is a verb (to map a page), and the crate's own first paragraph uses
both senses. The name is provisional by its own block, which says the minting set `user_layout` and `address_space` aside. The block does not record why (unverified). The name becomes the repository name, the crates.io name once a ruling allows publication, and the identifier at 281 sites in nife and in every
third-party implementation of the contract.

Recommendation: `address_space_layout`. "Layout" is the word the crate already uses for itself
(`VERSION`'s doc: "the layout's version"), it is a noun, it has no kernel verb sense, and it reads without Unix exposure. `user_address_space_layout` says more and costs a word that every other band constant already implies. `address_space_bands` names the table and not the functions. Keeping
`address_space_map` is defensible (Linux's own documentation calls its table "the memory map"), but
the rule exists for exactly this double sense and the move is the last cheap moment.

Prior art: FreeBSD's header is `vmparam.h`, "virtual memory parameters"; Linux's documentation says
"memory map"; the Rust crates above say `VirtAddr` and `GuestMemoryRegion` and have no word for a
fixed layout because none has one.

Concrete example: `crates/current_cpu_contract/src/lib.rs:141` would read
`pub const PAGE_VA: u64 = address_space_layout::CURRENT_CPU_PAGE;` (and, with decision 2,
`address_space_layout::CURRENT_CPU_PAGE_VIRTUAL_ADDRESS`).

### 2. The address and length constants

Background. `PAGE` holds a length in bytes; six constants and three enum fields hold virtual
addresses (item 7). nife's naming rules say a length carries its unit and an address says `virtual`
or `physical`, and the page-contract crates already spell `PAGE_BYTES` and `PAGE_VIRTUAL_ADDRESS`.
The template's gate does not catch these, so nothing mechanical forces the change; the rule does.
Ninety call sites in nife; after the first tag, every one is a semver break for an outside caller.

Recommendation: rename in the move pull request: `PAGE` to `PAGE_BYTES`; `IMAGE_BASE`,
`STACK_TOP`, `STACK_TOP_PAGE`, `CURRENT_CPU_PAGE` and `CODE_MODEL_CEILING` to the same stem plus
`_VIRTUAL_ADDRESS`; `ImagePlacement`'s `image_start`, `image_end`, `stack_base` to
`image_start_virtual_address`, `image_end_virtual_address`, `stack_base_virtual_address`. The
alternative, ratifying the short forms as they stand, is cheaper now and reopens at the release
review by nife's own rule, so it saves nothing. A middle course, renaming the constants and leaving
the fields, splits one rule across one enum and is not recommended.

Prior art: FreeBSD's `USRSTACK` and `SHAREDPAGE` carry no unit or kind, and a reader needs the header's comments. `vm-memory` types the kind instead (`GuestAddress`). This crate's bare `u64` constants cannot do that without a wrapper type, a larger change than a name.

Concrete example: `kernel/src/user.rs:21`, `pub const USER_STACK_TOP: u64 =
address_space_map::STACK_TOP;`, becomes `... = address_space_layout::STACK_TOP_VIRTUAL_ADDRESS;`,
and a reader of the kernel knows it is an address without opening the crate.

### 3. A whole-window check, and its name

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

Concrete example: `crates/process_builder/src/lib.rs:51-54` builds `SCRATCH_WINDOW` from two `runtime_window` calls, one for the start and one for the end. With `window` it is one call that also proves the 64 MiB between them lies in the band.

## Done when

The checks are in [the appendix](1076-address-space-map-outside-review/done-when.md).

## BUGS

- Item 13 is the template's to fix, and until it lands the crate's `check / standalone` is red on
  the first run for a reason no change here removes.
- The 1.87 floor is from the 1.85 failure and memory of the stabilization release; no 1.86 or 1.87
  toolchain was on the review machine, and the template's `msrv` job is the check.
- The review did not run the template's prose, README or history gates, because there is no README
  yet; the scaffolded README meets them in its own pull request.
- This review did not verify whether `cargo xtask std-src` resolves a git dependency. The move lane runs it before the pull request.
- The crate's `Name:` block does not give the reasons for setting `user_layout` and `address_space` aside. A lane that brings decision 1 reads the minting pull request first, so the recommendation does not propose a refused name.

## Index row

An outside review of `address_space_map` before it leaves nife found three findings that stop a standalone build (a test reading a file outside the crate, falsification records in nife's path form, a manifest that inherits from the workspace). It found nine to land before the first tag and six for later. It leaves three names for the architect: the crate's own, the address and length constants, and a whole-window check.
