---
status: PARTIAL
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
carries a checkable version, and the moved package declares what it links.

Reuse: the plan reuses machinery that exists, per AGENTS.md rule 6. It takes the `abi` crate's
tests, and milestone 597 (a program carries its manifest in an ELF note), the manifest-note encode
and decode with its one-encoding Kani property. It takes the `FORMAT_CONST` pattern in `helpers/interface_stability.py`. No new
dependency. The weekly series refused `cargo-semver-checks` and `cargo-public-api`, and they stay refused.

## The plan, four steps

Each step lands a provable piece. Every new name is provisional.

1. The version constants land beside the wire formats the first cut links. `abi` carries
   `ABI_VERSION` at the crate root; `address_space_map` and `glob` each carry `VERSION`;
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

- `abi`, `address_space_map`, `glob`, `machine_statistics_contract` and `block_roster` each export
  a version constant and an `INTERFACE_ID`, and the id check finds no duplicate.
- `capability_manifest_note`'s tests and its Kani property for the `INTERFACES` note pass.
- The progenitor's tests refuse a program whose ABI entry differs from `ABI_VERSION` and accept one
  that matches.
- `script/lint` runs the pinned-values test and the format-values check, and the check's selftest
  passes.
- Each of the fifteen programs of `process-tools`, `core-tools` and `disk-tools` carries a
  `carry_interfaces!` list, and `python3 helpers/interface_declarations_check.py` finds each list
  equal to the versioned interfaces that program's dependency graph reaches.

These hold. Milestone 756's condition 4 holds once the set below is ruled and every interface in
it carries a version and an `INTERFACE_ID`.

## The linked set, re-derived

Milestone 756 lists four interfaces, measured on 2026-10-04; step 1 adds `block_roster`. Measured
again on 2026-11-01 (UTC): the crates each program names, closed over `cargo metadata`'s normal
dependencies, less the cut's own crates, the `carry_interfaces!` lists included. The fifteen
programs reach twenty-two crates. The five above carry an `INTERFACE_ID`. Seventeen do not:

- With a version constant already: `byte_sink_contract`, `calendar`, `capability_manifest_note`
  (one per descriptor), `clock_contract`, `counter_frequency_contract`, `current_cpu_contract`,
  `entropy_contract`, `environment_contract`, `filesystem_contract` (block and file),
  `globally_unique_identifier_partition_table`, `program_argument_contract`, `socket_contract`.
- With none: `domain_name_resolution_contract`, `domain_name_system`, `filesystem_fixture`,
  `grant_plan`, `user_mode_runtime`.

No ruling yet says whether condition 4 means all twenty-two, the five, or another rule. Pull
request #3022 asks it. The declaration check reads `INTERFACE_ID`, so each crate that gains one joins every
list that reaches it, and the check names the program that must add it.

## What this block does not do

The bind-time check in message contracts, the one-generation dual path, retiring the legacy
generation and the swap-test invariants are a separate proposal,
[the message contracts carry bind-time version checks](proposals/message-contracts-carry-bind-time-version-checks.md).
The first cut links no message contract, so condition 4 does not need them. The macro takes its interface list by hand. Deriving the list from the dependency graph at package
build waits for the first package that leaves the tree.

## Follow-on

- **Outstanding.** Which crates condition 4 covers: the five above, the twenty-two the
  re-derivation finds, or another rule. Checked on 2026-11-01 (UTC) with `cargo metadata` and
  `helpers/interface_declarations_check.py`; asked on pull request #3022 with `needs-architect`.
- **Outstanding.** An `INTERFACE_ID` and a version for each crate the ruling adds, then the
  programs' lists grow as the check names them.

## BUGS

- The kernel's own build makes the programs the spawn path takes from the archive, and that path
  does not read their `INTERFACES` note; only the image path checks it.
- The declaration check reads source text and the dependency graph, not the linked bytes. A crate
  a program reaches only through another crate's macro counts as that other crate's.

## Index row

Every interface the first cut links carries a checkable version, and the moved package declares
what it links through an `INTERFACES` manifest note. That is condition 4 of milestone 756's first
cut, and the check the weekly series counts.
