---
status: PROPOSED
raised: 2026-11-17
milestone_dependencies: 304, 323
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# Attested falsifications replay where they compile

Raised 2026-11-17 (UTC) by lane `audit/2026-11-17-the-verifier`, as finding 3 of the security audit
[the verifier](../../audit-reports/2026-11-17-the-verifier.md).

Reuse: `script/falsifications --sweep-attested` and the `sweep-x86_64` job in
`.github/workflows/falsifications.yml`, which already replay attested records that carry a patch;
the patched Kani `script/verify-riscv64` builds and caches.

Name: provisional. Every new name below is the lane's, not a ruling.

## The problem

A Kani harness compiles for the host, and the sweeps run on arm64, so every architecture-specific
harness records `attested`. Four of the nine such records carry a patch, and the weekly x86_64 job
replays them. Five carry none, and no job shows that their proofs can still fail:

- the three riscv64 MMU harnesses in `kernel/src/arch/riscv64/mmu/proofs.rs`, which only the patched
  Kani proves;
- `every_cpacr_edit_changes_exactly_the_field_it_names` in `kernel/src/arch/aarch64/fp.rs`;
- `no_vector_belongs_to_two_bands` in `kernel/src/arch/x86_64/interrupt_controller.rs`.

A Kani bump, or a change to `patches/kani-0.67.0-riscv64-target.patch`, could blind any of these
five and nothing would notice.

Writing the patches is not enough alone. `--sweep-attested` takes every attested record that has a
patch, whatever its architecture, so an aarch64 or riscv64 patch would turn the x86_64 job red.

## The work

1. Each of the five records gains its patch at the conventional path, the edit its prose already
   describes, and names it in backticks.
2. `--sweep-attested` replays only the records whose patch path names the host's architecture
   (`arch.<arch>.`), and with `--target riscv64` the riscv64 ones through the patched Kani.
   Provisional: this reads the host from the path the convention already writes, and adds no
   per-record field.
3. `falsifications.yml` gains an arm64 leg and a riscv64 leg beside `sweep-x86_64`.
4. The `BUGS` entry in `script/falsifications` on attested arch records says what now replays them.

## For the architect, if the path is not enough

The `BUGS` entry calls a per-record statement of the host a format change, and the architect's.
Step 2 avoids that by reading the path. If a record ever needs a host its path cannot name, that is
the point to ask.

## Done when

The weekly falsification run replays all nine attested arch records on hosts that compile them, and
each goes red under its patch.

## Index row

Every attested architecture-specific falsification carries a patch and is replayed weekly where it
compiles.
