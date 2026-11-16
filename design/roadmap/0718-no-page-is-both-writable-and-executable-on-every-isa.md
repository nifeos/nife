---
status: BUILT
raised: 2026-10-03
built: 2026-10-03
promoted_from: no-page-is-both-writable-and-executable-on-every-isa
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 718. No page is both writable and executable, proved on every ISA

**Built 2026-10-03 (UTC).** Promoted from the proposal of the same name in `design/roadmap/proposals/`. Milestone 633 (An outside agent attacks the confinement claim) raised it in PR #1525, now merged. The number 718 is provisional until the queue lands it. *(Title and slug are drafts.)*

Row 11 of `notes/confinement-claims.md` was proved on x86_64 only. §19 (architectural parity is a tenet) makes a capability a gate on every ISA, so two Kani harnesses close the gap. Each is stated in raw-bit literals, over all eight `Flags` constructors and any masked physical address:

- `page_tables::aarch64::verification::no_encoded_leaf_is_both_writable_and_executable`: a leaf with `AP[2]` (bit 7) clear carries both `PXN` (bit 53) and `UXN` (bit 54).
- `page_tables::sv39::verification::no_encoded_leaf_is_both_writable_and_executable`: a leaf with `W` (bit 2) set has `X` (bit 3) clear.

Both verify in under a second. Each has a replayable falsification under `crates/page_tables/falsifications/` (aarch64 drops `PXN` on a writable page; Sv39 sets `X` on a writable page), and `script/falsifications --sweep paging` returns red on all three W^X harnesses. Row 11 now names three harnesses and three records.

No counterexample was found: both encoders already keep write and execute apart.

## Follow-on

- **Recorded.** The proof is over the encoder from `Flags`. It does not cover a leaf written by other means (a hand-built PTE word); row 22's two kernel tests cover the ELF loader layer.

## Index row

Row 11 of the confinement claims (no page is both writable and executable) was proved on x86_64 alone. It is now a Kani proof with a replayable falsification on aarch64 and Sv39 too. So §19 (architectural parity is a tenet) holds for a security claim and not only for features.
