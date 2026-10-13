# Milestone 161 appendix: the scheduler, real processes and userspace, steps 12 and 13

An appendix to [milestone 161 (the x86_64 kernel port)](../0161-x86-64-kernel-port.md), moved out
of that page on 2026-10-12 (UTC) under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md).
The main page is complete without it; this is the record of steps 12 and 13 (both 2026-08-24) and of
the open list's item 4, which the page now summarizes. Long sentences were split to meet
[§213 (writing standards)](../../decisions/0213-writing-standards.md); no fact changed.

## Step 12: the scheduler, real processes, and the kernel's own test suite

The open list's item 4. It closed the distance between step 11's "a program ran at CPL 3" (see
[ring-3.md](ring-3.md)) and "a process".

Almost none of it was new x86 code. That is the claim of
milestone 20 (a portable HAL, proven on a second architecture) holding a third time. `kmem`,
`untyped`, `sched` and `user::AddressSpace` are portable and came up here by being compiled for this
target. What had to be written was the arch layer underneath them. What had to be found was four
places where portable code encoded an assumption true of two architectures.

### The trap path's split

The trap path grew the split both other ports already had. `x86_trap_handler` became
`x86_trap_dispatch` plus `x86_trap_body`, with `dispatch_on_interrupt_stack` in trap.s between them.
The outer one runs on the interrupted thread's stack, where the deferred `schedule()` runs. The inner
one may run on this CPU's interrupt stack. That is not an optimization. `schedule()` parks the
running `rsp` in the outgoing thread's `Context`, so calling it from a per-CPU stack would park a
per-CPU address in a thread.

### The kernel stack, recomputed on every return

`TSS.RSP0` and the `syscall` path's stack are recomputed from the frame on every return to ring 3.
x86 has two doors from ring 3, and they find their stack differently. With a scheduler there is one
kernel stack per thread, so the pair has to be re-pointed at every switch. The frame's own address is
the answer: `rsp + 176` at the top of `isr_restore`.
[Milestone 71 (the thread-start fault)](../0071-thread-start-fault.md) put every thread's
`TrapFrame` at `stack_top - 176`. This makes the wrong state unrepresentable rather than a duty to
remember. RISC-V does exactly this at exactly this point.

### Reschedule IPIs and the fault arm

`send_reschedule` stopped being `unimplemented!()`. The local APIC's ICR is an IPI send. Its "self"
shorthand is also how this ISA raises an interrupt by hand. That puts x86 on aarch64's side of
`sched`'s test-fixture split rather than RISC-V's. The intid for a local APIC source is its vector,
because there is no controller input to name. Legacy IRQs (0..15) and these vectors (0x20..0x2f)
cannot collide.

The ring-3 fault arm became a real teardown through `sched::fault`. `ring3_probe.s` and the
`x86_enter_user_and_wait`/`x86_leave_user` pair were deleted, exactly as item 3 said they would be.
All three hand-overs item 4 named were taken.

### Four bugs in portable code, all one shape

Each was a default arm or an expression naming one of two answers, silently wrong at three.

- `thread.rs`'s stack area was `KERNEL_VA_BASE | 0x10_0000_0000`. That is the identity on x86,
  because that base already carries the bit. Every kernel thread stack would have been mapped over
  `.text`.
- `crates/elf`'s `EXPECTED_MACHINE` was `not(riscv64) => EM_AARCH64`. The x86 kernel was compiled to
  accept aarch64 binaries and refuse its own.
- `xtask`'s `ArchLegs` predicates were `self != the_other_one`.
- `smp::bring_up_secondaries` computed the secondary entry with `virt_to_phys` before checking
  whether any core can be started. That panics here, because `secondary_boot` is in `.boot`, linked
  at its physical address.

### Verified on q35

The frame line is the one that means the most:

```
  scheduler   : up on 1 cpu, preempting at 100 Hz (idle thread registered)
  kernel task : a spawned thread ran and carried its captured state (0x16100004)
  userspace   : a process built from untyped ran at cpl 3 and sent 0x1610004 on a granted cap
                thread 8589934594 died at pc 0x400005 on addr 0xa50000, delivered to its supervisor
                two children cost 0 frames the first round and 0 the second (steady state)
```

Two children, because the two halves of "a process" fail differently. The first is built out of a
single untyped region: address space, code page, stack page, TCB, two endpoints. It is granted a
capability and dispatched to ring 3 by the scheduler, invokes that capability, and exits. The second
faults, and its death arrives on its supervision endpoint naming the thread, the pc and the address.

The round runs twice, because a first round pays first-use carves that are not leaks. A system in
steady state charges the second one zero.

### The suite

`script/test --arch x86_64` ran 97 tests and skipped 7, in 2.7 seconds. `--arch` takes a third
value, and the default runs all three legs. That is what
[DECISIONS §19 (architectural parity is a tenet)](../../decisions/0019-architectural-parity.md)
means by parity being a gate. The seven skips each print a reason:

- two are fixtures the runner does not attach;
- four are SMP or roster facts a single-core machine with no device tree cannot have;
- one is the device-tree magic, which PVH's `hvm_start_info` genuinely is not.

## Step 13: every program in `user/`, an archive, and thirty gated test modules

Item 4's own hand-off, closed the same day it was written. Every program in `user/` compiles for
`x86_64-unknown-none`. `xtask` packs an archive, QEMU's PVH loader hands it over as a module, and
`cfg(initrd)` is on. The suite went from 97 pass / 7 skip to 170 pass / 67 skip, and every skip
prints the fixture it wants. The block's own prediction held exactly. Turning the cfg on was one
match arm in `kernel/build.rs`, and all thirty modules came back with nothing else edited.

### `crates/user_rt`: five transliterations and two decisions

The five follow [DECISIONS §124 (ratify the x86_64 syscall ABI)](../../decisions/0124-x86-64-syscall-abi.md)
mechanically. Three facts at those sites have no counterpart on either other architecture. Each is
the instruction rather than a choice:

- `syscall` clobbers `rcx` and `r11` unconditionally;
- `r10` carries the fourth argument, because `syscall` has already taken `rcx`;
- `syscall` pushes nothing.

`now()` and `cntfrq()` are the two that are not transliteration. The answer, ratified as
[DECISIONS §127 (x86_64's `now()`/`cntfrq()`: PIT-calibrated `rdtsc`)](../../decisions/0127-x86-64-timer-rdtsc.md),
is PIT-calibrated `rdtsc` with the measured frequency delivered through a mapped page.

`now()` is `rdtsc`. It is readable from ring 3 because `CR4.TSD` is clear at reset and this kernel
does not change it. It answers in `edx:eax`, so a single `out(reg)` would read a counter that wraps
every four seconds.

`cntfrq()` is RISC-V's recorded gap, one architecture worse. There is no architected TSC rate.
`CPUID` leaf 0x15 gives a ratio to a crystal that leaf 0x16 may not report, and neither leaf is
universal. So the kernel measures it against the PIT at boot (`kernel/src/arch/x86_64/timer.rs`). It
delivers the number through `counter_frequency_protocol::TimebasePage`, mapped read-only into every
process the kernel builds directly. That is the aux-vector-at-process-start shape riscv64's own
`cntfrq` doc comment already predicted.

A ring-3 program cannot repeat the PIT measurement itself. The PIT is at ports 0x40..0x43, behind
`IOPL` 0 and an empty TSS bitmap. That is
[§121 (what a device capability is when the device has no page)](../../decisions/0121-port-io-capability.md)
rather than an oversight.

The 1 GHz constant that remains is narrower than it first reads. Only a process built by
`supervision_protocol::build_child_space` falls back to it; that is the userspace ELF loader, not the
kernel. That loader maps a freshly retyped, zeroed placeholder page rather than a capability naming
the kernel's real one. Closing that gap is the separately scoped remaining piece of
[milestone 167 (handing a computed page to a userspace-built child)](../0167-timebase-page-delegation.md),
not a live design fork here.

### Three programs refuse rather than pretend

`console::uart_put`, `input`'s `uart` module and `swap_protocol::probe_device` cannot reach a device
from ring 3. Their x86 arms `trap()` rather than no-op. A silent no-op is a console that
acknowledges every byte and prints none.

`user/build.rs` compiles the C seam with `--target=x86_64-unknown-none-elf -mno-sse -mno-mmx
-mno-red-zone`. That matches the Rust target's own `-mmx,-sse,+soft-float` and `disable_redzone`.

### The archive arrives as a PVH module

That is `/chosen/linux,initrd-start` one machine over. `machine_discovery::x86_64::module` decodes
the list (host-tested). The x86 memory front end reserves the region and records it. Free memory
drops from 254 MiB to 249 with a 4.4 MB archive attached. `xtask` grew `initrd-x86`, and the two
packers now share one `portable_archive_entries()` table. `helpers/qemu-runner-x86_64.sh` passes
`-initrd`, and there is a real `target/init-measure-x86_64.txt`. `crates/elf` was indeed ready.

### Four bugs, every one latent since the day it was written

None was catchable before there were user programs here.

- `arch::x86_64::irq::enable` conflated two numbering schemes, a legacy IRQ and a local APIC vector,
  and panicked on `SELF_TEST_VECTOR`.
- `user_rt::trap()`'s `int3` is refused from ring 3 by a DPL-0 gate and raised #GP 0x1a instead of
  #BP, so it is `ud2` now.
- `user/link.ld` did not name `.got`, which x86 emits and the other two do not. The orphan landed
  after `.bss`, and the `data` segment lost its zero-fill tail entirely.
- `fs_service::blk_server_image()`'s x86 arm was a `panic!` that fired before any disk check.

`xtask`'s `read_stripped` cache tag was a fifth, latent and never yet fired. x86 shared aarch64's
`"host"` namespace. Packing both archives in one run would have measured one architecture's bytes
under the other's name. It was fixed before it could fire.

## What still bounded the architecture: devices, not userspace

Of the 67 skips, 21 were one toolchain failure, 13 the missing RTC, 15 the unenumerated PCI bus, 5
were §121, and 4 were SMP. What replaced item 4's list was shorter, and none of it was userspace. Two
items were new findings rather than restatements.

### `fs_server` did not compile for `x86_64-unknown-none`, and it was not our bug

It links the vendored RedoxFS engine, which depends on `aes` unconditionally; the crypto is not
behind a feature. `aes` for this target ends in
`rustc-LLVM ERROR: Do not know how to split the result of this operator!` at every optimization
level including zero. The target spec is the cause. `-mmx,-sse,+soft-float` leaves LLVM no 128-bit
vector register to legalize an AES block into, and no scalar fallback. 21 of the 67 skips were this
one fact.

The routes out were a patch against the vendored crate to make its crypto optional (`patches/` is
where a carried patch belongs), or an x86 userspace target that keeps SSE. Both wanted their own
milestone, minted as [milestone 164 (x86_64 userspace can't build `aes`)](../0164-x86-64-fs-server-aes.md).
Until one landed there was no point attaching a disk to the x86 runner, because there would be
nothing to open it with.

### One foot gun is marked rather than removed

AGENTS.md's ladder says an exception must say it is one. `spawn_init` grants slot 2 a device
capability over `user::UART_PHYS`, which on this architecture is physical page zero. The slot is
positional. Declining to grant would renumber the interrupt capability and every role that names it.
Nothing better can go there: §121's answer to what a port capability is, is that there is none.

Nothing reaches it. Every fixture that would map it asks
`user::machine_has_no_device_page_for_the_console()` first and skips. A role that mapped it anyway
would read the real-mode interrupt vector table and look like it worked.

### Where the rest of the 67 went

The rest were the items around this one, doing what they were always going to do. What closes some
of them has since changed.

- 13 skips wanted the RTC. That is now piece 2 of
  [milestone 176 (the x86_64 discovery seam's wide half)](../0176-x86-64-discovery-seam-wide-half.md),
  decided by [§130 (how the kernel-resident CMOS RTC reaches the userspace clock service)](../../decisions/0130-cmos-rtc-delegation.md).
  It is not §121's, which forecloses a CMOS port capability rather than building one.
- 15 wanted a PCI bus enumerated. The discovery seam no longer blocks that: item 0's PCI window is
  wired, by [milestone 165 (x86_64 PCI enumeration)](../0165-x86-64-pci-acpi-mcfg.md). But
  `helpers/qemu-runner-x86_64.sh` still did, since it attached no PCI device (`-device
  virtio-blk-pci` and friends) for anything to enumerate.
- 5 want a UART page. §121 answers that permanently: kernel-resident forever, no port capability.
  These five stay skipped by design rather than by omission.
- 4 wanted a second core (item 5).
- 1 wanted `CR4.PCIDE` (item 3's measurement).
- 1 wanted an `x86_64-unknown-nife` target and a `std` farm, then the tail of
  [milestone 27 (Rust `std` on the native ABI)](../0027-rust-std.md).

The page's follow-on list says what closed each of these later.
