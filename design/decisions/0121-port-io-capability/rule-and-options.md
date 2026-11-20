# §121 appendix: the rule this collides with, and the options

Two sections of [§121 (what a device capability is when the device has no page: x86 port
I/O)](../0121-port-io-capability.md). Moved out of that page on 2026-10-12 (UTC) under [§212 (a
prose budget)](../0212-a-prose-budget-for-every-document.md). The main page is complete without this
one, which exists to verify or challenge it. No ruled word changed. On the page, "What x86 actually
provides" sits between these two sections.

## The rule this collides with

On aarch64 and RISC-V, a device is a page. A driver holds a mapping with
`paging::Flags::user_device()`, its user-mode stores go straight to the hardware, and the MMU is what
enforces that it can touch that device and no other. That is the mechanism behind the whole
userspace-driver claim: the console server holds the UART's registers as a capability, and nothing
else in the system can reach them.

**x86's legacy devices are not in memory at all.** The 16550 COM ports, the PIT, the 8259 PICs and
the CMOS clock live in a separate 16-bit I/O address space reached only by the `in` and `out`
instructions. There is no page table in front of it, so there is nothing for a mapping to be *of*,
and `Flags::user_device()` has no meaning there.

This is not an x86 quirk to route around. It is the first case in this tree where **the object a
capability names is not memory**, and how it is answered decides the shape of every future
capability over something that is not a frame.

## The options

Option 1: a port-range capability, enforced by the TSS bitmap. A new capability type naming a
`(base, count)` range of ports. Granting it sets bits; revoking clears them; the scheduler writes the
holder's bitmap into the current CPU's TSS on switch-in.

- *For*: it is the honest analogue of the mapping model. The same "hold it or you cannot touch it"
  property, enforced by hardware, at the granularity a real driver needs (a 16550 is eight
  consecutive ports).
- *Against*: a new object type on the capability surface, which is the expensive kind of decision
  (§10, §16). It puts an 8 KiB copy per address space, or a shared bitmap plus a rewrite on switch,
  on the context-switch path, which is the path milestone 95 spent a lot of effort making fast.
  Revocation has to reach every CPU that might hold a stale bitmap, which is a shootdown protocol of
  its own.

Option 2: keep legacy devices in the kernel on x86, and grant only MMIO devices. The console
stays a kernel driver on this architecture; PCI devices, whose BARs *are* memory, get the existing
mapping-based capability and work identically on all three architectures.

- *For*: no new capability type, no context-switch cost, no shootdown. Everything modern is
  memory-mapped anyway, so the userspace-driver claim still holds for every device that matters on a
  machine built after about 1995, including everything on milestone 87's OptiPlex except the serial
  port.
- *Against*: it is a parity gap with a scope note, not parity. The one device the tree
  demonstrates userspace drivers with is exactly the one that would stay in the kernel, so the x86
  demo would be weaker in precisely the visible place. And it puts a device driver back in the kernel
  after milestone 8 spent the effort taking one out.

Option 3: an I/O-port *service*. A kernel-side port broker holding the ranges, with userspace
drivers issuing reads and writes as IPC.

- *For*: no new capability type in the system call surface; the grant is an endpoint, which the system
  already has; revocation is endpoint revocation, which already works.
- *Against*: it is a system call per register access, on a device whose driver polls a status register in
  a loop. Measured on nothing yet, but the shape is obviously wrong for a UART, and it reintroduces
  the kernel as the thing that actually touches the hardware, which is the arrangement being argued
  against.
