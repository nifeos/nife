# The arch audit: candidates cleared, and why each is safe

*Name: provisional. After the parent's own heading.*

An appendix to [`notes/arch-audit.md`](../arch-audit.md), from the audit of 2026-07-29 (UTC).

Recorded because "we looked and it is fine" is the other half of an audit, and because each of these
is a place a future change could break something.

aarch64 `RESTORE_CONTEXT` after the fix: safe by construction, claim verified. The only writes
between `msr daifset, #0xf` and the `eret` are to `SPSR_EL1`, `ELR_EL1`, `SP_EL0`, and the general
registers. None of them can alter live PSTATE, so interrupts stay masked for the whole sequence
regardless of what the frame contains. (`msr daifset` needs no `isb`; the masking is architecturally
effective for subsequent instructions, which is why Linux's `local_daif_mask` is the same bare
instruction.) This is the claim finding 1 shows does *not* hold on RISC-V.

Nothing stages `SPSR_EL1`/`ELR_EL1` or `sepc`/`sstatus` outside the restore macro. One `eret`,
one `sret`, both inside it. Two frame-fabrication sites, both in Rust, both audited.

`SP_EL0` in `RESTORE_CONTEXT`. Written early, alongside `SPSR`. Harmless: the kernel runs at EL1
with `SPSel = 1`, so `SP_EL0` is dead to it, and vector slots 0..3 ("Current EL, SP_EL0") are
unreachable.

The context switch, both ISAs. `switch_to` saves only callee-saved registers and swaps `sp`, and
`schedule()` masks interrupts across the whole decision *and* the switch, so there is no window for
the timer to decide twice. The two trampolines deliberately do **not** unmask before
`finish_switch`, which is a previously-found bug with its own account in
[threads.md](../threads.md); both files still carry the comment explaining why, and both are still
correct.

The RISC-V `tp` migration hazard, already fixed this session: verified. `trap_return` restores
`x4` (`tp`) from the frame only for a return to U-mode, and keeps the live value for a return to
S-mode, because a preempted kernel thread can migrate harts and its frame's `tp` names the hart it
left. The conditional reads `t0`, which still holds the frame's `sstatus` from the CSR write above,
and tests `SPP`. Correct. aarch64 cannot have this bug: its per-CPU pointer is `TPIDR_EL1`, a system
register the frame never carries, which is why `percpu_matches_hart` is a constant `true` there.

Sweeping for the same shape more generally, nothing else round-trips a hart-local value through a
saved frame. `switch_to` preserves `tp`/`TPIDR_EL1` across a migration by not touching them;
`finish_switch` re-reads `cpu::current()` after the switch rather than using a value cached before
it; `this_s_context()` is computed from `cpu::id()` inside the masked handler and used only there.

`set_percpu` on RISC-V: a real window, unreachable. It writes `tp`, then the stash's `percpu`,
then points `sscratch` at the stash. In between, `tp` names the new block while `sscratch` still
names the old one (or is 0 on the very first call, which would make the trap entry store through a
null `&TrapStash`). It is unreachable because of where it is called from: `cpu::init_this_cpu` is the
first thing `kernel_main` and `secondary_main` do, before the trap vector is installed
(`arch::init()` is the next step) and long before that hart enables interrupts. Nothing can trap
there. Worth knowing that the precondition is stronger than the one `cpu.rs` documents ("before that
core takes its first lock"): the real requirement is *before that hart can take a trap at all*. CPU
hotplug, or any future re-pointing of a live hart's per-CPU block, would make this reachable and
catastrophic.

MMU enable and the TTBR/SATP switches. `boot.s` (both ISAs) and `mmu::install` run on a hart that
has no trap vector yet and no interrupts, so the MMU-enable sequences are uninterruptible by
construction. `set_ttbr0` is a single register write bracketed by `dsb`/`isb`: there is no staging to
interrupt, and an exception landing mid-sequence is itself context-synchronizing while the trailing
barrier still executes on resume. RISC-V's `write_satp` (`csrw satp; sfence.vma`) is likewise safe
even when interrupted, because every process root carries the kernel high half (`share_kernel_half`),
so the handler translates fine, and the `sfence.vma` simply executes when the instruction stream
resumes. `init_secondary`'s `TTBR1_EL1.set_baddr` is a read-modify-write, but of a per-core
register on a hart that has not enabled interrupts yet.

Barriers where a system-register write must take effect before a dependent instruction. Checked
each one: `VBAR_EL1` + `isb` (present, and `exceptions.md` already explains why); `SCTLR_EL1`
MMU-enable + `isb`; `TCR`/`MAIR`/`TTBR1` + `isb` before the TLB work; `set_ttbr0`'s `dsb`/`isb`;
`satp` + `sfence.vma`; `sync_icache`'s clean/invalidate/barrier sequence on aarch64 and `fence.i` on
RISC-V. Two that legitimately need none: `CNTKCTL_EL1` (its dependent instruction is a userspace
counter read, after an `eret`, which is context-synchronizing) and `csrw stvec` (an ordinary CSR
write, effective in program order).

`PAR_EL1` in `translate_as_el0`. A genuine single-copy staging window, `at s1e0r` then `mrs
par_el1`, and it is already masked, with a comment naming this exact class. It is also
`#[allow(dead_code)]` with only test callers today. This is the precedent that shows the discipline
predates the bug; the gap was that `RESTORE_CONTEXT` never got the same treatment.

GIC acknowledge/EOI pairing. Cannot cross cores: `IAR` and `EOIR` live in the **banked** per-core
CPU interface, so each core acknowledges and completes its own. Within a core the handler runs with
interrupts masked, and the EOI is written before the deferred `schedule()`, which is what keeps the
GIC from refusing equal-or-lower-priority interrupts to the thread we switch to.

PLIC claim/complete pairing. Cannot cross harts either, and this was worth checking explicitly
because the path was recently context-parameterized. The handler computes `ctx` once from
`this_s_context()` (`2 * cpu::id() + 1`), and claim, `disable`, and complete all use that same local.
No `schedule()` intervenes: `sched::irq_notify` never blocks or switches, it takes `SCHED`, signals
the endpoint, and at most sends a reschedule IPI to a remote hart. The deferred `schedule()` is at
the bottom of the dispatch, after complete. So the hart that claims is the hart that completes.

The timers. aarch64's `CNTV_CVAL_EL0`/`CNTV_CTL_EL0` are per-core, and `rearm`'s
read-read-write runs inside the handler with interrupts masked. `CNTKCTL_EL1`'s read-modify-write is
per-core and boot-only. RISC-V arms through an SBI `ecall`, which traps to M-mode and so touches
`mepc`/`mstatus`, never the `sepc`/`sstatus` pair the S-mode return path stages. The same is true of
the other SBI calls (`sbi_send_ipi`, `sbi_remote_sfence_vma`, `psci_cpu_on`): none of them can
disturb S-mode return state, which is worth knowing because an `ecall` in the middle of `trap_return`
would otherwise be exactly the bug.

Secondary bring-up, both ISAs. Each hart runs its own prologue on its own stack with interrupts
masked and, on RISC-V, before `stvec` exists. The one genuinely shared structure it touches, the
online mask and count, is published with `Release` after everything else is set up. The
`ONLINE_MASK` bit is set before the count, so a TLB shootdown that runs the instant the hart is
counted also targets it.

The cross-core inbox. The one place a core reaches into another core's `PerCpu` is
`cpu::inbox_of`, which is a real `IrqSafeMutex`. Everything else in `PerCpu` (`switched_from`,
`need_resched`, `runq_len`, `rng`) is touched by its owner, with the relaxed atomics serving as
interior mutability through a shared static rather than as cross-core synchronization.
