---
status: PROPOSED
raised: 2026-10-29
milestone_dependencies: 597, 610
decision_dependencies: 235, 311
machine_requirements: none
specific_machine: none
needs_person: no
---
# The linked interfaces carry checkable versions

Raised 2026-10-29 (UTC) by the lane `proposal/version-carrying-interfaces`, as the implementing
proposal behind [the shape proposal](version-carrying-interfaces.md). calef ruled the shape that
day in PR #2852's comment; this proposal re-aims the eight-step plan at the ruled shape and
proposes it for the merge queue, milestone number provisional until an integrator mints one. The
shape proposal holds the questions, the options and the ruling; this one holds only the plan.

The gate the plan serves is milestone 756's condition 4 as re-aimed on 2026-10-28: each interface
the first cut links carries a checkable version, and the moved package declares what it links.

Reuse: the plan reuses machinery that exists, per AGENTS.md rule 6. The `abi` crate's
pinned-values test, milestone 597's manifest-note encode and decode with its one-encoding Kani
property, the `log_checks` assertions in `hot_swap_contract`, and 610's `FORMAT_CONST` pattern in
`helpers/interface_stability.py`. No new dependency; `cargo-semver-checks` and `cargo-public-api`
stay refused as milestone 610 records.

## The plan, eight steps

Each step lands a provable piece and names the ruled part it executes. Every new name is
provisional.

1. The version constants land in the four interfaces the first cut links. `abi` carries
   `ABI_VERSION` at the crate root; `address_space_map` and `glob` each carry `VERSION`;
   `machine_statistics_contract` gains a version word beside its `MACHSTA1` magic, a new index in
   the crate's `word` module. One constant per wire format, module-scoped where a crate holds
   several, as ruled (question 1). Proves: the constants are public, and 610's `FORMAT_CONST`
   pattern reads them, so the next weekly series counts them.

2. The bind check becomes set membership, in each message contract. The sketch from the shape
   proposal: `member(own: u32, served: &[u32]) -> Result<u32, Mismatch>`, with `Mismatch` carrying
   both versions (question 2). Each server gains the reserved operation, provisionally
   `OPERATION_VERSION` at opcode 0 where the protocol leaves it free, returning its served set. A
   client binds only when its declared version is in the set; a refusal names both sides. The
   kernel routes these words without reading opcodes; the check belongs to the two endpoints.
   Proves: the contract crate's tests bind a matching set and refuse a mismatched one, naming both
   versions.

3. The syscall ABI check runs at spawn. The progenitor compares the manifest note's declared ABI
   version against the kernel's `ABI_VERSION` (§235), and refuses a mismatch naming both. The ABI
   never dual-serves; the refusal is the hard edge. Proves: the progenitor refuses a program whose
   note declares a version the kernel does not serve, and accepts one it does.

4. The manifest note gains descriptor version 3 (question 3): version 1's 56 bytes unchanged, then
   a counted (interface, version) list. Each interface crate carries `INTERFACE_ID: u64` beside
   its version constant, values never reused. The stamp is mechanical at package build, derived
   from the dependency graph. Versions 1 and 2 stay untouched. Proves: the encode, decode and
   one-encoding Kani property carry over from 597's gate, and a built package's note decodes to
   the interfaces its link graph names.

5. The message-contract servers owe the current generation and the last (N-1, the compatibility
   ruling). A break bumps the version, owes the dated ledger entry naming the migration, and
   carries a one-generation dual path; shipping the next generation closes the window. The
   carve-outs hold: shared pages stay single-version, and a page-format break rides the update
   path (§235's U4 coherent set). Stateful services ride the update path. Hot swap carries a
   version change only on an N-1 contract, only for a stateless component. The operator verifies
   that the new instance serves every bound client's version. Proves: a contract crate's tests
   hold a dual-serving server and a single-generation one, per the carve-outs.

6. Retiring the legacy generation is itself a version event, never silent. It bumps the contract
   version, owes the dated ledger entry naming the migration, and rides the update path, so
   in-tree v1 clients move in the same release while the compiler proves the move. The refusal
   then fires on a stale client binding, naming both versions. The ledger records whether any client
   exercised the N-1 shim, the recorded evidence for revisiting the N-1 rule. Proves: the retirement
   lands in one release with its ledger entry, and the stale-client refusal fires.

7. The enforcement lands now (question 5): the `abi` test spells every public number as an
   expected literal, and `script/lint` gains the diff-grep check that fails a pull request which
   changes a magic or version value without its constant. The rustdoc diff waits for a consumer
   outside the tree. Proves: a renumber without the test's edit fails CI, and the lint fires on a
   synthetic pull request that moves a value.

8. The swap test's two invariants generalize into the contract crates' tests: the version per
   sequence number never goes backwards (`log_checks::MONOTONE`), and both versions appear
   (`log_checks::BOTH_VERSIONS`). A dual-serving contract's test asserts both across its own swap.
   Proves: `script/lint` green, the contract crates' tests green, the Kani property green, and
   610's next series reads the new constants; 756's condition 4 is checkable by command.

## Dependencies and order

Steps 1 through 3 stand on their own and land first. Step 4 depends on step 1, the constants the
stamp reads, and on 597's note machinery, which exists. Steps 5 and 6 depend on steps 1 and 2.
Step 7 depends on step 1. Step 8 depends on steps 2, 5 and 6. The milestone follows 610's series,
so the first week of constants reads as additions, a shape the series already records.

## What this proposal does not do

This proposal writes no code. §311 (a contract is code, not prose) reserves the checks for an
implementing lane; the sketches above are the shape proposal's, restated as the plan's content.
The implementing lane writes and proves them.
