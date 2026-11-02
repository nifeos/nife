---
status: BUILT
built: 2026-11-01
raised: 2026-10-29
promoted_from: the-linked-interfaces-carry-checkable-versions
milestone_dependencies: 597, 610
decision_dependencies: 235, 311
machine_requirements: none
specific_machine: none
needs_person: no
---
# 956. The linked interfaces carry checkable versions

Raised 2026-10-29 (UTC) by the lane `proposal/version-carrying-interfaces`, the implementing
block behind [milestone 979 (interfaces a peer can check)](0979-version-carrying-interfaces.md).
calef ruled the shape that day in PR #2852's comment, and ruled on 2026-10-31 (UTC) that this block
covers condition 4 of milestone 756 and nothing more. The shape proposal holds the questions, the
options and the ruling; this block holds the plan.

The gate the plan serves is condition 4 of milestone 756 (`process-tools`, `core-tools` and `disk-tools` are the first
code to leave this repository). Each interface the first cut links
carries a checkable version, and the moved package declares what it links. Ruled 2026-11-01 (UTC)
on pull request #3022: an interface here is a crate that defines a run-time format exchanged
between separately released parts. Every other linked crate is a Rust API, which its Cargo version
and a semver check guard once it is versioned, and nothing declares it. Interfaces need not stop
breaking; the cut needs the mechanisms that accommodate a break, which are checkable versions.

Reuse: the plan reuses machinery that exists, per AGENTS.md rule 6. It takes the `abi` crate's
tests, and milestone 597 (a program carries its manifest in an ELF note), the manifest-note encode
and decode with its one-encoding Kani property. It takes the `FORMAT_CONST` pattern in `helpers/interface_stability.py`. No new
dependency. The weekly series refused `cargo-semver-checks` and `cargo-public-api`, and they stay refused.

## The plan, four steps

Each step lands a provable piece. Every new name is provisional.

1. The version constants land beside the wire formats the first cut links. `abi` carries
   `ABI_VERSION` at the crate root; `address_space_map` carries `VERSION`;
   `machine_statistics_contract` carries a version word beside its `MACHSTA1` magic, an index in the
   crate's `word` module; `block_roster` carries `VERSION` in the digits of its magic. One constant
   per wire format, as ruled (question 1). `program_argument_contract`'s `nifeargv` stays
   unnumbered: basalt's published `rg` reads an argv page from the prebuilt archive the gates
   fetch, so its digits move with that rebuild. Proves: the constants are public, and the
   `FORMAT_CONST` pattern reads them.

2. A package declares what it links in a new manifest-note type, `INTERFACES = 4`, on the pattern
   of `SUBTREE_GRANTS` and `VERBS` (calef, 2026-10-31 UTC; descriptor versions 1 to 3 stay
   untouched). Its descriptor is a version word, a count, then that many records of
   `INTERFACE_ID: u64` and a version `u32`, in id order with no duplicates. Each interface crate
   carries `INTERFACE_ID: u64` beside its version constant, and no value repeats. A program carries
   the note through a macro that reads the constants, so a declaration cannot drift from them. Proves: the encode, decode and one-encoding Kani property carry over from 597's gate,
   and no two crates share an id.

3. The syscall ABI check runs at spawn. The progenitor reads the ABI entry of a program's
   `INTERFACES` note and compares it with the kernel's `ABI_VERSION` (§235 (the OS is built and
   updated from packages, and the tree divides by what releases together)). It refuses a mismatch,
   naming both. A program with no `INTERFACES` note declares nothing and runs as it does today;
   refusing it waits on packages carrying the stamp. The ABI never dual-serves, so the refusal is
   the hard edge. Proves: the progenitor refuses a program whose note declares a version the kernel
   does not serve, and accepts one it does.

4. The enforcement lands now (question 5). The `abi` test spells every public number as an expected
   literal. `script/lint` gains the diff-grep check that fails a pull request which moves a magic
   or a public ABI number without its version constant. The rustdoc diff waits for a consumer
   outside the tree. Proves: a renumber without the test's edit fails CI, and the check fires on a
   synthetic pull request that moves a value.

## Order

Step 2 depends on step 1, the constants the note reads, and on 597's note machinery. Step 3 depends
on step 2. Step 4 depends on step 1. The milestone follows the weekly series, so the first week of
constants reads as additions, a shape the series already records.

## Done when

All of these hold, each checkable by a command:

- Every interface in the table below exports a version constant and its `INTERFACE_ID`, and
  `python3 helpers/interface_declarations_check.py` finds each number where the table puts it,
  none twice and none retired.
- `capability_manifest_note`'s tests and its Kani property for the `INTERFACES` note pass.
- The progenitor's tests refuse a program whose ABI entry differs from `ABI_VERSION` and accept one
  that matches.
- `script/lint` runs the pinned-values test and the format-values check, and the check's selftest
  passes.
- Each of the fifteen programs of `process-tools`, `core-tools` and `disk-tools` carries a
  `carry_interfaces!` list, and the same check finds every linked crate classified and each list
  equal to the interfaces the program links.
- The check's `OWED` table is empty.

With them, milestone 756's condition 4 holds.

## Interface ids

Each run-time format the first cut links carries the number below, at version 1 when assigned
(ruled 2026-11-01 UTC). A number belongs to one interface for good: a format change is a new
version, never a new number, and a retired number stays retired. A new interface takes the next
unused number by adding a row here; `script/lint` holds every `INTERFACE_ID` in the tree to this
table. No registry outside it exists.

| id | interface |
|---|---|
| 1 | `abi` |
| 2 | `address_space_map` |
| 3 | retired, `glob` |
| 4 | `machine_statistics_contract` |
| 5 | `block_roster` |
| 6 | `byte_sink_contract` |
| 7 | `capability_manifest_note` |
| 8 | `clock_contract` |
| 9 | `counter_frequency_contract` |
| 10 | `current_cpu_contract` |
| 11 | `domain_name_resolution_contract` |
| 12 | `entropy_contract` |
| 13 | `environment_contract` |
| 14 | `filesystem_contract::blk` |
| 15 | `filesystem_contract::fs` |
| 16 | `filesystem_fixture` |
| 17 | `grant_plan` |
| 18 | `program_argument_contract` |
| 19 | `socket_contract` |
| 20 | `user_mode_runtime` |

An interface is the module that holds the id, so `filesystem_contract` carries one in `blk` and
one in `fs`. The check also classifies every crate the cut links. Four define no run-time format
and stay Rust APIs: `calendar`, `domain_name_system`, `glob` and
`globally_unique_identifier_partition_table`. Its `NOT_A_FORMAT` table gives each reason.

Carrying the note links `capability_manifest_note`, whose `grant_plan::Manifest` type brings
`grant_plan` and `program_argument_contract` into five programs that do not otherwise link them,
so those five declare both.
Moving the note type into a leaf crate belongs to pull request #3026, ruled 2026-11-01 (UTC).

## What this block does not do

The bind-time check in message contracts, the one-generation dual path, retiring the legacy
generation and the swap-test invariants are a separate proposal,
[the message contracts carry bind-time version checks](proposals/message-contracts-carry-bind-time-version-checks.md).
The first cut links message contracts (`byte_sink_contract`, `entropy_contract`,
`filesystem_contract`, `socket_contract`, the resolver's); condition 4 asks each for a checkable
version, which a constant and an id give, and leaves the run-time check at bind to that proposal. The macro takes its interface list by hand. Deriving the list from the dependency graph at package
build waits for the first package that leaves the tree.

## Follow-on

- **Done.** The interface ids for the fourteen formats owed one, and the programs' lists, carried
  by this block's own `Interface ids` table and the declaration check.
- **Recorded.** The `INTERFACES` note type in a leaf crate of its own, so carrying the note
  links no planner (ruled 2026-11-01 (UTC) on pull request #3026; the order is in
  `design/roadmap/proposals/the-contract-crates-leave-in-this-order.md`). The declaration check
  names `capability_manifest_note::` as the macro's crate in two places and must classify the new
  crate; both move with the crate.

## BUGS

- The kernel's own build makes the programs the spawn path takes from the archive, and that path
  does not read their `INTERFACES` note; only the image path checks it.
- The declaration check reads source text and the dependency graph, not the linked bytes. A crate
  a program reaches only through another crate's macro counts as that other crate's.
- In a crate with an interface per module, a program declares the modules its source names. A
  program that names only a module without an id, such as `filesystem_contract::dir`, declares
  every interface of the crate.

## Index row

Every run-time format the first cut links carries a checkable version, and the moved package
declares what it links through an `INTERFACES` manifest note. That is condition 4 of milestone 756's first
cut, and the check the weekly series counts.
