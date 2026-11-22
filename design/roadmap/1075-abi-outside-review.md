---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017, 1034
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1075. `abi`: what an outside maintainer would fix before the move

Raised by a read-only review of `crates/moho_abi` as a stranger would meet it in a repository of its own.
It changes nothing; it lists what the next lanes change. The crate is the last of step 2 of
milestone 1017 (the contract crates leave in this order). No review package exists for it under
`notes/crate-reviews/`, so this block has no settled baseline to defer to: the crate's ruled name,
`moho_abi` (§323), and the sixteen ratified item names stand, and everything else below is open.

Reuse: `script/crate-move --dry-run`, the template `nifeos/crate-workflows` and its helpers,
`helpers/lint_externalized_crates.py`, `helpers/format_values_check.py`, `helpers/name_release.py`,
cargo-mutants' recorded census, the crates.io API. No new tool.

## Goal

A stranger who finds this crate can tell in one screen what it defines.
The stranger can build and test it from the README alone. Every method's contract reads without a path into another repository. A changed number cannot ship without the version changing with it. The repository's gates are green
on arrival for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1075-abi-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1075-abi-outside-review/prior-art.md).

## The items

Effort: S is under half a day for a lane, M a day or two, L more. A door is one-way when somebody
outside the repository acts on it after the release (a published name, a wire number, a layout, a
dependency); everything else is two-way and the maintainer decides it.

### P0: red on arrival

1. `cargo clippy -- -D warnings` fails in the staged repository.
   Finding: `src/survey.rs` line 39 writes `VisionFive 2's` without backticks, which `clippy::doc_markdown`
   (a lint the manifest turns on) reports. nife passes it only because the root `clippy.toml` lists
   `VisionFive` under `doc-valid-idents`; the template's scaffold carries no `clippy.toml`, so the
   `check / clippy` job is red on the first push. Verified on stable 1.99.1 and on the pinned nightly.
   Fix: write `` `VisionFive 2` `` (one edit, no new file). The alternative, shipping a
   `clippy.toml`, adds a file every future lint difference will grow.
   Effort: S. Door: two-way.

2. The crate cannot publish under its name.
   Finding: crates.io already has a crate named `abi` (version 0.1.4, 251 downloads, unrelated). The
   repository can be `nifeos/abi` and consumers can pin it by `rev`, but the publication step of the
   runbook (`notes/crate-move.md`, steps 16 to 19) and the README's badge row have no name to use.
   Fix: decision A below: the crate is `moho_abi` in code and as the package name, so the
   manifest and every `use abi::` change together.
   Effort: S. Door: one-way (a published name).

### P1: before the tag

The P1 items are in [the appendix](1075-abi-outside-review/before-the-tag.md).

### P2: later

The P2 items are in [the appendix](1075-abi-outside-review/later-items.md).

## Decisions for the architect

The architect ruled decisions A and C, and decision B is the maintainer's provisional ruling.

### A. The published name (ruled)

Rule in force, ruled 2026-11-14 (UTC) as §323 ruling 2: the crate is `moho_abi`, in code and as
the package name. Moho names the capability binary interface, the boundary programs target, and
`nife` stays the name of the capability microkernel. The name was free on crates.io when ruled,
where `abi` belongs to another author's crate (0.1.4). Prior art: Redox publishes its system call
numbers as `redox_syscall`, and Hubris keeps a crate named `abi` that it never publishes. Example:
the rename turns `abi::SYSTEM_CALL_INVOKE` in `kernel/src/system_call.rs` into `moho_abi::SYSTEM_CALL_INVOKE`.

### B. `console::WRITE` stays reserved (ruled)

Rule in force, ruled 2026-11-14 (UTC) by the maintainer as a provisional two-way door; the architect
may revisit it before the first publish. Nobody outside the project uses nife, so the door stays
two-way until the crate publishes (as `moho_abi`) and has outside users.

`console::WRITE` is a method number of a kernel object that no kernel serves and no program invokes.
The number stays in the crate as reserved: the module and its pinned line stay, `ABI_VERSION` does
not change, and the number is never reused for another method. Removing a number that no kernel
serves is not done, and changing the meaning of a number a kernel serves owes the bump. The gate
(`helpers/format_values_check.py`) refuses a changed or removed public number of the crate without a
changed `ABI_VERSION`; it is unchanged. A test (`reserved_numbers_are_never_reused`) holds the
reservation, and the crate's docs state the rule beside `ABI_VERSION` and in the README.

### C. Clearing the review package (ruled)

Rule in force, ruled 2026-11-22 (UTC), option A: the review of
[the package](../../notes/crate-reviews/moho-abi.md) clears once step 3 of milestone 1121 (the Moho
rename: the order in which the binary interface takes its name) lands the `moho_abi` rename. The
clearance is the go to create the repository with `publish = false`, no tag and no publish. The
same ruling ratifies six names: `address_wait::SIZE_U8`, `SIZE_U16`, `SIZE_U32` and `SIZE_U64`
become `WIDTH_8_BITS`, `WIDTH_16_BITS`, `WIDTH_32_BITS` and `WIDTH_64_BITS`,
`address_wait::SIZE_MASK` becomes `WIDTH_MASK`, and `usage::SIZE` becomes `SIZE_PAGES`. No number
changes.

Prior art. Linux keeps its system call numbers in `arch/*/include/uapi`, reviewed on the kernel's
own mailing lists, and never renumbers one once a release carries it. nife differs in that the
numbers leave the kernel's tree as a crate of their own, with a counting test that refuses a
changed number without a changed `ABI_VERSION`. Example: `block_roster_contract` cleared on
2026-11-09 (UTC) and lives in a repository of its own, which nife pins at the commit of a
pre-release tag.

## Done when

The checks are in [the appendix](1075-abi-outside-review/done-when.md).

## Finding status

- Item 1, P0, clippy red on `VisionFive 2`: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 2 and decision A, P0, the published name: Done, [#4013](https://github.com/nifeos/nife/pull/4013), step 3 of
  milestone 1121 (the Moho rename: the order in which the binary interface takes its name). The
  package, the directory and every use are `moho_abi`, as §323 (the binary interface, and the names
  of the layers) ruling 2 rules.
- Item 3, P1, the crate names nife and nife programs: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 4, P1, twenty-eight references into nife: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 5, P1, the virtio error that does not exist: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 6, P1, the register convention: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 7, P1, three self-contradictions: Done, (a) and (c) in [#3565](https://github.com/nifeos/nife/pull/3565), (b) in [#3596](https://github.com/nifeos/nife/pull/3596).
- Item 8, P1, dates and rulings as history: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 9, P1, README prose gates and badge row: At the move. The move commit adds the badge row
  with the repository's name; the prose passes the template's gates ([#3565](https://github.com/nifeos/nife/pull/3565), [#3942](https://github.com/nifeos/nife/pull/3942)).
- Item 10, P1, the manifest's publish comment: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 11, P1, the version gate: Done, [#3942](https://github.com/nifeos/nife/pull/3942), on [#3565](https://github.com/nifeos/nife/pull/3565)'s counting test.
- Item 12, P1, `futex::admitted` and `Error::from_ret`: Done, [#3942](https://github.com/nifeos/nife/pull/3942).
- Item 13, P1, names not ready and no package: Done, [#3942](https://github.com/nifeos/nife/pull/3942); decision C clears
  [the package](../../notes/crate-reviews/moho-abi.md), and the rename has landed.
- Item 14, P2, `Error` lacks `Display`, `Error` and `Hash`: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 15, P2, `CapabilitySlot` has no user: Done, [#3942](https://github.com/nifeos/nife/pull/3942).
- Item 16, P2, `RegisterShape::new`'s parameters: Done, [#3942](https://github.com/nifeos/nife/pull/3942).
- Item 17, P2, `CycleMeaning` is not a system call contract: Done, [#3942](https://github.com/nifeos/nife/pull/3942).
- Item 18, P2, doc nits: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 19, P2, `Name:` blocks quote the decider: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 20, P2, six survivors with no record: Waiting on the new repository's first weekly mutation
  run.
- Item 21, P2, no machine checks: Waiting on the new repository's first proofs run.
- Decision B, `console::WRITE` stays reserved: Done, [#3596](https://github.com/nifeos/nife/pull/3596).
- Found at the finish, six size names without a unit (`address_wait::SIZE_U8` to `SIZE_U64`,
  `SIZE_MASK`, `usage::SIZE`): Done, [#3998](https://github.com/nifeos/nife/pull/3998), as decision C ratifies them.
- The move: Decision C clears the package now that step 3 of milestone 1121 has landed
  ([#4013](https://github.com/nifeos/nife/pull/4013)), so the repository may be created.
- Publishing: Waiting on the last outside-review milestone closing (ruled 2026-11-18 UTC).

## Follow-on

- **Done.** Decision B: `console::WRITE` stays as a reserved operation (#3596).
- **Done.** Decision A's rename to `moho_abi`, milestone 1121 step 3, [#4013](https://github.com/nifeos/nife/pull/4013).
- **Done.** The six ratified size names, [#3998](https://github.com/nifeos/nife/pull/3998).
- **Outstanding.** The repository, then items 9, 20 and 21 at and after the move.
  The finding status above says what each needs.

## BUGS

- Item 6's x86_64 column for the fourth and fifth words comes from reading the runtime's aarch64 and riscv64 traps and the x86_64 `START` doc. The lane that writes the table reads the
  x86_64 trap and the kernel's entry before it writes a register name.
- Item 20 names three survivors of six; the other three are what the first weekly run reports, and
  this block does not guess them.
- Item 21 does not know whether the proofs job is green with no harness; the lane that stages the
  move finds out on the first push.
- The coverage figure was not measured; every function in the crate has a test that calls it, so the 85 floor should pass, unverified.

## Index row

An outside review of `abi` before its move found these problems. One lint is red on arrival. Another crate holds the crates.io name. Six nife references and twenty-eight internal citations need a rewrite. The docs name an error the kernel never answers. The root states the register convention too narrowly. A version gate stays behind in nife. Two function names break the rules. 130 names lack a ratified block, and no review package exists. The architect ruled the published name `moho_abi`. A dead method's number stays reserved.
