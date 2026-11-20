# Milestone 161 appendix: ring 3 and the `system_call` pair, step 11

An appendix to [milestone 161 (the x86_64 kernel port)](../0161-x86-64-kernel-port.md), moved out
of that page on 2026-10-12 (UTC) under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md).
The main page is complete without it; this is the record of step 11 (2026-08-24, the open list's
item 3) and of that item's own text, which the page now summarizes. Long sentences were split to
meet [§213 (writing standards)](../../decisions/0213-writing-standards.md); no fact changed.

## What was built

Ring 3, the `system_call` pair, and a program the page tables refuse. Four pieces, and the order below
is the order they have to work in.

### The user address spaces

`arch/x86_64/mmu.rs`'s user block stopped being a wall of `unimplemented!()`. x86 is RISC-V's
single-root model, not aarch64's: `CR3` names the whole address space. So `share_kernel_half` copies
PML4 entries 256..512 out of the kernel root into every process root. That shares the image
(PML4[511]), the direct map (PML4[273]) and every device window in one `copy_from_slice`. Without it
the `mov cr3` unmaps the instruction after it.

### The `swapgs` pair, guarded on the interrupted CPL

The guard is the whole design. `swapgs` is an exchange. A trap from ring 0 that swapped would
install the user's GS base while running kernel code, and the next nested trap would hand the
kernel's back to the user. Both sites test the saved `CS`'s low two bits, which are the interrupted
privilege level.

It does not trip the hazard `segments.rs` documents: loading a segment register in long mode zeroes
that segment's base MSR. This code writes the MSR pair and loads no selector. That was checked
against the instruction's definition rather than assumed. It is exactly the shape of bug that cost
this port an afternoon once.

### The `system_call` entry

Four MSRs are programmed in `arch::init`, beside the GDT and the IDT, because they are per-CPU state
of the same kind. They are `IA32_STAR`, `IA32_LSTAR`, `IA32_FMASK`, and `EFER.SCE` last. Setting
`EFER.SCE` last makes the instruction legal only after the address it jumps to exists.

The entry path is the part with no counterpart on the other two architectures. `system_call` does not
switch stacks and pushes nothing. So the first three instructions are `swapgs`, park the user's
`rsp`, and load the kernel's. `segments::set_kernel_stack` writes `TSS.RSP0` and the `system_call`
path's stack together. The two doors into the kernel cannot come to name different stacks.

### The return is an `iretq`, not a `sysretq`

That is a decision rather than an omission. `sysret` returns to `rcx` without checking it is
canonical. A non-canonical `rcx` faults in ring 0 on the user's stack, the shape of CVE-2012-0217.
Using it safely needs an explicit canonicality check and an `iretq` fallback. Sharing one restore
path costs some tens of cycles per system call. It buys one `swapgs` rule with one place to get it
wrong. Recorded in that function's `BUGS` as worth revisiting with a benchmark, once there is a
system-call-heavy workload here to measure.

## What it was proved with, and the honest size of the claim

There was no ELF built for `x86_64-unknown-none` and no scheduler on this architecture. So the proof
is a hand-assembled probe (`arch/x86_64/ring3_probe.s`) entered from the boot thread. The other two
ports shipped the same shape on their first day. Verified on q35:

```
  ring 3      : a program ran at cpl 3 (cs 0x0023, ss 0x001b) and made 2 system calls
                the portable dispatcher answered BadSystemCall (-6) to an unknown number
                reading the kernel's .text at 0xffffffff80109000 from ring 3: Permission(Read)
```

Three separate facts, and the third is the strongest.

- `cs 0x23` is the hardware's answer to what ring this is, since CPL is literally `CS[1:0]`.
- `BadSystemCall (-6)` came out of the portable `crate::system_call::dispatch`, through
  `TrapFrame::{system_call_nr, arg, set_arg}`, and reached the program in `rdi`. So the ABI accessors,
  the entry and the return all work. Asking for a refusal is deliberate. It is the one answer that
  dispatcher can give on a kernel with no scheduler.
- `Permission(Read)` rather than `Translation(Read)` is the page tables saying the walk found the
  kernel's `.text` and refused it. That is the privilege boundary doing its job, not an accident of
  an incomplete map.

## Two latent bugs, found and fixed while reading

Both were on the untested user-thread path. `Context::for_user_thread` wrote the child's three
arguments into `r13`/`r14`/`r15`, while `user_entry_trampoline` read them from `r12`/`r13`/`r14`. A
child would have received `(0, arg0, arg1)`. And the trampoline did not reserve a trap frame's worth
of stack. [Milestone 71 (the thread-start fault)](../0071-thread-start-fault.md) established that
both other architectures need that reservation.

Neither could have been caught before this item, because nothing had ever entered ring 3 here. Both
were still unexecuted at the time. That trampoline is the scheduler's entry path, and the self test
entered through `enter_user` directly.

## What item 3 said was proven, and what was not

What is proven is the arch layer, and the boundary between that and a process is where item 3
stopped. A program runs at CPL 3; the hardware's own `cs` reads `0x23`. It makes system calls that reach
the portable `crate::system_call::dispatch` and gets its answer back in `rdi`. It returns to ring 3, and
it is refused by the page tables when it reads the kernel's `.text`.

What was not proven was anything above the arch layer. There was no ELF loader path, no capability
grant, and no argument passing beyond the three registers `TrapFrame::for_user_entry` sets. There
was no scheduler, so the program was a hand-assembled probe rather than a process. Item 4 is where
that gap closed (see [processes-and-userspace.md](processes-and-userspace.md)), and it is why item 4
came next.

## The system call ABI went up as a decision

The system call ABI is `rax` + `rdi`/`rsi`/`rdx`/`r10`/`r8`/`r9`. It is a boundary rather than a habit:
[DECISIONS §10 (process model: capability-based, microkernel)](../../decisions/0010-capability-microkernel.md)
and [§16 (object revocation)](../../decisions/0016-object-revocation.md). One probe program
agreeing with the kernel was not the same as a decision having been made. So it went up as
[DECISIONS §124 (ratify the x86_64 system call ABI)](../../decisions/0124-x86-64-system-call-abi.md),
which calef ratified on 2026-08-24.

## The two `CR4` bits

The two `CR4` bits stay off, and that became a recorded choice rather than a deferral.

`CR4.PCIDE` off means `crates/address_space_identifier`'s tags have nowhere to live. PCID is
`CR3[11:0]`, and with PCIDE clear those bits are reserved-zero. Writing a tag into them faults rather
than tagging anything. So `arch/x86_64/mmu.rs`'s `ttbr0_value` drops the number, and `flush_asid`
flushes the whole TLB. Each says so in its own words rather than implying a selectivity the hardware
does not have. `CR4.PGE` off means every `mov cr3` discards the kernel's mappings too.

Both are worth turning on, and neither could be shown to be worth it yet. Nothing on this
architecture switched address spaces often enough to measure. Turning them on then would have
bought a number nobody could check. That measurement arrives with the scheduler (item 4), which is
where the decision belongs.

Item 4 landed, and the measurement was then available but deliberately not taken. There is a
workload to measure against: the suite context-switches throughout. `mmu::switch_user_root` already
skips a `CR3` write that would change nothing. That is worth more here than on either other
architecture, precisely because PGE is off. Turning either bit on is an architect's call and wants a
number rather than an argument. `script/icount` had no x86 leg, so producing one was its own small
piece of work.

The tooling half was done on 2026-08-25; the decision was not. `cargo xtask bench --x86` gates on a
deterministic instruction count the same way the other two ISAs already do. The baseline is
`bench/baseline-x86_64.txt`, run with `-icount shift=0,sleep=off`. notes/benchmarks.md's 2026-08-25
section has the measurement that established `rdtsc` tracks icount's virtual clock on `q35`.

This is a leg of `cargo xtask bench`, not of `script/icount` itself. The instrument of
[milestone 78 (the load-sensitive assertions)](../0078-load-sensitive-assertions.md),
`kernel/src/icount.rs`, still refuses `--arch x86_64`, and correctly so. Its claims need a re-armed
deadline timer to compare against. This port's LAPIC timer is periodic hardware reload with no
deadline to read.

What existed then was `yield_switch`'s deterministic baseline: 9,132 instructions per switch, a bare
kernel-thread switch with no `CR3` write. The CR4 question itself was still unmeasured. Nothing yet
exercised `switch_user_root`'s address-space-switch path under load. So this baseline was not the
number the two bits would be judged against, only the tooling that would report it once such a
workload existed. The item's status did not move. The 2026-09-19 measurement that closed it is in
[closing.md](closing.md).
