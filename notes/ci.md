# CI job warnings, classified

*Name: provisional. Naming is an architect's (AGENTS.md); a lane ships a
provisional name and says so. The name is short for a note that starts with one job's warnings and
may later hold the next job's.*

Every CI job carries warnings that no gate denies. They scroll past green, so nobody has to read
them, and then somebody does. This note is the first such reading: the `swish-check` job, taken
whole, every warning classified against the tree's own rules as benign with a reason, already
tracked under a named record, or a defect fixed on the lane's branch.

The reading is from **run 37089019746, job 111105191050** (main's `swish-check`, commit
`d6a902a9b`, taken 2026-10-02 UTC). It replaces an earlier reading of run 36776276733; the one difference
found is the `calendar` manifest line, which #1370 has since removed.
Nothing in the job's trigger caused any of this; whatever it printed is the tree's, surfacing
wherever someone looks. The job's three architecture legs print overlapping
sets: the aarch64 and riscv64 legs each carry a manifest warning naming a dependency their build never
opens, and the x86_64 leg carries thirty-nine dead-code warnings. Counts below are what
`cargo check -p kernel --features shell` prints per target on this lane's branch before and
after its fixes.

## The classification

| warning, as the log prints it | location | verdict | action, or the record that owns it |
|---|---|---|---|
| `hint: ... will suppress this warning` (git's `init.defaultBranch` advice) | `actions/checkout`'s `git init` | benign | The runner's own git advises about its default branch inside checkout. Nothing in this repository runs that `git init`, and the tree cannot configure the runner's global git. |
| `warning: default toolchain implicitly overridden ... by rustup toolchain file` (twice, during `cargo install cargo-machete`) | `script/bootstrap` | benign | The pinned `rust-toolchain.toml` is doing exactly its job (see `rust-toolchain.toml`'s own header); rustup merely notes that a `cargo install` inherits it. Installing machete under the pinned nightly is wanted, not drift. |
| `warning: the following packages contain code that will be rejected by a future version of Rust: core v0.0.0` (twice) | the `nife-farm` sysroot, `script/ci-build` | benign | A build-std artifact: the tree compiles upstream nightly `library/core` from its own farmed sources ([std.md](std.md)), and the flagged code is upstream's, not this tree's. It moves with each toolchain bump, whose lane rebuilds the farm and runs the gates. |
| `warning: ... rejected by a future version of Rust: argon2 v0.6.0` | `credentialer`'s dependency graph | benign | Upstream crate's own code; this tree only links it. Measured 2026-09-30 (UTC): `cargo update --dry-run -p argon2` locks nothing, so no fixed release exists to move to. The pinned toolchain means it cannot become an error between bumps, and the nightly bump re-reads it. |
| `warning: unused dependency 'tock-registers'` (riscv64 and x86_64 legs) | `kernel/Cargo.toml` | already tracked | PR #1477 (kernel dependencies declared where used) gates it to aarch64. The `calendar` twin went with #1370. This lane no longer touches the manifest. |
| `function 'set_syscall_kernel_stack' is never used` | `arch/x86_64/exceptions.rs` | defect, fixed | Deleted. `isr_restore` in trap.s writes this slot and `TSS.RSP0` together on every return to ring 3, so the Rust setter lost its job to the asm. |
| `function 'set_kernel_stack' is never used` | `arch/x86_64/segments.rs` | defect, fixed | Deleted with its partner above, same commit, same reason. Pointer comments keep both names greppable. |
| `function 'interrupt_remapping_available' is never used` | `arch/x86_64/iommu.rs` | defect, fixed | Its allowance said bench only, and its comment claimed two callers. Reading the body: one caller, the module's own test, so it now carries `not(test)`. The correction commit records the stale comment. |
| `function 'is_enabled' is never used` | `arch/x86_64/mmu.rs` | defect, fixed | Deleted, with a pointer comment. No caller in any configuration on its only architecture; the boot print that asks "is paging on" is riscv's (`lib.rs`). |
| `constant 'IER_ERBFI'`, `methods 'configure' and 'enable_rx_interrupt' are never used` | `drivers/ns16550.rs` | defect, fixed | Live on riscv64, dead here: their callers are the device-tree console bring-up and `console::rx_enable`, both riscv paths. Each now carries a scoped `x86_64` allowance naming that. |
| `function 'plic_region' is never used` | `memory.rs` | defect, fixed | The PLIC is riscv's controller; the allowance now says so (inverted to `not(riscv64)`, since one of three uses it). |
| `function 'smmu_region' is never used` | `memory.rs` | defect, fixed | x86_64's VT-d init takes the DMAR's units, not one device-tree base (milestone 594 (every VT-d unit translates its own devices)); scoped allowance added. |
| `constant 'IRQ_CONTROLLER' is never used` | `sync.rs` | defect, fixed | The GIC and PLIC drivers are its only takers; the local APIC claims with one register read and takes no rank lock. Scoped allowance, and the doc now says the rank has two implementations, not three. |
| `function 'note_boot_stage' is never used` | `sched.rs` | defect, fixed | The allowance covered aarch64 because the riscv tour is the caller; x86_64 is in that same situation and is now in the list. |
| `constant 'RESCHED_SGI' is never used` | `sched.rs` | defect, fixed | Same shape: riscv64 was allowed, x86_64 sends its own reschedule IPI (`arch/x86_64/irq.rs`), so the allowance inverts to `not(aarch64)`. |
| `function 'raise_self_interrupt'`, `constant 'ICR_SELF'` are never used | `arch/x86_64/irq.rs` | already tracked | The caller is `sched`'s `raise_test_irq`, a `cfg(test)` helper of the kernel's own suite. The missing per-item allowance is exactly the debt [system-tests-and-the-kernel-crate.md](system-tests-and-the-kernel-crate.md) BUGS records: x86_64's allowances were added only where a gated build found them, and none did. |
| `constant 'XMM0' is never used` | `arch/x86_64/fp.rs` | already tracked | Its users are the `cfg(any(test, feature = "system_tests"))` test impl on `FpState`, twenty lines below. Same BUGS entry. |
| `function 'interval' is never used` | `arch/x86_64/timer.rs` | already tracked | Callers are the icount feature's tick math and the timer tests; the shell boot arms its tick inline. The aarch64/riscv64 twins stay silent because their files already carry per-item allowances. Same BUGS entry. |
| `function 'installed_port_grant' is never used` | `arch/x86_64/segments.rs` | already tracked | `system_tests`' x86 port tests read it under the `system_tests` feature; its own doc says "read for tests and diagnostics". Same BUGS entry. |
| fourteen in `x86_programs`: `pack_1/4/7/9/10/16`, `SPIN`, `port_out`, `port_out_then_exit`, `cap_delete_then_port_out`, `receive_then_port_out`, `receive`, `call`, `invoke_then_exit` are never used | `user/x86_programs.rs` | already tracked | `system_tests`' supervision, cpu-time, force-kill and x86 port suites build these; the boot tour calls only `report` and `fault`, which is why those two stayed silent. Same BUGS entry. |
| `function 'set_interrupt_stack' is never used` | `arch/x86_64/segments.rs` | defect found, proposal | Not dead weight but a missing wire, and the warning is how it was found: the IDT's vector-8 gate selects IST slot 1 (`exceptions.rs`), and `TSS.ist[0]` is only ever zeroed at init, so the documented double-fault safety net is not installed. A double fault today would load `rsp` 0 and triple-fault into a silent reset, the very outcome the IST entry exists to prevent. Fixing it wants a design call (which stack, and the per-vector exclusivity its Safety contract demands), so this branch keeps the setter and records the proposal, now filed as the [proposal](../design/roadmap/0717-x86-64s-double-fault-runs-on-a-stack-of-its-own.md) (#1496). |
| `enum 'GuardPage'`, `guard_page_at` (stack and interrupt_stack), `thread_stack_site`, `warn_if_guard_page`, `print_text_words`, `stack_area_span`, `is_mapped` are never used (eight warnings) | `stack.rs`, `interrupt_stack.rs`, `thread.rs`, `arch/x86_64/mmu.rs` | defect found, proposal | One chain, one cause: `warn_if_guard_page` is the aarch64 and riscv64 fault handlers' guard-page diagnostic, and x86_64's trap path never calls it, so everything under it is dead here. The stacks themselves are live on x86_64 (`top_for_trap` is called); it is the naming diagnostic that is missing, a gap in DECISIONS §19 (architectural parity is a tenet). Wiring it into the x86_64 page-fault path is the proposal; the warnings stay until that call is made, because an allowance here would launder a parity gap into "fine". Filed as the [proposal](../design/roadmap/0716-x86-64-names-a-kernel-stack-overflow-the-way-aarch64-and-riscv64-do.md) (#1496). |

## What the lane's branch changes, and what it leaves

Four purposes: the two stale cross-arch allowances in `sched.rs`, the arch-split items (with one
deletion among them), the obsolete ring-3 stack setter pair, and the iommu allowance. Verified
with `cargo check -p kernel --features shell` on all three targets: aarch64 prints nothing, and
x86_64 falls from forty warnings to twenty-nine (twenty-eight in CI, see BUGS). The manifest
warning that remains on riscv64 and x86_64 is #1477's. Every remaining dead-code one is in the two rows above that end in "already
tracked" or "proposal"; each carries its record.

The two proposals this reading leaves for an architect:

1. **Arm the double-fault IST** ([filed](../design/roadmap/0717-x86-64s-double-fault-runs-on-a-stack-of-its-own.md)). `set_interrupt_stack` exists, the IDT selects its slot, and
   nothing installs the stack. Any fix must name its stack and hold the no-shared-slot rule the
   function's Safety contract states.
2. **Wire the guard-page diagnostic into x86_64's trap path** ([filed](../design/roadmap/0716-x86-64-names-a-kernel-stack-overflow-the-way-aarch64-and-riscv64-do.md)), or scope-note its absence. The
   other two architectures name a guard-page fault before the machine reports it as an address
   nothing recognizes; x86_64 has the stacks and skips the naming.

And the sweep the BUGS entry already owns: the per-item allowances for x86_64's test-called items,
mirroring the aarch64 twins. This note's table is that sweep's worklist, with each item's caller
already read.

## BUGS

- The reading is one job on one day. The classes here are structural, but a second job's log could
  carry warnings this one did not print; the note does not claim to be a census.
- The x86_64 leg's dead-code count is thirty-nine in CI and forty locally. The extra local one is
  `timer::spin_for`, which the job's log never printed. Its callers are in `lib.rs` and the preemption
  tests; why the CI leg's feature set uses it and the local `shell` check does not was not chased,
  so it is unclassified here beyond that.
