# What each system makes you trust, measured

*Provisional name (`notes/trusted-base.md`), written 2026-09-20 by the `maintainer/redleaf-comparison`
lane, alongside `notes/redleaf.md`. That note compares the systems; this one supplies the units,
because the comparison everyone reaches for is between numbers that do not mean the same thing.*

The failure this note exists to prevent is one subtraction. nife's kernel is 39,892 code lines.
Tock's paper says *"under 1000 lines out of over 6000 lines of kernel code."* A reader who sets those
side by side concludes that this project's trusted base is six times Tock's whole kernel and forty
times its trusted part, and that reader is not doing arithmetic wrong. They are comparing three
different definitions of "trusted" without being told there are three.

**The note's job is to make the comparison possible, not to win it.** No positioning sentence is
written here; where nife stands is an architect's to say.

## The three definitions, and why they are not interchangeable

The seL4 Foundation whitepaper gives the definition everyone is nominally using (Heiser, *The seL4
Microkernel: An Introduction*, Revision 1.4.1 of 2026-07-17, `sel4.systems`): the TCB is the
*"subset of the overall system that must be trusted to operate correctly for the system to be
secure."* Every system below agrees with that sentence and then disagrees about which subset it
picks out, because they put the isolation boundary in different places.

nife: the trusted base is the kernel plus the hardware, and that is the whole claim. Isolation is
the MMU and the capability table, both programmed by the kernel. A userspace server is outside the
base *by construction*, whatever language it is in and whether or not it contains `unsafe`: the C
component in DECISIONS §31 (the foreign-language seam) is the worked demonstration, and
`notes/c-seam.md` puts the arithmetic as *"what a bad index reaches: any physical memory, any device
register [in a monolith] | one page it was granted [confined here]."* A safe-Rust bug inside
`kernel/src` is in the base; an `unsafe` block in a user program is not.

Tock: one address space, so the trusted base is the `unsafe` portion of the kernel. Nothing
separates a driver from a driver except the type system, so the safe part of the kernel is *not*
trusted for isolation and the unsafe part is. That is what makes "under 1000 of over 6000" a real TCB
measurement rather than a code-quality statistic. The full sentence matters and is usually truncated:
*"The kernel's trusted computing base includes the Rust core library as well as under 1000 lines out
of over 6000 lines of kernel code."* `libcore` is inside the base, unmeasured, and so is the
compiler that enforces the type system the whole scheme rests on.

RedLeaf: the same shape as Tock, one layer larger, and stated rather than measured. Its domains *"are restricted to safe Rust (i.e., microkernel and
trusted libraries are the only parts of RedLeaf that are allowed to use unsafe Rust extensions)"*,
and *"all domains and the microkernel run in ring 0."* So the compiler is in the trusted base, and
RedLeaf says so first: *"The core assumptions behind RedLeaf are that we trust
(1) the Rust compiler to implement language safety correctly, and (2) Rust core libraries that use
unsafe code."* Its own enumeration: *"RedLeaf's TCB includes the microkernel, a small set of trusted
RedLeaf crates required to implement hardware interfaces and low-level abstractions, device crates
that provide a safe interface to hardware resources, e.g., access to DMA buffers, etc., the RedLeaf
IDL compiler, and the RedLeaf trusted compilation environment."* That is a list and not a number;
the paper contains no line count anywhere. It also adds two components Tock does not have (an IDL
compiler and a signing build environment), and the same authors later called the second
*"incomplete and error prone"* (PLOS '21).

seL4: the kernel, and the number is published. *"In a well-designed microkernel, such as seL4, it
is of the order of ten thousand lines of source code (10 kSLOC)."* Its proof is *"200,000 lines of
proof script at the time"* (2009). Both from the whitepaper above. Secondary sources give tighter
figures that disagree with each other and are not used here: the Atmosphere KISV '23 paper says
*"200,000 lines of proof code ... for 8,700 lines of C and required 22 person-years"* while the SOSP
'25 paper from the same group says *"180,000 lines of proof code ... for 8,700 lines of C and
required 20 person-years."* Same authors, same citation, three numbers moved. So this note
quotes the primary source and flags the rest.

The definitions in one place:

| | what is trusted for isolation | what enforces it | is the compiler in the base? | is userspace in the base? |
|---|---|---|---|---|
| **nife** | the kernel | MMU + capability table | no | **no** |
| **seL4** | the kernel | MMU + capability table | no (the proof is over C) | no |
| **Tock** | the `unsafe` part of the kernel, plus `libcore` | the Rust type system | **yes** | **yes**, for capsules; user processes use hardware |
| **RedLeaf** | microkernel + trusted crates + device crates + IDL compiler + build environment | the Rust type system | **yes** | **yes**, all domains are in one ring |

## The numbers, re-derived on this worktree

From `script/metrics --table`, run 2026-09-20 on `maintainer/redleaf-comparison`:

| measure | value |
|---|---|
| `kernel_code_lines` | **39,892** |
| `kernel_comment_lines` | 35,788 |
| `other_code_lines` | 88,334 |
| `other_comment_lines` | 64,582 |
| `unsafe_outside_arch` | **824** |
| `unsafe_inside_arch` | **314** |
| `unsafe_thread_safety` | 23 |
| `unsafe_code_lines` (the density denominator) | 106,436 |
| `unsafe_density` | **77** per 10,000, against `script/lint`'s ceiling of 88 |
| `harnesses_total` | 178 |

`script/lint`'s own check reports **119 `unsafe fn` declarations**, 26 of them trait-impl methods
whose contract belongs to the trait, and every other one carries a `# Safety` section or the gate
fails.

One derived split is needed and the tree does not publish it, so it is computed here with
`helpers/rust_source.py`'s own stripper and the same `HOST_ONLY` exclusion `script/metrics` applies:

| | code lines | `unsafe` blocks |
|---|---|---|
| `kernel/src/arch/` | 9,015 | 314 |
| `kernel/src/` outside `arch/` | 30,877 | 263 |
| `crates/` reachable **only** from the kernel | -- | 117 |
| **the trusted base, total** | -- | **694** |
| userspace | -- | 382 |
| shared by both | -- | 19 |
| the boot chain, before the kernel exists | -- | 37 |

This table said 577 when it was written on 2026-09-21 and that was short by 117, corrected the
same day. The error stays because it is the mistake this page exists to prevent: **the boundary is not a path.** A count of `kernel/src/` misses
sixteen `crates/` members that only the kernel depends on, several of which were **deliberately
lifted out of `kernel/src` so that Kani could reach them. Drawing the line at the directory
therefore undercounts the trusted base by exactly the code this project moved in order to prove
things about it.

Milestone 522 (a boundary drawn by dependency, not by path) draws it at a real `cargo metadata`
dependency edge instead. `script/metrics` now reports the four populations separately:
`unsafe_trust_kernel` is 694, at a density of 142 per 10,000 lines against userspace's 120.

**The number of `unsafe` blocks in nife's TCB is 694**, not 1,138, not 824, and not 577.

So the closest thing to Tock's ratio that this tree can state is 694 unsafe blocks against the
kernel's code, and it is not Tock's ratio, for two reasons:

1. **The numerator is blocks, not lines.** Nothing in the tree converts one to the other, and a block
   ranges from a one-line register write to a page of context-switch glue.
2. **The denominator is not measuring the same thing.** Tock's 6,000 lines is a base of which only the
   1,000 is trusted. nife's 39,892 lines is trusted in its entirety, unsafe or not: a safe-Rust
   bug in `kernel/src/syscall.rs` can mint an authority nobody should hold, and no type system
   prevents that. The unsafe census is therefore not a TCB measurement at all. It measures how much
   of the code the compiler cannot check, which is a proof-burden and review-burden number, and the
   tree says so in its own words: the density exists to hold the claim that *"the tree is getting
   proportionally safer."*

And the comparison that is actually apples to apples is with seL4, not with Tock, because seL4
draws the boundary where nife does: 39,892 code lines against *"of the order of ten thousand."*
Interpreting that gap is not this note's job, but a reader needs two facts first: nife carries three architectures where seL4's published figure is one configuration, and
that `kernel/src/arch/` is 9,015 of the 39,892.

## Their six categories, run against this tree

The APSys '17 paper's contribution is the claim that only six pieces of *kernel* code must be unsafe:
context switches, memory-mapped I/O and structures, the memory allocator, userspace buffers,
interrupt and exception handlers, and `TakeCell`. Run against this tree, **the list is shorter here,
not longer**, and every deletion has a reason in the architecture rather than in the tidiness.

| their category | nife | where |
|---|---|---|
| **Context switches** | yes, and the assembly is separated from the Rust | `kernel/src/arch/<isa>/context.s` holds the register save/restore; `context.rs` has **zero** `unsafe` blocks, because the unsafety is in the `.s` file the Rust calls |
| **Memory-mapped I/O and structures** | yes | `kernel/src/drivers/` (`pl011.rs`, `ns16550.rs`, `gic.rs`, `gicv3.rs`, `plic.rs`, `ramfb.rs`): volatile accesses at named offsets, each under a `SAFETY` comment, and no register crate |
| **The memory allocator** | **no. The kernel has no allocator** | there is no `#[global_allocator]` anywhere in `kernel/src`. milestone 14 (kernel objects from untyped: remove the kernel heap) is the decision; `kernel/src/kmem.rs` is a fixed carve, not a heap; `notes/kernel-budget.md` has the one draw it missed and how 19c.1 closed it |
| **Userspace buffers** | **no. No pointer crosses the boundary** | `kernel/src/syscall.rs`'s own header: *"Today every argument is a scalar in a register ... so the kernel follows no user pointer and there is no deputy to confuse."* The console is a userspace server, milestone 8 (the console driver leaves the kernel). The primitive is kept (`mmu::user_can_read`) for the next syscall that needs it |
| **Interrupt and exception handlers** | yes | `kernel/src/arch/<isa>/exceptions.rs` (10 `unsafe` blocks on aarch64), `vectors.s`, `irq.rs` |
| **`TakeCell`** | **no, and the reason is the whole difference** | see the next section |

Two of their six do not exist here, and neither absence is an accident. The allocator is gone
because of the capability model: milestone 14 (remove the kernel heap) has as its thesis that the kernel spends nothing it was not
handed, so there is no heap to write unsafely. The userspace-buffer category is gone because of the
syscall surface: a kernel that follows no user pointer has no buffer to validate, and DECISIONS §10 (process model: capability-based, microkernel) is why. Both are cases where hardware isolation plus
a capability discipline removed an unsafe category outright, which is the opposite of the direction
the 2017 paper predicts for a hardware-isolated kernel.

**And this tree has categories they do not**. Every one of them exists *because* isolation is hardware:

- Programming the MMU. `kernel/src/arch/aarch64/mmu.rs`, 36 `unsafe` blocks, whose own header
  calls turning the MMU on *"the sketchiest moment in the kernel."* This is exactly the structure
  Levy left to future work (*"data structures on disk or in hardware (e.g. the page table)"*), and
  the answer here is to lift the math into `crates/paging` where a model checker can reach it.
- System-register access per architecture. `isa.rs`, `timer.rs`, `pmu.rs`, `fp.rs` on three ISAs.
  A single-ISA embedded kernel does not pay this three times.
- DMA, which is the one hole hardware isolation does not plug by itself. A device reads physical
  memory with no MMU in front of it, so `crates/direct_memory_access_validator` checks every
  descriptor and the device reads a shadow the driver cannot touch. `notes/dma.md` and
  `notes/iommu.md` have the software and hardware halves. RedLeaf declines this: *"We trust devices
  to be non-malicious."*
- Secondary-core bring-up and per-CPU state. `kernel/src/smp.rs`, `kernel/src/cpu.rs`,
  `kernel/src/interrupt_stack.rs`, all `UnsafeCell` over static per-CPU storage. Levy's paper
  explicitly did not evaluate this (*"we did not evaluate our design in a multi-processor setting"*),
  and its ask was *"how to avoid growing the trusted computing base in service of concurrency."*
  This tree is a data point on that question and has never been read as one.

## Does this tree need `TakeCell`?

No, and it does not pay the copies either. It avoids the problem, because the kernel has a
different shape.

Their problem, stated in their words: kernel code is event-driven, *"multiple components must both be
able to mutate a shared data structure"*, Rust permits one mutable reference, and `Cell` solves it
only partly because *"it imposes the significant cost of requiring memory copies, which is an
unacceptable overhead for complex or large kernel data structures."* `TakeCell` passes code in through
a closure instead, and `map(f)` *"is a no-op and does not execute the closure"* when a reference
already exists, making it *"a form of mutual exclusion ... unlike a mutex, it skips the operation
instead of blocking."*

The last clause is the whole answer. Tock cannot use a mutex because there is nothing for a mutex
to block: it is a single-threaded, event-driven kernel for low-power uniprocessors, with no scheduler
to run something else and no other core to wait for. `TakeCell` is the mutual exclusion you are left
with when blocking is not available.

nife has both. It has threads, a scheduler and up to `MAX_CPUS` cores, so it takes a lock:
`kernel/src/sync.rs`'s `IrqSafeSpinLock`, which masks interrupts before acquiring and restores on
release. A global lock ranking is enforced by the type (DECISIONS §9 (locking: IrqSafeMutex, plus a
discipline) governs both halves). A lock
hands out `&mut` through its guard. No copy is made, and no closure indirection is needed, so
neither of the two things Levy is choosing between applies.

Checked rather than asserted: there is no `Cell<T>` and no `RefCell` in `kernel/src` at all. The
only interior mutability in the kernel is `UnsafeCell`, in six places. Every one of them is static
per-CPU or per-core storage (`interrupt_stack.rs`, `cpu.rs` for the run queue and the x86_64
syscall-stack scratch words, `smp.rs`), plus two test-local `Racy<T>` wrappers in `bench.rs` and
`sched.rs`. None of that is the pattern `TakeCell` addresses.

So: nothing to propose, and one thing worth recording. The two designs differ in what happens when
the exclusion is violated, and the difference is not in nife's favor by accident. `TakeCell::map`
silently does nothing on re-entry; the operation is dropped and the caller is not told. A lock
taken twice on one core is a hang, which is loud, which is precisely what `kernel/src/sync.rs`'s
header is about: *"This is not a race. It is a guaranteed hang the moment the timing lines up."*
Trading a silent no-op for a loud hang is the right trade for a kernel with a supervisor over it
(DECISIONS §26 (the fault endpoint: thread death becomes a message)). It is the wrong one for a
kernel that must keep running on a battery, which is the system Tock is.

## BUGS

- This was true when written and was fixed within hours, which is why the entry stays. It read:
  *"`script/metrics` tracks `kernel_code_lines` and `unsafe_density` over time, but not the TCB split
  this note's central table depends on, so the finding cannot be watched for drift and will be stale
  the moment a lane lands."* It went stale faster than that: the table's own figure was wrong when
  published. `script/metrics` now carries the split as its own columns, so the drift is watchable. The remaining half of the entry is still true: the figures in this note are
  a snapshot, nothing regenerates them, and a reader should re-derive before quoting.
- "The trusted base is the kernel" is a claim about the design, not a measurement. It assumes the
  MMU and the capability table do what the code says, which is what `design/fatal-risks/README.md`'s risk 2
  (the proofs prove trivia) exists to interrogate, and that risk is AMBER. Most of `kernel/src/arch/`
  is behind `asm!` and MMIO, riscv64's included since milestone 589 (Kani can prove
  riscv64 from the hosts we already have).
- Line counts are a poor proxy for a proof obligation and a worse one across languages. 39,892
  lines of Rust and 10 kSLOC of C are not the same unit of anything, and a tree can shrink this
  number by moving code out of `kernel/src` without reducing what anyone has to trust. The move to
  `crates/paging` and `crates/direct_memory_access_validator` is exactly that shape, and it was done
  for the prover rather than for the number, but a future reader cannot tell those two motives apart
  from the series.
- **The `unsafe` block count is a regex over stripped source** (`helpers/rust_source.py`), so it
  counts `unsafe {` and `unsafe fn` and cannot see how much code is inside one.
- **The six-category audit is a reading, not a proof.** "nife has no allocator in the kernel" was
  checked by grepping for `#[global_allocator]` in `kernel/src` and reading
  `notes/kernel-budget.md`; "no pointer crosses the boundary" was checked by reading
  `kernel/src/syscall.rs`'s header, which is documentation. Neither is a gate, and neither would
  catch a new syscall that reintroduced the category tomorrow.
- **Tock today was not read.** The 2017 paper is treated as an argument. Nothing
  here is a claim about what Tock's trusted base looks like in 2026.
