---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1100. `memory_region_heap`: what an outside maintainer would fix before it leaves

Raised by a read-only outside review of `crates/memory_region_heap` as a stranger meeting it for
the first time would. It changes nothing; it lists what the next lanes change. The number is the
recording lane's to assign.

The review took its baseline from `notes/heap.md` ("The `memory_region_heap` crate's contract"),
from milestone 1017 (the contract crates leave in this order), step 9 and its appendix on where
`talc` goes, and from the vendoring record in `vendor/README.md` and `vendor/talc.pin`. It raises none of the decisions those settle. Those are the algorithm (`talc` 5.1.1's core, `Manual` source), the geometric growth with a floor of eight pages and the one contiguous heap. They also include the untyped slot as part of the program's capability contract, and the step at which the crate leaves. The review package is
[`notes/crate-reviews/memory-region-heap.md`](../../notes/crate-reviews/memory-region-heap.md).

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-standalone`,
`script/crate-review-packet`, `helpers/name_release.py`, cargo-mutants, Kani, and the std farm's
copied-file list (`xtask/src/farm/copied.rs`). No new tool.

## Finding status

- P0-1, names and the missing `Name:` blocks: Done, [#3597](https://github.com/nifeos/nife/pull/3597).
  The names later items added are ratified (ruled 2026-11-19 UTC).
- P0-2, `talc` and the path dependencies: Ruled (Decision B, option A, 2026-11-14 UTC). The README
  half is Done, [#3840](https://github.com/nifeos/nife/pull/3840): its BUGS section states what
  crates.io's 5.1.1 lacks. The pins wait on milestone 1017 steps 2 and 3: `abi`,
  `address_space_layout` and `user_mode_runtime` have no repositories yet, so the manifest keeps
  their path dependencies, and `script/crate-move --dry-run` fails only on them.
- P0-3, manifest, README and license files: Done, [#3840](https://github.com/nifeos/nife/pull/3840).
  The manifest states its license, `rust-version`, description, keywords, categories and lints,
  both license files sit beside it, the README's host example runs as a doctest, and the crate's
  documentation cites nothing outside it.
- P0-4, the `va` binding and `commit_all`: Done, [#3597](https://github.com/nifeos/nife/pull/3597),
  and the fuzz workspace's binding in [#3840](https://github.com/nifeos/nife/pull/3840). The
  coverage half is Open: the template's `coverage` job first measures the host tests after the
  move.
- P1-5, a second `initialize`: Done, [#3559](https://github.com/nifeos/nife/pull/3559).
- P1-6, a grow of zero bytes: Done, [#3559](https://github.com/nifeos/nife/pull/3559).
- P1-7, kernel refusals read as budget spent: Done, [#3609](https://github.com/nifeos/nife/pull/3609).
- P1-8, page-source seam, host tests, proofs and one copy of the policy: Done. The seam, host tests
  and three harnesses landed in [#3609](https://github.com/nifeos/nife/pull/3609). In
  [#3840](https://github.com/nifeos/nife/pull/3840) the growth arithmetic lives in the core-only
  `src/growth.rs`, which `cargo xtask std-src` copies into std as `heap_growth.rs`, and
  `second_mount` runs on `MemoryRegionHeap` over a host page source. The fuzz target keeps a copy,
  because it reads talc's counters, and its README says so.
- P1-9, documentation and the minimum cap: Done. The crate documentation landed in
  [#3609](https://github.com/nifeos/nife/pull/3609), the README in
  [#3840](https://github.com/nifeos/nife/pull/3840).
- P2-10, `PAGE_BYTES`: Done, [#3609](https://github.com/nifeos/nife/pull/3609).
- P2-11, a panic inside the allocator: Done, [#3609](https://github.com/nifeos/nife/pull/3609).
- P2-12, growth is monotonic: Done. The crate documentation and the README say so, and the README's
  BUGS section carries the line, [#3840](https://github.com/nifeos/nife/pull/3840).
- P2-13, accessors for the cap and the base: Done, [#3609](https://github.com/nifeos/nife/pull/3609).
- P2-14, the `Sync` argument: Done, [#3609](https://github.com/nifeos/nife/pull/3609).
- Decision A, the seven names and `commit_all`: Done (option A, 2026-11-14 UTC),
  [#3597](https://github.com/nifeos/nife/pull/3597).
- Decision B, which `talc` outside nife: Ruled (option A, 2026-11-14 UTC). It stands where item 2
  stands; the pins wait on the same steps of milestone 1017.
- A fork of `talc` if upstream stays silent: Open, and conditional. It is an open decision below.
- Review package: Done, [memory-region-heap.md](../../notes/crate-reviews/memory-region-heap.md).
  The review clearance: Done (ruled 2026-11-20 UTC, option A;
  `nifeos/memory_region_heap` approved with `publish = false`, no tag, no publish). The move stays
  Open.

## Open decisions

### Ratify the names the later items added (ruled 2026-11-19 UTC)

The rule in force, option A. The thirteen names the fixes for items 5 to 13 added stand as built:
`Short`, `PageSource`, `MemoryRegionPages`, `Refusal`, `WiringRefusal`, `base_virtual_address`,
`maximum_bytes`, `initialize_source`, `with_source`, `is_budget_spent`, `is_initialized`,
`last_refusal` and `map_page`. Each public item's `Name:` block reads ratified. The unpublished
fuzz crate's names (the modules `src`, `policy` and `sequence` and the function `run`) stay
provisional, since that crate never publishes.

Prior art. `dlmalloc` names its page seam `Allocator`, and `rlsf` names it `FlexSource`; both
crates settled the name before 1.0. `Vec::try_reserve` returns a named error type, which is the
shape `Short` and `Refusal` follow. nife differs in ratifying every public item, not only the
ones a README shows. Concrete example: a consumer writes
`MemoryRegionHeap::with_source(source)` and matches `Refusal` from `last_refusal()`; those
spellings are what it compiles against.

### Fork `talc` if upstream stays silent

Background. The crate's allocator core is `talc` 5.1.1 from crates.io. nife builds a patched copy
(`vendor/talc`, carrying upstream pull request 57, two fit-check overflow fixes), but a `[patch]`
table does not travel with a library, so outside nife the crate builds the unpatched release.
Decision B accepts that, because the overflow needs addresses that wrap at the top of a 64-bit
space. It also rules that the question returns as a fork request if upstream stays silent past
the next monthly `script/vendor-watch` report. Pull request 57 is open with no comments, and no
release carries the fix. This decision exists only once that report passes in silence.

Recommendation: A, when the question arises. The fix does not reach a 64-bit process, nife's own
images build the patched source, and a fork adds a crate, a repository and a maintenance duty
that a later upstream release would make redundant.

- A. Keep the unpatched crates.io dependency and the README sentence, and keep watching upstream.
- B. Fork `talc` under `nifeos`, publish the fork, and depend on it.
- C. Replace `talc` with another allocator core.

Door: one-way for B and C (a published crate and a dependency other repositories build); A
changes nothing.

Prior art. Rust's std depends on crates.io `dlmalloc` 0.2 for its wasm targets with no patch,
and takes fixes only through releases. Debian and the BSD ports keep a patch at the distribution,
which is the shape nife's root `[patch]` has today. A fork is the usual answer when an upstream has
stopped, and `talc` has a working maintainer. Concrete example: under A, the crate's own repository runs its
host tests against unpatched 5.1.1 and passes, while basalt's image, built in nife's workspace,
links the patched source.

Blocks: nothing until the vendor-watch report passes in silence; after that, the first publish.

### Clear the review package and approve the repository (ruled)

Ruled 2026-11-20 (UTC), option A: the package is cleared and the clearance creates the repository,
`nifeos/memory_region_heap`, with `publish = false`. No tag and no publish. The move waits for `abi`,
`address_space_layout` and `user_mode_runtime` to get repositories and leaves with `subtree_scope`
and before `redoxfs`.

Background. A crate leaves this tree for its own `nifeos` repository only when the architect
clears its review package; that clearance approves the repository. A package lists every public
item with its signature, and `script/crate-review-packet` writes it. The package for this crate is
[memory-region-heap.md](../../notes/crate-reviews/memory-region-heap.md), written under the
recommendation below. The
components, the fixtures and the RedoxFS server all depend on it, so it serves several consumers and
meets the bar for leaving.

Recommendation: A. A package written after the names are ratified shows the architect the crate
as it will ship.

- A. A lane writes the package; the architect's clearance creates the repository.
- B. Clear on this outside review alone, without a package.
- C. Keep the crate in nife and move it with the runtime package.

Door: one-way. Creating a public repository and moving the crate there are outward steps that
other pins then follow.

Prior art. The Rust library team reviews a new std interface through an API change proposal
before it stabilizes, and Fuchsia's API council reviews a library before it enters the SDK (both
recalled, not re-read). nife differs in reviewing a whole crate once, before its first publish.
Concrete example: the architect cleared `generational_table` on 2026-11-08 (UTC), and its
repository `nifeos/generational_table` dates from that day.

Blocks: the move.

## Goal

A stranger who meets `memory_region_heap` in its own repository can tell in one screen what pays
for the heap, where it lives, how it grows and when it refuses. The same stranger can build and test
the growth policy on a host without a kernel, can read every `unsafe` block against a stated
invariant that holds, and sees the template's gates green for the crate's own reasons.

## How the review checked

The tools run, the sources read and what was not run are in [the appendix](1100-memory-region-heap-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1100-memory-region-heap-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the move (a public name, a published behavior, a
dependency). Everything else is two-way and the maintainer decides it.

### P0: blocks the move or turns the template's first run red

1. Seven names are not ratified, and five public items carry no `Name:` block.
   Finding: `script/crate-standalone` step 0 fails on the crate name (provisional) and on
   `initialize` (provisional). `DEFAULT_BASE`, `MemoryRegionHeap`, `new`, `commit_all` and
   `committed_bytes` have no block at all, so they never entered the worklist. A crate that leaves
   is a third-party crate and the move commit strips the blocks, so every name must read `ratified`
   first.
   Evidence: `crates/memory_region_heap/src/lib.rs` lines 11, 29, 54, 80, 98, 119, 128.
   `design/module-block-baseline.txt` lines 5190 to 5198. `helpers/name_release.py --draft`.
   Change: one lane writes the five missing blocks, brings the seven names to the architect as one
   table (Decision A below carries the recommendation), and applies the ruling.
   Door: one-way (public names). Effort: S plus the ruling.

2. The `talc` dependency does not survive the move as written, and three path dependencies point
   into nife.
   Finding: the manifest asks crates.io for `talc = "5.1.1"` and relies on the nife workspace's
   `[patch.crates-io]` to build `vendor/talc`, which carries upstream pull request 57 (two fit-check
   overflow fixes; the first fixes undefined behavior that Miri reports). A `[patch]` table applies
   only in the root workspace, so a repository of the crate's own, and every workspace that consumes
   it without the patch, silently builds the unpatched package. The pull request has been open for
   seven weeks at the time of the review with no maintainer comment, which is a reliability signal
   for the upstream. `abi`, `address_space_layout` and `user_mode_runtime` are path dependencies. The
   order of milestone 1017 moves them at steps 2 and 3, before this crate at step 9, so their
   repositories will exist, and the lines become `git` and `rev` pins with `version` beside them.
   Evidence: `crates/memory_region_heap/Cargo.toml` lines 19 to 28. `vendor/talc.pin`.
   `design/roadmap/1017-the-contract-crates-leave-in-this-order.md`, "Where `talc` goes at step 9".
   Change: Decision B below. Whatever the ruling, the move lane states in the README which `talc`
   the crate builds against and what the unpatched package lacks.
   Door: one-way (a dependency). Effort: S.

3. The manifest and the files a stranger needs.
   Finding: no README, no `LICENSE-MIT`, no `LICENSE-APACHE`, no `rust-version`, `license` and
   `lints` inherited from the workspace, no `description`, `repository`, `keywords` or `categories`.
   The `publish = false` comment points at `crates/publish-allowlist.txt`, a file that is not in the
   crate's repository. The crate docs link `notes/heap.md` by path. The manifest comment names the
   architect and a decision section.
   Evidence: `Cargo.toml` lines 2 to 28. `src/lib.rs` line 2. `script/crate-standalone` output, 15
   of its 16 failures.
   Change: fill in the scaffold's `README-crate.md` (what pays, where it lives, how it grows, the
   minimum useful cap, what a null return means, the `talc` it builds against). Add both license
   files, an explicit `license = "MIT OR Apache-2.0"` and a `rust-version`. Write a manifest comment
   a stranger can act on. The license and the version policy are already ruled for every crate that
   leaves. Nothing here reopens them.
   Door: two-way (the license is one-way and already ruled). Effort: S.

4. The template's `check` workflow goes red on the first pull request for the crate's own reasons.
   Finding: two naming helpers fail at the template's ceiling of 0. The local binding `va` is on the
   spelled-out list (`virtual_address`), and `commit_all` returns `bool` through a shared reference,
   which `question_shape` reads as a getter with a question's answer. nife holds `commit_all` in
   `design/naming-rule-baseline.txt`. The template has no baseline. Separately, `[lib] test = false`
   and `doctest = false` mean the `tests` job runs nothing. The `coverage` job floors each file at
   85 percent of lines covered. With no test binary the report is either empty or shows `src/lib.rs`
   at zero (unverified which). Either reads as a red or a vacuous green.
   Evidence: `src/lib.rs` lines 119 and 165. `Cargo.toml` lines 11 to 14. The template's
   `helpers/spelled_out.py`, `helpers/question_shape.py`, `helpers/coverage-floor.sh`.
   Change: rename the binding to `virtual_address` now (XS). For `commit_all`, Decision A carries
   the recommendation. For the tests, item 8 is the real fix. Until it lands, the move lane either
   exempts `src/lib.rs` in `.github/coverage-exemptions` with the stated reason ("every path ends in
   a syscall; tested under QEMU") or lands item 8 first.
   Door: `va` is two-way. `commit_all` is one-way (a public name). Effort: XS, plus item 8.

### P2: later

The P2 items are in [the appendix](1100-memory-region-heap-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1100-memory-region-heap-outside-review/before-the-tag.md).

## Decisions for the architect

Two one-way doors, both ruled. The decisions still open are under "Open decisions" above.

### A. The seven public names, and the shape of `commit_all` (ruled 2026-11-14 UTC)

The rule in force, option A (block 1100 decision A):

- `memory_region_heap`, `MemoryRegionHeap`, `initialize`, `committed_bytes` and `new` are ratified
  as they stand. Each public item carries a `Name:` block that reads ratified.
- `DEFAULT_BASE` is `DEFAULT_BASE_VIRTUAL_ADDRESS`, under the rule that an address is
  `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS` in every location.
- `commit_all` keeps its name and returns `Result<(), Short>`, where `Short` carries the bytes the
  heap could not commit. `Short` is provisional. The one caller, the shell's `initialize_heap`,
  discards the value with `let _ =`.
- `maximum_bytes` and `base_virtual_address` are the two accessors of item 13. Both are
  provisional.
- The local binding `va` is `virtual_address` (item 4).

Prior art. `Vec::try_reserve` returns `Result<(), TryReserveError>` for exactly this shape: act,
and say how it failed. `std_runtime_contract::HEAP_BASE` is the same value as the heap's default
base under another spelling; the move is the moment to make the two agree or to make one defer to
the other. `address_space_layout::HEAP.start` is where both come from.

### B. Which `talc` the crate builds against outside nife (ruled 2026-11-14 UTC)

The rule in force, option A (block 1100 decision B):

- Outside nife the crate depends on crates.io `talc = "5.1.1"`, unpatched.
- The crate's README, written by the move lane, states what the unpatched package lacks (upstream
  pull request 57's two fit-check overflow fixes) and why a 64-bit process does not reach it.
- nife's root `[patch.crates-io]` keeps applying to nife's whole graph, this crate included.
- `redoxfs` carries its own patch file, as the step 9 appendix of milestone 1017 rules.
- When a `talc` release carries the fix, the crate bumps and the README sentence goes.
- If upstream stays silent past the next monthly `script/vendor-watch` report, the question
  returns as a request to fork. A fork under `nifeos` needs its own approval.

The evidence, background, prior art and the options not taken are in
[the appendix](1100-memory-region-heap-outside-review/talc-decision.md).

## Follow-on

- **Outstanding.** The move, which leaves with `subtree_scope` and before `redoxfs`. The path dependencies become `git` and `rev` pins once milestone 1017 steps 2 and 3
  give `abi`, `address_space_layout` and `user_mode_runtime` repositories. The move drops the fuzz
  workspace's `[patch.crates-io]` line for the vendored talc, and the template's `coverage` job
  then measures the host tests for the first time.

## Done when

The checks are in [the appendix](1100-memory-region-heap-outside-review/done-when.md). Each item's
state is under "Finding status" above.

## BUGS

- The template's `coverage` job has not run on the crate; its 85 percent floor is unmeasured
  against the 16 host tests and the README doctest.
- The fuzz target drives a copy of the policy, so a change to the crate's policy that the copy
  misses goes unchecked there.
- The page cost of a refused `MAP` at an already-mapped address comes from the kernel's own comment
  (`kernel/src/syscall/memory_region.rs` line 72), not from a run.
- The minimum first-heap figure (1,575 bytes) comes from a hand computation over `DefaultBinning`'s
  bit field (`[usize; 3]`, `BIN_COUNT = 191`) and talc's `min_first_heap_size`;
  `the_first_heap_fits_in_one_page` pins it.
- Whether the unpatched `talc` 5.1.1 fit-check overflow can occur in a 64-bit process comes from
  reasoning over the patch, not from a proof. Decision B states it as practice, not as a theorem.
- The mutation census groups this crate with two others; no per-crate figure exists to report, and
  nobody ran one.
- The review did not run `cargo-semver-checks` (no previous release), Miri, the QEMU suite or a
  release dry run.

## Index row

An outside review of `memory_region_heap` before it leaves. Every code and documentation fix has
landed: a page-source seam, host tests, three Kani harnesses, one copy of the growth arithmetic that
std generates from, a README and both licenses. What remains is the move: path dependencies that
become pins once their crates leave, and clearing the review package.
