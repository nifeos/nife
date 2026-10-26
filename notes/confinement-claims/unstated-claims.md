# Six claims that are stated nowhere

An appendix to [notes/confinement-claims.md](../confinement-claims.md), whose table lists what the
system claims. These are the limits it does not claim, found by enumerating the claims, which is
what step 1 of milestone 202 (every confinement test is a ritual until somebody breaks the
confinement and watches it fail) was for. The parent's heading counts five; a sixth was added on
2026-09-17.

## A confined component's timing is not confined

This was added 2026-09-02 with DECISIONS 139 (who may read the cycle counter and by what authority).
The words `timing`, `side channel` and `covert` appeared zero times in this note, in `DECISIONS.md`
and in `design/fatal-risks/README.md` before that decision. So nothing here was falsified by it; the
absence was the finding. seL4 states its own position in one clause, that exporting the PMU to user
level "opens the possibility of timing channels". This tree intends to publish cycle-denominated
numbers against seL4's while saying nothing.

The reason it cannot be claimed is measured rather than assumed. Two threads and a shared word
reconstruct a fine clock with no privileged instruction: 6.8 ns of usable resolution on cordoba
under load, matching Schwarz et al. (FC 2017). That holds on all three architectures, so gating a
cycle counter cannot deliver timing isolation on any of them. What the grant in DECISIONS 139 buys
is accountable authority. The cheap accurate path is granted rather than ambient, and the kernel
knows which threads hold it.

## A confined device's values are not confined, only its reach

`notes/untrusted-input-audit.md` says this in its own words ("the IOMMU confines placement, not
values"). No test asserts it, because it is a limit rather than a guarantee. It belongs beside the
table as a claim the system does not make, so that nobody reads row 13 as covering it. The audit's
finding 1 is the live consequence. The NVMe driver panics on a device-written index it does not
check, which the IOMMU cannot prevent and does not claim to.

## A `SURVEY` cursor counts threads the viewer cannot name

`kernel::user::survey_tests::the_survey_cursor_counts_threads_the_viewer_cannot_name` is a test that
*states a limit*, found by a 2026-08-17 audit. Row 8's claim is about which threads are shown.
Nothing claims the count is confined, and one is not.

## A confined component's interrupt target is not confined, and nothing here has ever asked

This was added 2026-09-03 by the research pass of DECISIONS §86 (whether an NVMe driver can leave
the kernel, and what capability would let it). That pass went looking for what a userspace NVMe
driver would need, and found this underneath every option it was pricing. An MSI or MSI-X message is
a memory write to an architecturally special address. So an IOMMU doing DMA remapping alone does not
confine it. A component that can write a device's MSI-X table can aim an interrupt at a vector it
was never given. Linux refuses to hand a device to an untrusted userspace driver on a machine without
interrupt remapping for this reason, and names its escape hatch `allow_unsafe_interrupts`.

The absence is the finding, and it is three-deep. Rows 12 and 13 are about where a device may *read
and write*, and neither covers where it may *interrupt*. When this was written, no boot this tree
runs could exercise the question even if a claim existed. `helpers/qemu-runner-x86_64.sh` attached
`-device intel-iommu` with no `intremap=on`. `helpers/qemu-runner-aarch64.sh` used `gic-version=2`,
which has no ITS. And no driver touches an MSI-X table. notes/non-volatile-memory-express.md's `BUGS`
says the NVMe controller is brought up with `IEN=0` and no MSI-X table is touched, so nothing has ever
come near it.

The first of those three was false, and the machine is what said so (milestone 317 (the
interrupt-remapping flags, and where MSI confinement actually lives), 2026-09-17). The x86_64
sentence above is a true reading of the runner script and a wrong conclusion, reached without
booting it. QEMU's `intremap` property is tri-state and defaults to `auto`. That resolves ON when
there is no in-kernel irqchip. So on patagonia `ECAP.IR` reads set on the default machine
(`ECAP = 0xf00f4a`), and clear only under an explicit `intremap=off` (`0xf42`). Interrupt remapping
has been offered in every x86_64 boot this tree has ever run, and nothing read the bit, so nobody
noticed. The guest now reports it. `NIFE_INTREMAP=off` is the flag that reaches the machine without
it, and one test asserts `GSTS.IRES` stays clear whatever `ECAP.IR` says.
`design/decisions/0086-el0-nvme-driver.md` still carries the uncorrected sentence. A lane may not
edit that file, so it is flagged rather than fixed.

The second is measured, and it holds. aarch64 is not one flag away. `gic-version=3` gives QEMU's
`virt` an ITS. It also moves `reg[1]` from the CPU interface to the redistributor, which
`memory::initialize`'s `intc@` name-prefix match hands to a GICv2 driver without ever reading
`compatible`. The measured result is a boot that says `GICv2` while printing a redistributor base,
with zero timer ticks. That is the bill of milestone 227 (a GICv3 driver, because GICv2 boots and
silently loses every interrupt); see design/roadmap/0317-interrupt-remapping-flags.md.

And there is a third architecture the paragraph above never mentions, which is the sharpest part.
riscv64 was not surveyed, and it is the one where the mechanism is closest to hand. The RISC-V IOMMU
puts MSI confinement *inside the device context this kernel already writes on every attach*.
Measured from a boot, `CAPS = 0x78c2cf4f10` has `MSI_FLAT` set, so every context is the extended
64-byte format. `attach` writes all four MSI words zero (`msiptp.MODE = Off`). No flag is needed
there and none is available. It is also the only one of the three that can never get a second
witness. No silicon ships the ratified RISC-V IOMMU (milestone 143 (silicon IOMMU)), which inverts
x86_64, where xenon is waiting.

So MSI confinement lives in three different places, and none of the three is exercised: a separate
IOMMU feature on x86_64, a separate device (the GICv3 ITS) on aarch64, and one mode field on riscv64.

The claim itself stays stated nowhere. Nothing writes an `IRTE`, nothing programs an MSI page table,
and nothing forges an MSI to see where it lands.

It is latent, not false, and stays latent while every component that can reach a BAR is the kernel.
It goes live the first time a driver leaves the kernel and wants interrupts instead of polling,
which is what §86 decides. Whatever §86 settles has to say who owns the page holding the MSI-X
table. The cheap first move was two runner flags, which milestone 317 took. x86_64's boot path was
already running with the hardware present, so what the flag buys there is the machine *without* it.
aarch64's does not reach the question at all. (`kernel-irqchip=split` turned out not to be needed on
patagonia: it is a KVM constraint, and there is no KVM here. 317's block has the three
invocations.) The hazard is the one milestone 202 already caught in the headline assertion of §31 (the foreign-language seam). Green
after turning the flags on proves nothing by itself. The falsification has to be a driver aiming an
interrupt where it was not given one, coming back red.

## The progenitor's bytes are unsigned

§14 (the project's direction) says so plainly in its own honest caveat. It is not in the table,
because it is not a confinement claim. It is the reason the confinement has an unverified component
inside it.

## The kernel cannot execute a confined component's code

This was added 2026-09-17 by the audit of milestone 313 (userspace confinement, read adversarially),
which found it stated nowhere and true on one architecture only by accident. No row says it and no
test asks.

On aarch64 it is `PXN`, which the encoder sets on every user page. On riscv64 it is the hardware,
which refuses a supervisor fetch from a `U` page unconditionally. On `x86_64` it is `CR4.SMEP`, which
nothing in this kernel set until that audit. So a user code page with `XD` clear was executable at
ring 0, whatever `crates/paging`'s decoder reported.

It is not a userspace escape on its own, since a kernel control-flow bug has to come first. That is
why it belongs here rather than in the table: it is the thing that turns any such bug into a full
one. `arch::x86_64::initialize` now sets the bit on every core whose CPUID offers it, and prints a
line either way. There is still no test, because a falsification would need ring 0 to survive its
own page fault. `SMAP` (its sibling, per-access and not free) stays off, with the reason
`mmu::permit_kernel_access_to_user_pages` records. Both are milestone 424 (a ring 0 that provably
cannot execute ring-3 pages, and SMAP with a number).
