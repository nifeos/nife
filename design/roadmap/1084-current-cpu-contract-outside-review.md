---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017, 956
decision_dependencies: 204
machine_requirements: none
specific_machine: none
needs_person: yes
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
  `INTERFACE_ID` 10 in milestone 956 (the linked interfaces carry checkable versions)'s table (ratification is asked below, the values are not);
- the deliberate `!Send` and `!Sync` of `CurrentCpuPage`, which the sibling package records as a
  difference from `CounterFrequencyPage`;
- the two renames the page-contract family rule already settles (`design/naming/crates.md`), which
  `helpers/name_release.py --draft current_cpu_contract` prints as pre-ruled: `PAGE_BYTES` to
  `PAGE_CONTENT_BYTES` and `PAGE_VA` to `PAGE_VIRTUAL_ADDRESS`;
- the dependency on `address_space_map`, which milestone 1017 step 3 orders ahead of this crate.

Reuse: `script/crate-standalone`, `helpers/name_release.py`, `helpers/lint_externalized_crates.py`,
`helpers/template_report_gates.py`, Miri, the template's `scaffold/` and `sample/`. No new tool.

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
   ruling by date. The dependency on `address_space_map` is by path.
   Evidence: `crates/current_cpu_contract/Cargo.toml` lines 1 to 21.
   Change: inline `license = "MIT OR Apache-2.0"`; `[lints.rust]` carrying
   `unsafe_op_in_unsafe_fn` and `missing_docs`; `rust-version = "1.88"` as the family states, after one build proves it. Add a one-sentence `description`, `repository`, and the three bare-metal targets under `[package.metadata.docs.rs]`. Write a `publish = false` comment a stranger can act on ("set to
   `true` only in the release that publishes"). Depend on `address_space_map` by version once it has left.
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
   Change: one pull request that carries the `--draft` table, with Decision B below as its ask. Then one rename lane applies the ratified spellings in the crate, the kernel (six sites), the
   runtime, the fixtures, the system tests and `xtask/src/farm`. Ratified names get their blocks in
   the commit that applies them; the move commit strips them.
   Door: one-way (published names). Effort: S for the rename lane once ruled.

### P2: later

The P2 items are in [the appendix](1084-current-cpu-contract-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1084-current-cpu-contract-outside-review/before-the-tag.md).

## Decisions for the architect

Only the one-way doors. The architect rules on each one on its own.

### Decision A. Is the bound on CPU ids part of this page's identity?

Background. Every program that reads this page sizes its per-CPU arrays by a constant compiled into
it, `CPU_ID_BOUND`, today 8. The kernel's own `MAXIMUM_CPUS` is the same constant, so inside this
tree the two cannot drift, and a system test carries the bound back across the boundary to prove it.
`machine_statistics_contract` re-exports the constant and its page's word count depends on it, so a
change to the bound already changes that page's layout.

Once the crate is in its own repository, the kernel and a published program may pin different
versions. The page's magic and version cover its layout, and the layout does not change when the
bound does. So a kernel with a bound of 16 writes a page a program built for 8 accepts, reads a CPU
id of 9, and indexes past its array. Today the reader hands that id back without looking.

Recommendation: the bound is part of the identity. A change to `CPU_ID_BOUND` re-mints `MAGIC` and
bumps `VERSION` in the same change, and the constant's documentation says so. `cpu()` refuses an id
at or past the bound with `None` (item 5), so a mismatch that slips through degrades to "unknown"
rather than a panic. This is the restrictive option; loosening it later is compatible, tightening it
later is not.

Options:

- A, recommended: the bound is identity; `cpu()` refuses ids at or past it. Costs one compare on a
  path that already makes two, and a magic re-mint whenever a bigger machine arrives.
- B: publish the writer's bound as a fourth word of the page (an append, which reads zero under an
  older kernel and so as unknown), and have the reader compare it with its own. Costs eight bytes
  and a compare, and makes the mismatch visible without a re-mint. A consumer still cannot size
  a static array by a run-time word, so the compile-time constant stays and B is a check on top of A
  rather than a replacement.
- C: leave it; document that the two sides must agree. Costs nothing now and a panic in a published
  program later.

Prior art: Linux's `rseq` publishes a `u32` with no bound and leaves sizing to a run-time query, so
the mistake is the reader's own. FreeBSD compiles `MAXCPU` into the kernel and exposes the run-time
`mp_maxid` (recalled, unverified). The sibling `counter_frequency_contract` has no bound to agree on.

Concrete example. A kernel with a bound of 16 publishes CPU 9 to a program built with 8. Today:
`caches[9]` on an 8-element array panics. Under A: the kernel's crate carries a new magic, the
program reads `None` for every word, and its standard library walks on one thread, which is the
degradation the crate already documents for a magic mismatch.

Ruled 2026-11-12 (UTC): option A. The bound is part of the page's identity. A change to
`CPU_ID_BOUND` re-mints `MAGIC` and bumps `VERSION` in the same change, and the constant's
documentation says so. `cpu()` answers `None` for a published id at or past the bound. The
documentation of `machine_statistics_contract`'s re-export states that its identity follows the
same rule. Tests cover an id one below the bound (accepted) and an id at the bound
(refused). Item 5 is done by the pull request that carries this ruling.

### Decision B. The names

Background. Ratification covers the crate's name; 18 public names lack it, and the standalone gate refuses the move until every name a stranger meets has it. The page-contract family rule pre-rules two, and the table lists them for completeness only. One pull request carries the table below as
`helpers/name_release.py --draft` prints it; this section adds a recommendation per row.

Recommendation first: ratify every name as it stands except the six that carry `va` (the family
rule and the abbreviation list decide them) and `build_page_allowing`, which says one of its two
parameters.

| name | kind | recommendation | why |
|---|---|---|---|
| `MAGIC`, `VERSION`, `INTERFACE_ID` | constants | ratify | the family's names, ratified in the sibling |
| `UNSCHEDULED` | constant | ratify | says the state, not the value; `rseq`'s analogue is `RSEQ_CPU_ID_UNINITIALIZED` |
| `CPU_ID_BOUND` | constant | ratify; the alternative is `MAXIMUM_CPUS`, the kernel's ratified name for the same value | the crate's doc insists it is a bound on ids and not a count, and `MAXIMUM_CPUS` reads as a count; one spelling for one idea argues the other way. The architect decides which reading wins |
| `PAGE_BYTES` | constant | `PAGE_CONTENT_BYTES` | pre-ruled (32 bytes is content, not a page) |
| `PAGE_VA` | constant | `PAGE_VIRTUAL_ADDRESS` | pre-ruled |
| `CPU_WORD_OFFSET` | constant | ratify | an offset is neither a length nor a limit, so it carries no unit suffix |
| `THREAD_PAGE_SLOTS` | constant | ratify | a count names the things |
| `thread_page_va` | function | `thread_page_virtual_address` | `va` is spelled out everywhere |
| `build_page` | function | ratify | the family rule names it as outside the rule |
| `build_page_allowing` | function | `build_page_with` | the function takes an allowance and a tick; "allowing" names one and reads as a permission |
| `publish`, parameter `page_va` | function | ratify `publish`; `page_virtual_address` | the verb is right; the parameter carries `va` |
| `CurrentCpuPage` | type | ratify | the sibling is `CounterFrequencyPage` |
| `new`, parameter `va` | method | ratify `new`; `virtual_address` | the family rule names the parameter |
| `cpu` | method | ratify | the ruled word for the unit the scheduler runs a thread on |
| `allowance` | method | ratify | the word the thread model uses for the core budget |
| `tick_hertz` | method | ratify | the unit is spelled out |

Prior art: the sibling package ratified 13 names in one pull request with the same table.

Concrete example: after the rename, the reader's one line in a runtime reads
`CurrentCpuPage::new(PAGE_VIRTUAL_ADDRESS)`, which is the sibling's shape to the word.

## Done when

The checks are in [the appendix](1084-current-cpu-contract-outside-review/done-when.md).

## BUGS

- No review package exists for this crate, so the baseline is the review's own reading. A package
  written later may record rulings this block does not know; where the two disagree, the package
  wins.
- Item 6 and the `rust-version` claim came from reading. The review ran no 32-bit or 1.88 build.
- Item 10's claim that stock Kani proves the two harnesses in seconds is an estimate from their
  size, not a run.
- The template's behavior on a crate with zero harnesses was not exercised; the sibling's package
  left the same question open.
- The prior-art rows marked "recalled" were not read in their sources during this review.
- The review ran no mutation census and found none recorded for this crate; the first run in the
  crate's own repository is the first figure.

## Index row

An outside review of `current_cpu_contract` before it leaves. Four P0 items are the standalone gate's thirteen failures (manifest, files, citations, names). Eight P1 items come before the first tag, and one of them is a reader that hands back an id the consumer cannot index. Four P2 items remain. Two
one-way doors for the architect: whether the CPU id bound is part of the page's identity, and the
name review of eighteen names.
