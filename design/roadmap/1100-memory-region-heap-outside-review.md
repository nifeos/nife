---
status: NOT-STARTED
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
`talc` goes, and from the vendoring record in `vendor/README.md` and `vendor/talc.pin`. It raises
none of the decisions those settle: the algorithm (`talc` 5.1.1's core, `Manual` source), the
geometric growth with a floor of eight pages, the one contiguous heap, the untyped slot as part of
the program's capability contract, and the step at which the crate leaves. **There is no package
for this crate in `notes/crate-reviews/`**, so no name ruling exists to reopen; every name below is
provisional or unmarked.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-standalone`,
`script/crate-review-packet`, `helpers/name_release.py`, cargo-mutants, Kani, and the policy copies
in `patches/std-nife` and `redoxfs_server/src/bin/second_mount.rs`. No new tool.

## Goal

A stranger who meets `memory_region_heap` in its own repository can tell in one screen what pays
for the heap, where it lives, how it grows and when it refuses. The same stranger can build and test
the growth policy on a host without a kernel, can read every `unsafe` block against a stated
invariant that holds, and sees the template's gates green for the crate's own reasons.

## What was checked, and how

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
   for the upstream. `abi`, `address_space_map` and `user_mode_runtime` are path dependencies. The
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

Two one-way doors. Everything else above is the maintainer's.

### A. The seven public names, and the shape of `commit_all`

Background. A crate that leaves carries no `Name:` blocks, so the architect must ratify every name a
stranger meets before the move commit. Five of the six public items have no block. The crate name
and `initialize` are provisional. The template's `question_shape` helper refuses `commit_all`
because it returns `bool` through a shared reference, and the template has no baseline. The naming
rules exempt "a function that acts and reports whether it worked", which `commit_all` is, but the
helper cannot see that through `&self` (its header calls this a known false stop), and the kept form
it offers is for names another project owns.

Recommendation first. Ratify `memory_region_heap`, `MemoryRegionHeap`, `initialize`,
`committed_bytes` and `new` as they stand. Rename `DEFAULT_BASE` to `DEFAULT_BASE_VIRTUAL_ADDRESS`
under the rule that an address is `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS` in every location. Keep
the name `commit_all` and change what it returns: `Result<(), Short>` where `Short` carries the
bytes that the heap could not commit. The helper does not judge a `Result`, the shape honors the
rule's exemption, and the one caller (`components/src/swish/heap.rs`) discards the value today, so
no call site changes meaning. Item 13's two accessors, `maximum_bytes` and `base_virtual_address`,
ride in the same table.

Prior art. `Vec::try_reserve` returns `Result<(), TryReserveError>` for exactly this shape: act, and
say how it failed. `std_runtime_contract::HEAP_BASE` is the same value as `DEFAULT_BASE` under
another spelling. The move is the moment to make the two agree or to make one defer to the other.
`address_space_map::HEAP.start` is where both come from.

Concrete example. After the ruling, swish's `initialize_heap` reads
`HEAP.initialize(BUDGET, memory_region_heap::DEFAULT_BASE_VIRTUAL_ADDRESS,
swish::HEAP_MAXIMUM_BYTES); let _ = HEAP.commit_all();` and the template's name checks pass at a
ceiling of zero.

Not taken. `commit_all` returning the committed byte count (a `u64` the caller must compare against
a cap it has to know), and `is_fully_committed` (a question, but the function acts).

### B. Which `talc` the crate builds against outside nife

Background. nife builds `talc` from `vendor/talc`, the crates.io 5.1.1 package plus upstream pull
request 57, through `[patch.crates-io]` in each workspace root. A `[patch]` table does not travel
with a library crate: the crate's own repository, and any consumer whose root workspace lacks the
patch, builds the unpatched package. Pull request 57 fixes two fit checks that compared addresses
where they meant sizes. The first fixes undefined behavior that Miri reports through
`allocation_cache`'s host test, and both can hand out overlapping memory where addresses wrap, which
a 64-bit process never reaches in practice (unverified as a proof). The pull request has been open
for seven weeks with one commit and no maintainer comment. The step 9 appendix of milestone 1017
already rules that `redoxfs` takes `talc` as a git dependency pinned by `rev` with `version`,
divergence as a patch file in that repository, and that `talc` gets no repository of nife's own.
That ruling does not say what this crate does, and this crate is a library that other repositories
consume.

Recommendation first. Depend on crates.io `talc = "5.1.1"` in the crate's manifest, unpatched, and
state in the README what the package lacks and why it does not reach a 64-bit process. nife's own
root `[patch.crates-io]` keeps applying to nife's whole graph, this crate included, so every nife
image still builds the fixed source. The `redoxfs` repository carries the patch file that the ruling
gave it. When a `talc` release carries pull request 57, bump and delete the README sentence. This
keeps the crate publishable (crates.io publishes a git dependency only by its `version`, so a pin
would not reach a registry consumer anyway), costs no fork, and leaves the fixed source in every
build that ships.

Prior art. Rust std's wasm target depends on `dlmalloc` from crates.io and carries no patch; a fix
waits for a release. Debian and the BSD ports carry patches at the distribution, not in the library,
which is the shape nife's `[patch]` has today.

Concrete example. The crate repository's `check` runs its host tests (item 8) against the unpatched
5.1.1 and passes, because the wrapped-address case needs a gap that ends within a chunk of the top
of a 64-bit address space. basalt's image, built in nife's workspace, links the patched source. If
upstream stays silent past the next monthly `script/vendor-watch` report, the question returns as a
fork request, which this block does not make.

Not taken. A git pin on upstream at a revision carrying the fix (there is none; the pull request has
not merged). A fork under `nifeos` (refused in the step 9 appendix as needing its own approval). A
`[patch]` instruction in the README for consumers (an instruction a build cannot check).

## Done when

The checks are in [the appendix](1100-memory-region-heap-outside-review/done-when.md).

## BUGS

- The `coverage` job's behavior on a library with `test = false` comes from reading
  `helpers/coverage-floor.sh`, not from a run; item 4 names both outcomes.
- The page cost of a refused `MAP` at an already-mapped address comes from the kernel's own comment
  (`kernel/src/syscall/memory_region.rs` line 72), not from a run.
- The minimum first-heap figure (1,575 bytes) comes from a hand computation over `DefaultBinning`'s
  bit field (`[usize; 3]`, `BIN_COUNT = 191`) and talc's `min_first_heap_size`. Item 9 asks for a
  test that pins it.
- Whether the user-mode panic handler allocates was not read; item 11 asks for it.
- Whether the unpatched `talc` 5.1.1 fit-check overflow can occur in a 64-bit process comes from
  reasoning over the patch, not from a proof. Decision B states it as practice, not as a theorem.
- The mutation census groups this crate with two others; no per-crate figure exists to report, and
  nobody ran one.
- The review did not run `cargo-semver-checks` (no previous release), Miri, the QEMU suite or a
  release dry run.

## Index row

An outside review of `memory_region_heap` before it leaves. Four items block the move: seven
unratified names, a `talc` dependency that loses its patch at the boundary, a manifest without a
README or licenses, and two template name checks that go red. Five items come before the first tag.
They are a re-callable `initialize` that can make talc span unmapped memory, a zero-byte grow that
reports success, and every kernel refusal read as "budget spent". The other two are a growth policy
with no host test and three copies, and documentation that assumes nife. Five items are for later.
