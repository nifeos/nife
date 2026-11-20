# Milestone 589's probe transcripts

Moved verbatim from [`../0589-kani-can-target-riscv64-from-the-hosts-we-have.md`](../0589-kani-can-target-riscv64-from-the-hosts-we-have.md)'s "The answer" on 2026-10-13 (UTC), to bring that
block under §212 (a prose budget)'s cap by the graph's whole-file count. Both runs are a patched Kani 0.67.0 on
patagonia (aarch64 macOS, stock CBMC 6.8.0), as the block says.

## The reach probes

The second harness fails on purpose: `flush_asid` runs `sfence.vma`, and Kani reports the reachable
`asm!` as unsupported rather than refusing to compile.

```console
$ KANI_TARGET=riscv64gc-unknown-linux-gnu cargo-kani -p kernel -Z unstable-options \
    --ignore-global-asm --output-format=terse
Checking harness arch::riscv64::mmu::riscv64_reach_probe::reaches_sfence...
Failed Checks: TerminatorKind::InlineAsm is not currently supported by Kani. Please post your example at https://github.com/model-checking/kani/issues/2
 File: "kernel/src/arch/riscv64/mmu.rs", line 681, in arch::riscv64::mmu::flush_asid
VERIFICATION:- FAILED
Checking harness arch::riscv64::mmu::riscv64_reach_probe::satp_asid_round_trips...
VERIFICATION:- SUCCESSFUL
Checking harness system_call::proofs::every_page_between_the_checked_ends_is_itself_a_user_page...
VERIFICATION:- SUCCESSFUL
Checking harness system_call::proofs::the_run_end_is_exact_and_refuses_exactly_what_does_not_fit...
VERIFICATION:- SUCCESSFUL
Complete - 3 successfully verified harnesses, 1 failures, 4 total.
        7.27 real         2.27 user         0.68 sys
```

## The stub probe

`read_satp` stubbed with a model, proving `current_root_pa` extracts the root it was given.

```console
$ KANI_TARGET=riscv64gc-unknown-linux-gnu cargo-kani -p kernel -Z unstable-options -Z stubbing \
    --ignore-global-asm --harness current_root_is_the_ppn_field
Checking harness arch::riscv64::mmu::riscv64_stub_probe::current_root_is_the_ppn_field...
VERIFICATION:- SUCCESSFUL
Complete - 1 successfully verified harnesses, 0 failures, 1 total.
```
