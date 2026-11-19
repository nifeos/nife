---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017, 956
decision_dependencies: 204
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1084. `current_cpu_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review lane that met `crates/current_cpu_contract` as a stranger would: the
crate, every caller in the tree, the naming rules, the standalone gate and the `nifeos/crate-workflows`
template. It changes nothing; it lists what the lanes that move the crate change, and the two
one-way doors that need the architect.

There is no review package for this crate in `notes/crate-reviews/` and no row for it in
`notes/crate-reviews.md`. The review therefore took its baseline from the crate itself,
[notes/current-cpu-page.md](../../notes/current-cpu-page.md), the sibling package
[counter-frequency-contract.md](../../notes/crate-reviews/counter-frequency-contract.md), §204
(how userspace asks where a thread runs), milestone 1017 (the contract crates leave in this order)
and `design/naming.md`. Nothing below reopens:

- the mechanism (a per-thread read-only page, §204), the `Relaxed` store and load with the
  scheduler's handoff as the ordering argument, or the fixed address beside the stack;
- the identity values a published program already compares: `MAGIC` `CURRCPU1`, `VERSION` 1, and
  `INTERFACE_ID` 10 in milestone 956 (the linked interfaces carry checkable versions)'s table (the values are not reopened);
- the deliberate `!Send` and `!Sync` of `CurrentCpuPage`, which the sibling package records as a
  difference from `CounterFrequencyPage`;
- the two renames the page-contract family rule already settles (`design/naming/crates.md`), which
  `helpers/name_release.py --draft current_cpu_contract` prints as pre-ruled: `PAGE_BYTES` to
  `PAGE_CONTENT_BYTES` and `PAGE_VA` to `PAGE_VIRTUAL_ADDRESS`;
- the dependency on `address_space_layout`, which milestone 1017 step 3 orders ahead of this crate.

Reuse: `script/crate-standalone`, `helpers/name_release.py`, `helpers/lint_externalized_crates.py`,
`helpers/template_report_gates.py`, Miri, the template's `scaffold/` and `sample/`. No new tool.

## Finding status

- 1, the manifest is nife's: Open. The license, lints, `rust-version`, description and docs.rs
  targets are the crate's own ([#3657](https://github.com/nifeos/nife/pull/3657)). The move adds
  `repository` and turns the `address_space_layout` path dependency into a pinned one once that
  crate has left.
- 2, README and license files: Done, [#3657](https://github.com/nifeos/nife/pull/3657).
- 3, docs cite nife and quote the decider: Done, [#3657](https://github.com/nifeos/nife/pull/3657).
- 4, names that lack ratification: Open. The public renames are in
  ([#3657](https://github.com/nifeos/nife/pull/3657)) and ratified (ruled 2026-11-19 UTC, option A);
  the `Name:` blocks follow. `spelled_out` still refuses the test helper
  `Aligned::va` and a test binding `va`; the maintainer rules their spelled-out names provisionally.
- 5, `cpu()` returns an id past the bound: Done, [#3542](https://github.com/nifeos/nife/pull/3542).
- 6, `allowance()` truncates on narrow targets: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565).
- 7, the reader reaches its word through an integer: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565).
- 8, unsafe entry points check nothing in debug builds: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565).
- 9, the builder and the readers disagree on unknown: Done,
  [#3657](https://github.com/nifeos/nife/pull/3657).
- 10, nothing machine-checked: Done, [#3657](https://github.com/nifeos/nife/pull/3657).
- 11, a stale BUGS section: Done, [#3657](https://github.com/nifeos/nife/pull/3657).
- 12, the single-file constraint: Open. The README and `BUGS` sentence are in
  ([#3657](https://github.com/nifeos/nife/pull/3657)). The farm check that refuses a private `mod
  x;` or an `include!` in a copied single-file crate, and tells `cfg(test)` and `cfg(kani)` modules
  from real ones, is not built and has no other home than this line.
- 13, `cfg_attr(not(test), no_std)`: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- 14, the type of `CPU_OFFSET_BYTES`: Done (ruled 2026-11-19 UTC, option A). It stays `u64`.
- 15, the kernel's safety comment says 16 bytes: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565).
- 16, two notes that do not link: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Decision A, the CPU id bound is part of the identity: Done,
  [#3542](https://github.com/nifeos/nife/pull/3542).
- Decision B, the names: Done (ruled 2026-11-19 UTC, option A). Built
  ([#3657](https://github.com/nifeos/nife/pull/3657)) and ratified as built.
- Review package: Open. No package for this crate exists under `notes/crate-reviews/`; a lane writes
  one.
- Review clearance: Done (ruled 2026-11-19 UTC, option A), once the package exists; the package is still to be written.

## Goal

A stranger who finds `current_cpu_contract` in its own repository can tell in one screen what the
page is, who writes it, who reads it, and what each of the three words means when it is absent. The
same stranger can build and test it from the README alone, on the host and on the three bare-metal
targets, and trust that every id the reader hands back can index the array the crate tells them to
size. The repository's gates are green for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1084-current-cpu-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1084-current-cpu-contract-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the move (a public name, a published behavior, a
layout, an identity, a dependency); everything else is two-way and the maintainer decides it.

### P0: the standalone gate fails on each of these; the move cannot start

1. The manifest is nife's, not a crate's.
   Finding: `license.workspace = true` and `[lints] workspace = true` inherit from a workspace the
   copy does not have. There is no `rust-version`, no `description`, no `repository`, no `[package.metadata.docs.rs] targets`. The `publish = false` comment sends a reader to `crates/publish-allowlist.txt`, a nife path. The package comment names the architect and a
   ruling by date. The dependency on `address_space_layout` is by path.
   Evidence: `crates/current_cpu_contract/Cargo.toml` lines 1 to 21.
   Change: inline `license = "MIT OR Apache-2.0"`; `[lints.rust]` carrying
   `unsafe_op_in_unsafe_fn` and `missing_docs`; `rust-version = "1.88"` as the family states, after one build proves it. Add a one-sentence `description`, `repository`, and the three bare-metal targets under `[package.metadata.docs.rs]`. Write a `publish = false` comment a stranger can act on ("set to
   `true` only in the release that publishes"). Depend on `address_space_layout` by version once it has left.
   Door: two-way (the license is the family's, already ruled). Effort: S.

2. The files a stranger needs are missing.
   Finding: no README, no `LICENSE-MIT`, no `LICENSE-APACHE`. The sibling crates in
   `crates/` carry all three; this one carries only `Cargo.toml` and `src/lib.rs`.
   Evidence: `ls crates/current_cpu_contract`.
   Change: both license files with their `SPDX-License-Identifier` lines, and a README with the
   template's required headings in its order: the title and summary, `## EXAMPLES`, `## What is
   proven`, `## BUGS`, `## Minimum Rust version`, `## License`. The README states, in the crate's own words, what the page is. It says that a kernel maps it read-only at `PAGE_VIRTUAL_ADDRESS` in every process and writes the CPU word at every switch-in on the destination core. It says that the reader may be stale one instruction later. It says that the three words read `None` for a frame without the magic, `None` for a thread that never ran, and `None` for an allowance or tick of zero. It says that the page type is deliberately neither `Send` nor `Sync`. It says that the layout grows only by appending a word that reads zero where a kernel never wrote it. It says that the crate ships one source file (item 12).
   Door: two-way. Effort: S.

3. The documentation cites what a stranger cannot open, and quotes the decider.
   Finding: the standalone gate counts 3 architect-name lines, 7 milestone citations, 7 section
   citations and 8 paths into the nife tree. The externalization lint counts 3 consumer-name hits.
   The crate docs name `basalt`, `rg`, `user_mode_runtime`, `counter_frequency_contract`,
   `clock_contract`, `crates/cpu_set`, `script/undefined-behavior-check`, `AGENTS.md rule 7`, a
   pull request number and `notes/current-cpu-page.md`. Two doc comments quote the decider by name
   (`UNSCHEDULED`, `tick_hertz`) and the crate doc's `Name:` block quotes the ratifying word. The docs use `TCB` without expansion.
   Evidence: `src/lib.rs` lines 1 to 21, 27 to 34, 53 to 54, 75 to 77, 110 to 111, 117 to 120, 135
   to 140, 259 to 265, 289 to 293, 437 to 439; `Cargo.toml` lines 8 to 14.
   Change: rewrite every doc comment in the crate's own terms. Sentences that today cite a section
   or a note carry the argument itself (the ordering argument is four sentences; the stale-reader
   caveat is one). Describe consumers by role ("the kernel", "a runtime", "a published program's standard library"), never by name. The two quotations become statements of the rule
   ("zero is a valid CPU id, so the unset state is not zero"). `TCB` becomes "thread control block"
   at first use, or the sentence goes. The move commit strips `Name:` blocks, so they need no edit here.
   Door: two-way. Effort: M.

4. Eighteen names lack ratification, and six carry the abbreviation `va`.
   Finding: `name_release.py` lists 14 public items with no `Name:` block and 4 provisional ones;
   the standalone gate refuses the crate until every block reads `ratified`. The template's
   abbreviation gate flags `thread_page_va`, `PAGE_VA`, the parameter `page_va` of `publish`, the
   parameter `va` of `new`, and two test helpers, all for the component `va`, which
   `design/naming.md` spells out everywhere. `PAGE_VA` and `PAGE_BYTES` are already pre-ruled by the
   page-contract family rule.
   Evidence: `src/lib.rs` lines 35, 42, 51, 73, 98, 108, 115, 121, 127, 141, 149, 159, 179, 211,
   228, 255, 269, 277; the gate's output.
   Change: the `--draft` table, built and ratified. One rename lane applies those spellings in the crate, the kernel (six sites), the
   runtime, the fixtures, the system tests and `xtask/src/farm`. Ratified names get their blocks in
   the commit that applies them; the move commit strips them.
   Door: two-way before the first tag (Decision B). Effort: S for the rename lane.

### P2: later

The P2 items are in [the appendix](1084-current-cpu-contract-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1084-current-cpu-contract-outside-review/before-the-tag.md).

## Decisions for the architect

Decision A, the type of `CPU_OFFSET_BYTES`, the public names and the review clearance have rulings,
which are the rules in force.

### Decision A. The bound on CPU ids is part of this page's identity

Ruled 2026-11-12 (UTC), option A. A change to `CPU_ID_BOUND` re-mints `MAGIC` and bumps `VERSION`
in the same change, and the constant's documentation says so. `cpu()` answers `None` for a
published id at or past the bound. The documentation of `machine_statistics_contract`'s re-export
states that its identity follows the same rule. Tests cover an id one below the bound (accepted)
and an id at the bound (refused).

The reason. Every program that reads this page sizes its per-CPU arrays by `CPU_ID_BOUND`, a
constant compiled into it, which equals the kernel's `MAXIMUM_CPUS`. Once the crate is in its own
repository, the kernel and a published program may pin different versions, and the page's layout
does not change when the bound does. Without the rule, a kernel with a bound of 16 writes a page a
program built for 8 accepts, and the program indexes past its array. This is the restrictive
option; loosening it later is compatible, tightening it later is not.

Prior art: Linux's `rseq` publishes a `u32` with no bound and leaves sizing to a run-time query, so
the mistake is the reader's own. FreeBSD compiles `MAXCPU` into the kernel and exposes the run-time
`mp_maxid` (recalled, unverified).

Concrete example. A kernel with a bound of 16 publishes CPU 9 to a program built with 8. The
kernel's crate carries a new magic, the program reads `None` for every word, and its standard
library walks on one thread, which is the degradation the crate documents for a magic mismatch.

### The published type of `CPU_OFFSET_BYTES`

Ruled 2026-11-19 (UTC), option A. `CPU_OFFSET_BYTES` is published as `u64` and stays `u64`. The
constant exists for address arithmetic: its one caller in the tree, the x86 test program in the
kernel, adds it to `PAGE_VIRTUAL_ADDRESS`, also a `u64`, and `socket_contract` publishes its page
offsets as `u64` too. The crate's private offsets stay `usize`, so a Rust reader slicing the page
writes `as usize`. A published constant's type is part of the API every caller compiles against,
so this is a one-way door.

Prior art. Linux's `rseq` user ABI describes the area as a C struct with fixed-width fields and
leaves offsets to `offsetof`, which yields a `size_t`, the C analogue of `usize` (from memory, not
re-read). The tree has both: `socket_contract` publishes `u64` offsets and
`domain_name_resolution_contract` a `usize` one. Concrete example: the x86 program computes
`(PAGE_VIRTUAL_ADDRESS + CPU_OFFSET_BYTES) as u32` for a 32-bit absolute load; a `usize` constant
would make that line `(PAGE_VIRTUAL_ADDRESS + CPU_OFFSET_BYTES as u64) as u32`.

### The public names

Ruled 2026-11-19 (UTC), option A. Every public name stands as built. These are `PAGE_CONTENT_BYTES`,
`PAGE_VIRTUAL_ADDRESS`, `thread_page_virtual_address`, `build_page_with`, `CPU_OFFSET_BYTES` (the
family-wide `<FIELD>_OFFSET_BYTES` form), and the parameters `page_virtual_address` and
`virtual_address`. `MAGIC`, `VERSION`, `INTERFACE_ID`, `UNSCHEDULED`, `CPU_ID_BOUND` (kept over
`MAXIMUM_CPUS`, since it bounds ids and is not a count), `THREAD_PAGE_SLOTS`, `build_page`,
`publish`, `CurrentCpuPage`, `new`, `cpu`, `allowance` and `tick_hertz` keep their spellings. No
alias carries an old name. Each name follows a ruled rule: the page-contract family pattern, units
and `virtual_address` spelled out, and no alias for an old nife spelling. The names become permanent
at the first publish, so this is a one-way door. The `Name:` blocks in the crate source follow in a
later change.

Prior art. Linux's `rseq` calls the unscheduled state `RSEQ_CPU_ID_UNINITIALIZED`, and the sibling
`counter_frequency_contract` ratified its 13 names in one pull request with the same table. nife
differs by spelling every abbreviation out. Concrete example: a runtime's one line reads
`CurrentCpuPage::new(PAGE_VIRTUAL_ADDRESS)`, the sibling's shape to the word.

### The review clearance and the move

Ruled 2026-11-19 (UTC), option A. The review of this crate is cleared once a lane writes the review
package, and that clearance is the go to create `nifeos/current_cpu_contract` and move the crate
there. The package is still to be written by a lane, and the move starts after the package lands.
No tag is part of this ruling, and nothing is published. A public repository is one-way: others see
and clone it once it exists, and nobody can withdraw its history.

Prior art. Servo moved crates it shared with others, such as `url`, into repositories of their own
(from memory, not re-read). nife differs by having one person clear a written review package before
the repository exists. Concrete example: `counter_frequency_contract`, this crate's sibling, follows
the same order of milestone 1017.

## Done when

The checks are in [the appendix](1084-current-cpu-contract-outside-review/done-when.md).

## BUGS

- No review package exists for this crate, so the baseline is the review's own reading. A package
  written later may record rulings this block does not know; where the two disagree, the package
  wins.
- Item 6 came from reading: no 32-bit build has run. The `rust-version` of 1.88 is checked by one
  build of the crate on that toolchain.
- The farm check of item 12 is not built. The farm inlines a copied crate's `pub mod` files, so a
  second source file does not by itself break the copy.
  A private `mod x;` or an `include!` would, and a check for them must tell `cfg(test)` and `cfg(kani)` modules from real ones. Until it
  exists, the crate's README and `BUGS` tell a stranger to keep one file.
- The template's behavior on a crate with zero harnesses was not exercised; the sibling's package
  left the same question open.
- The prior-art rows marked "recalled" were not read in their sources during this review.
- The review ran no mutation census and found none recorded for this crate; the first run in the
  crate's own repository is the first figure.

## Follow-on

- **Outstanding.** Items 1, 4 and 12 and the review package, each as [Finding status](#finding-status) gives it. Checked against the crate's
  code and the template's naming helpers.
- **Milestone 1017.** The contract crates leave in its order. This crate moves once the `Name:` blocks
  read ratified, so the standalone gate's `names` check can pass.

## Index row

An outside review of `current_cpu_contract` before it leaves. Four P0 items are the standalone gate's thirteen failures (manifest, files, citations, names). Eight P1 items come before the first tag, and one of them is a reader that hands back an id the consumer cannot index. Four P2 items remain. The CPU id
bound is part of the page's identity (ruled); the published type of `CPU_OFFSET_BYTES` stays `u64` (ruled); the public names stand as built (ruled).
