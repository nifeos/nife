---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: 956
decision_dependencies: 235, 311
machine_requirements: none
specific_machine: none
needs_person: no
---
# Message contracts carry bind-time version checks

Raised 2026-10-31 (UTC) by the lane `milestone/956-checkable-interface-versions`. calef ruled that day that milestone 956 (the linked interfaces carry checkable versions) stops at the constants, the `INTERFACES` note, the spawn-time ABI check and the enforcement, which is all of milestone 756's condition 4. The four steps below left 956 and wait for a trigger. Every name here is provisional.

Reuse: the steps reuse the tree's rustdoc-JSON `FORMAT_CONST` pattern (`helpers/interface_stability.py`, milestone 610 (the interface's stability is measured weekly)) and the `log_checks` assertions in `hot_swap_contract`. The shape is [milestone 979 (interfaces a peer can check)](../0979-version-carrying-interfaces.md)'s ruling, questions 2 and 4 and its compatibility section. No new dependency.

## Why this left 956

The first cut links `abi`, `address_space_map`, `glob`, `machine_statistics_contract` and `block_roster`. None of them is a message contract, so condition 4 of milestone 756 does not need a bind-time check, a dual path or a swap invariant. Building them now would put code in front of a consumer that does not exist. The trigger to raise this as a milestone is a message contract about to leave the tree. This proposal keeps clear of `filesystem_contract` while UFS2 replaces RedoxFS; that contract gets a version only after the replacement settles.

## The steps

1. The bind check becomes set membership in each message contract. The sketch from the shape proposal: `member(own: u32, served: &[u32]) -> Result<u32, Mismatch>`, with `Mismatch` carrying both versions. Each server gains the reserved operation, provisionally `OPERATION_VERSION` at opcode 0 where the protocol leaves it free, returning its served set. A client binds only when its declared version is in the set, and a refusal names both sides. The kernel routes these words without reading opcodes. Proves: a contract crate's tests bind a matching set and refuse a mismatched one, naming both versions.
2. The message-contract servers owe the current generation and the last (N-1). A break bumps the version, owes the dated ledger entry naming the migration, and carries a one-generation dual path. Shared pages stay single-version, and a page-format break rides the update path (§235's U4 coherent set). Stateful services ride the update path. Hot swap carries a version change only on an N-1 contract, only for a stateless component. The argv magic's digits move in the window where basalt rebuilds `rg` and every other foreign program that reads argv; basalt has no consumers yet, so that break owes no dual path. Proves: a contract crate's tests hold a dual-serving server and a single-generation one.
3. Retiring the legacy generation is a version event, never silent. It bumps the contract version, owes the dated ledger entry, and rides the update path, so in-tree v1 clients move in the same release. The refusal then fires on a stale client binding, naming both versions. The ledger records whether any client exercised the N-1 shim. Proves: the retirement lands in one release with its ledger entry, and the stale-client refusal fires.
4. The swap test's two invariants generalize into the contract crates' tests: the version per sequence number never goes backwards (`log_checks::MONOTONE`), and both versions appear (`log_checks::BOTH_VERSIONS`). A dual-serving contract's test asserts both across its own swap. Proves: the contract crates' tests and the Kani property pass, and 610's next series reads the new constants.

## Open: where `member` and `Mismatch` live

One copy per contract crate repeats a ten-line function a dozen times, against rule 7's one definition. One small shared crate (provisionally `interface_version`) holds the function and the error once. No crate name exists until a message contract is about to leave the tree, and that name is the architect's call. The question stays open here.
