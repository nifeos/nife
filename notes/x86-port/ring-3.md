# The x86_64 port: ring 3

*An appendix to [`notes/x86-port.md`](../x86-port.md), which is the page to read. This file holds
the address spaces, `swapgs`, the `syscall` pair, and the first ring-3 proof. It exists to verify or
challenge the main page. A reader who only needs to build, boot or test the x86_64 port should not
have to open it. The directory `notes/x86-port/` and this file's stem are provisional names; naming
is calef's.*

*Records cited below: milestone 161 (the x86_64 kernel port) and milestone 71 (the thread-start
fault).*

## Ring 3, and the four pieces that have to work in order

Milestone 161's roadmap item 3, built 2026-08-24. Everything above this heading is the kernel
talking to the machine. This is the kernel refusing a program, which is the thing the rest of it
exists for.

### The address spaces are RISC-V's model, not aarch64's

What matters is how many roots
the hardware has: aarch64 has two (`TTBR0` for the user, `TTBR1` for the kernel), so switching a
process leaves the kernel mapped for free. RISC-V has one `satp` and x86 has one `CR3`, so a
process's own root must carry the kernel's entries or the `mov cr3` unmaps the instruction after
it.

`share_kernel_half` is therefore one `copy_from_slice` of PML4 entries 256..512, exactly the RISC-V
twin. It is worth noticing how much it buys on this architecture specifically. Both kernel bases
are up there (the image at PML4[511], the direct map at PML4[273]) along with every device window,
so one copy shares all of it. The entries point at the kernel's own intermediate tables, so this
shares the map rather than a snapshot: a page the kernel maps afterwards appears in every process,
which is what a shared half has to mean.

Where x86 is neither: the ASID. PCID lives in `CR3[11:0]` and is honored only with `CR4.PCIDE`
set, and it is not set here. So `ttbr0_value` drops the tag `crates/address_space_identifier` hands it, because with
PCIDE clear those bits are reserved-zero and `root | asid` would `#GP` rather than tag anything; and
`flush_asid` flushes the whole TLB, because there is no tag for it to select on. Both say so
where a reader meets them. Over-flushing is correct and slow; under-flushing would be one process
reading another's memory with nothing to announce it, which is the failure that function exists to
prevent.

### `swapgs` is an exchange, which is the whole reason it is guarded

A trap from ring 3 arrives with the *user's* `GS` base, and this kernel keeps its per-CPU pointer in
`IA32_GS_BASE`, so the first thing that takes a lock would read whatever the program left there.
`swapgs` exchanges `IA32_GS_BASE` with `IA32_KERNEL_GS_BASE`, and that is what makes the pointer
unforgeable: the value the kernel needs sits in a register ring 3 cannot write, which is the problem
RISC-V solves with `sscratch`.

Because it is an exchange rather than a load it must run exactly once per privilege change in each
direction. A trap from ring 0 that swapped would install the user's base while running kernel
code; a nested trap that swapped again would put the kernel's back and then hand it to the user on
the way out. So both sites test the saved `CS`'s low two bits, which *are* the interrupted CPL.

It does not trip the bug this port already has a section about. "Loading a segment register in
long mode destroys that segment's base MSR" applies to loading a *selector*; `swapgs` writes the MSR
pair and loads nothing. That was checked against the instruction's definition rather than assumed,
precisely because the earlier bug cost an afternoon and presented as something else entirely.

The one delicate window is on the way out. Between the exit `swapgs` and the `iretq` the CPU is
in ring 0 holding the user's GS base, so an interrupt taken there would see CPL 0, decline to swap,
and dereference the user's value. `RFLAGS.IF` is clear at every such site (an interrupt gate cleared
it; `IA32_FMASK` clears it for `syscall`; the two ring-3 entry points `cli` first), which closes it
for everything except an NMI or a machine check. Closing it for those needs a paranoid entry path
that reads `IA32_GS_BASE` and decides, which this port does not have. Recorded rather than pretended
away.

**The same gap is open on the way in, and it is worse there** (found 2026-10-13 UTC by lane
`maintainer/supervision-label-hang`, unfixed). `x86_syscall_entry` runs two instructions on the
user's `rsp` before it loads the kernel's, and the NMI gate has no IST stack (only vector 8 does).
An NMI landing there is taken at CPL 0 with no stack switch, so the CPU, `isr_common` and the
handler's calls write a few hundred bytes below whatever address the user put in `rsp`, a kernel
address included. NMIs are not rare here: every TLB shootdown on a machine of more than one core sends
one to every other core. The fix is an IST stack per core for vector 2, with the nesting rules an
NMI on an IST stack needs. It wants a milestone of its own rather than a line in a flake fix.

### `syscall` shares almost nothing with the IDT

Four MSRs are its entire configuration and none has a useful default:

| MSR | What it carries |
|---|---|
| `IA32_STAR[47:32]` | the kernel CS; the kernel SS is that plus 8, which is why the GDT's order is arithmetic |
| `IA32_STAR[63:48]` | the base `sysret` derives the user pair from: SS = base + 8, CS = base + 16 |
| `IA32_LSTAR` | where a 64-bit `syscall` jumps. A raw address, so a zero here is a jump to zero |
| `IA32_FMASK` | the `RFLAGS` bits cleared on entry, `IF` among them |
| `IA32_EFER.SCE` | whether `syscall` is a legal instruction at all, rather than `#UD` |

`SCE` is enabled last, so the instruction becomes legal only after the address it jumps to and
the flags it clears are already in place. Three `const` assertions in `exceptions.rs` tie
`SYSRET_SELECTOR_BASE` to the selectors in `segments.rs`, so changing one file without the other
stops the build rather than landing a program on the kernel's data segment.

The entry path is where the difference from a trap gate is felt: `syscall` does not switch stacks
and pushes nothing. `rsp` still names the user's stack and every register still holds a user value,
so the first three instructions are `swapgs`, park the user's `rsp` in a static, and load the
kernel's. `segments::set_kernel_stack` writes `TSS.RSP0` and that static together, so the two doors
into the kernel cannot come to name different stacks; both are one CPU's, which is the same
single-TSS limitation SMP bring-up already has to fix.

The return is an `iretq`, not a `sysretq`, and that is a decision. `sysret` returns to whatever
`rcx` holds without checking it is canonical, and a non-canonical `rcx` faults *in ring 0 on the
user's stack*: the shape of CVE-2012-0217. Using it safely needs an explicit canonicality check plus
an `iretq` fallback, which is Linux's "opportunistic sysret" dance. Sharing one restore path costs
some tens of cycles per syscall and buys one `swapgs` rule with one place to get it wrong. It is in
the handler's `BUGS` as worth revisiting with a benchmark, once there is a syscall-heavy
workload here to measure.

### What it was proved with, and the honest size of the claim

There was no ELF built for `x86_64-unknown-none` and no scheduler on this architecture, so the proof
was a hand-assembled probe (`arch/x86_64/ring3_probe.s`) entered straight from the boot thread. That
is the shape both other ports shipped on their first day and then deleted, and it was deleted here
for the same reason by roadmap item 4; the transcript below is what it printed while it existed.

```
  ring 3      : a program ran at cpl 3 (cs 0x0023, ss 0x001b) and made 2 syscalls
                the portable dispatcher answered BadSyscall (-6) to an unknown number
                reading the kernel's .text at 0xffffffff80109000 from ring 3: Permission(Read)
```

Three facts, and they get stronger down the list.

`cs 0x0023` is the hardware's own answer to what ring this is, because CPL is literally
`CS[1:0]`. A program that had somehow stayed in ring 0 would have reported `0x08`.

`BadSyscall (-6)` came out of the *portable* `crate::syscall::dispatch`, through
`TrapFrame::{syscall_nr, arg, set_arg}`, and reached the program in `rdi`. Asking for a refusal is
deliberate rather than lazy. An unimplemented number is the one thing that dispatcher can answer on a
kernel with no scheduler, so it is the only round trip available through the real thing rather than a
stand-in. Getting the answer back proves the entry, the ABI accessors and the return all work. It
is checked in code rather than printed and left to a reader, because the interesting failure is
not "no answer" but "an answer from somewhere else". A return register the restore path never wrote
would most likely still hold the zero the entry frame put there.

`Permission(Read)` rather than `Translation(Read)` is the strongest of the three. The page *was*
found, because a process root carries the kernel's high half, and the walk refused it on the `U/S`
bit. A test that only asserted "something faulted" could not tell those apart, and the sloppier
of the two would have passed it. x86 is the most forthcoming of the three architectures here: it
pushes an error code saying so outright, where aarch64 reads it out of `ESR_EL1` and RISC-V has to
re-walk the tables to find out.

### The way back, which was scaffolding and is gone

`enter_user` never returns, and the boot tour had to print what happened. So `trap.s` grew
`x86_enter_user_and_wait` and `x86_leave_user`. Together they park the six callee-saved registers and the call's
own return address on the current stack, record that block's address, enter ring 3, and resume it
from the trap handler when the probe was done. It was `switch_to`'s two halves with a ring change in
the middle, which was not a coincidence.

Both, and `ring3_probe.s` with them, were deleted by roadmap item 4, which is what the paragraph
above predicted: a scheduler does with two threads what that pair did with one. What replaced them
as the tour's proof is a pair of real processes; see [scheduler.md](scheduler.md).

### Two latent bugs on a path nothing had ever executed

Found by reading `context.s` against `context.rs` while wiring this up, and worth recording because
neither could have been caught earlier and both were in code that looked finished:

- The child's arguments were in the wrong registers. `Context::for_user_thread` wrote them to
  `r13`/`r14`/`r15`; `user_entry_trampoline` read them from `r12`/`r13`/`r14`. A child would have
  received `(0, arg0, arg1)` and `arg2` would have vanished. Both files were internally consistent
  and neither was executed.
- The trampoline did not reserve a trap frame's worth of stack. Milestone 71 established that
  both other architectures need one. The thread's `TrapFrame` lives at `top - 176` for the life of the
  thread, and the entry path's own frames start at the same top.

Both are fixed and both are still unexecuted: that trampoline is the *scheduler's* entry path,
and the self test enters through `enter_user` directly. They are the argument for reading the two
halves of a context switch side by side rather than one at a time.
