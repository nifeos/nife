# The x86_64 port: what does not fit, and where it pays

*An appendix to [`notes/x86-port.md`](../x86-port.md), which is the page to read. This file holds
the four places the `arch/` seam did not stretch, two bugs worth knowing, and where TSO pays out. It
exists to verify or challenge the main page. A reader who only needs to build, boot or test the
x86_64 port should not have to open it. The directory `notes/x86-port/` and this file's stem are
provisional names; naming is calef's.*

*Records cited below: milestone 161 (the x86_64 kernel port).*

## The three things that genuinely do not fit

Recorded because the failures of an abstraction are worth more than its successes.

### 1. Permissions are inherited down the page-table walk

Both existing formats treat the leaf as the single source of truth for access rights, which is why
`PageFormat::table_entry` takes no flags. x86 ANDs every level's `U/S` and `R/W` (and ORs `XD`), so a
leaf saying "user, writable" under a PML4 entry saying "supervisor, read-only" is supervisor-only and
read-only.

`Ia32e` therefore makes intermediate entries maximally permissive and lets the leaf decide, which
reproduces the other two formats' meaning exactly. The cost is real and is named in the module: the
hierarchical bits are x86's mechanism for revoking a whole subtree in one store, and this gives it
up in exchange for one meaning of "what does this mapping grant" across three architectures.

### 2. Port I/O has no page, so a device capability cannot be a mapping

On aarch64 and RISC-V a device *is* a page, so a device capability is a mapping and the MMU enforces
it. The legacy x86 devices, including the console UART, live in a 16-bit I/O space with no page
tables in front of it. x86 gates that space two ways: `RFLAGS.IOPL` (all-or-nothing per privilege
level) and the TSS I/O permission bitmap, one bit per port, which is per-task rather than
per-page.

Nothing uses the bitmap. `TrapFrame::for_user_entry` leaves `IOPL` at 0 and the TSS's bitmap offset
points past the end of the structure. So a ring-3 program on this kernel may not touch a port at
all, which is now the *permanent* answer, not a placeholder while an open question sits above it.
`user::UART_PHYS` is zero on this architecture, and that zero stays the marker for the decision
below rather than for anything still open. Written up as §121 (DECIDED): legacy port I/O stays
kernel-resident permanently (option 2). The port-range-capability alternative this section once
priced as future work (a real TSS-bitmap grant, a genuinely different shape of capability from the
one the tree has) is closed, not deferred. §121 is the first case in this tree where the object a
capability names is not memory. The answer landing on "the kernel keeps this one" is itself a
recorded finding worth reading if a later capability over something that is not a frame comes up.

### 3. One base cannot do both jobs, so this architecture has two

Resolved 2026-08-24 (milestone 161's roadmap item 1); left here because it is the one place the
three architectures' address arithmetic genuinely diverges, and a reader of the other two ports will
arrive expecting a single constant.

`KERNEL_VA_BASE` is `0xffffffff80000000` because the target's code model requires it, and there are
only 2 GiB of address space above it. So `VA = PA | KERNEL_VA_BASE` can never address more than
2 GiB of physical memory, which is not enough for a real machine and not enough to reach the local
APIC at `0xfee00000` either. It is not even invertible up there: `phys_to_virt(0xfee00000)` produced
a valid distinct address whose `virt_to_phys` did not give `0xfee00000` back.

Linux separates the two jobs this one constant was doing, and so does this port now. The kernel
*image* sits at `KERNEL_VA_BASE`, where the code model needs it; the *direct map* sits at
`DIRECT_MAP_BASE = 0xffff888000000000`, which is Linux's `page_offset_base`, taken rather than
invented so that a reader who has met one x86_64 kernel has met this number. They are PML4[511] and
PML4[273], so nothing about them interferes, and both are canonically high, so the same bit-47
`Ia32e::is_in_half` test admits both. `crates/paging` needed no change, which is the second time
this port has been able to say that.

`virt_to_phys` therefore has two branches, and that is not a wart: the kernel hands it linker
symbols (`memory::image_start`) as well as pointers that came out of `phys_to_virt`
(`sched`, `kmem`). Linux's `__pa()` makes the same distinction for the same reason. `phys_to_virt`
has one branch, because everything physical is in the direct map.

`mmu::device_va`, the foot gun that reached device registers through the identity map because the
direct map could not, is deleted. So is the identity map itself, which was a complete alias of
physical memory sitting in the half user programs get.

### 4. Two names in the arch contract are aarch64's and do not stretch

- `arch::psci_cpu_on` is an ARM firmware interface's name. RISC-V already had to implement it as
  an SBI call underneath. x86 has no third mechanism to hide behind it, because SMP bring-up here is
  INIT followed by two STARTUP IPIs through the local APIC, naming a page below 1 MiB to begin
  executing at in 16-bit real mode. The operation is not "power on a CPU", it is "send an
  interrupt".
- `kernel_main`'s single pointer is called `dtb`. What arrives on x86 is `hvm_start_info`. The
  *shape* is right (one pointer to everything discoverable) and only the name is wrong.

Both are naming decisions, so both are calef's; a lane records them rather than deciding.

## The bug worth knowing about

Loading a segment register in long mode destroys that segment's base MSR. `gs`'s base is where
this kernel keeps its per-CPU pointer, so `segments::init`'s reload of the data selectors silently
zeroed it, and the next `println!` dereferenced null through the console lock's per-CPU rank check.

It presented as an instruction fetch from the middle of a static several frames away, with a
register dump showing a perfectly correct GDT, TSS and IDT. Nothing about the symptom pointed at the
cause. The fix saves and restores the base around the reload, inside `segments::init`, so the
ordering constraint stops existing rather than being documented for callers to remember.

## The SMP bug that was a counting bug (2026-09-19)

For a month `arch::x86_64::ap_boot`'s BUGS #1 said a secondary core "fails intermittently", with two
refuted hypotheses beside it. No core was failing. `cpu_start` read the online count before the
INIT, then read it *again* after the STARTUP IPIs and waited for it to move from the second value. A
core that checked in during the 200 µs settle delay had already moved it. So the loop waited ten
seconds for an increment nobody would make and reported a running core as absent. The tell had been
in every failing transcript: the "failed" core printed its own `cr4.smep : set on core N` line, which
only that core's `secondary_main` can print, one line above `smp: cpu N did not start`.

26 of 40 four-core boots showed it before the fix; 80 of 80 boots at three, four and eight cores
brought every core online after it. It also reached two cores (the first secondary is as able to be
quick as any other), which is the UEFI leg's one-in-three failure recorded in
`design/roadmap/0412-the-uefi-boot-gate-asserts-two-cores-that-do-not-always-start.md`.
`ap_boot.rs`'s BUGS has the evidence, including the instrumented build that settled it.

## Where TSO pays out

Rule #4 says to assume weak memory ordering because ARM is the weak one and that is a gift. This is
the port where the bet settles, and it settles in the direction it was made: code proven correct
under ARM's model and RISC-V's RVWMO is correct on x86's TSO by construction.
`direct_memory_access_write_barrier` is an `sfence` here and is nearly free, because stores are
already globally ordered; the fence is there only for non-temporal stores and write-combining
memory, which TSO does not cover.

`sync_icache` does nothing on x86, and that is the one place this architecture's complexity buys
something: the instruction cache is architecturally coherent with the data caches. aarch64 needs a
clean/invalidate loop and a broadcast; RISC-V needs `fence.i` locally and an SBI RFENCE remotely, and
getting that wrong is what hung init on first silicon (notes/visionfive2.md).
