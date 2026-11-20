---
status: DECIDED
raised: 2026-09-26
decided: 2026-09-26
ratified_by: calef
---

# 225. `free` sees the machine and your share: a region method and a withholdable memory page

calef, 2026-09-26 (UTC), on milestone 126 (the `procps` package: who else is running)'s memory
statistics fork: *"Rule 3."* *(Section number provisional until the merge queue lands it.)*

## The question

How `free` and `vmstat` learn about memory. `kernel/src/memory.rs` keeps the figures (`stats()`,
`free_page_frames()`), and today only the boot summary and kernel tests read them. Section 3 of
[`notes/process-view/remaining-procps-programs.md`](../../notes/process-view/remaining-procps-programs.md) asked whether
"how much memory is free" carries region-scoped authority like everything else here, or becomes the
second machine-wide ambient fact after monotonic time. It named two options and no winner, because
both touch the system call surface. calef ruled a third, which combines them.

## The decision

Shape 3, in two parts.

1. A per-region method on `MemoryRegion`, gated by `Rights::ENUMERATE`. It reports how much of the
   region behind the capability is committed. This is option 1's method, and it answers "your
   share". It is a new method inside the capability model, as §114 (`ENUMERATE` extends to the
   address-space object) was for `pmap`, not a new system call number.
2. A read-only machine memory page carrying the machine-wide figures. It is held as a capability,
   never ambient. Owner policy grants it to every login by default, the way `UNVOUCHED_MANIFEST`
   grants the clock and configuration pages under §219 (how the shell names an installed program to
   the spawner), and the owner can withhold it.

`free` prints a machine line always, and a "yours" line when the caller holds a region.

Left provisional, for the building lane to propose: the method's number, the page's layout, and
the page's name.

## Why

calef's reason: a `free` that does not show a non-root user the box's health is confusing. Option
1 alone gives a correct answer to a question nobody asked when they typed `free`.

The counter-case was weighed. Inside a Linux container, `free` reads the host's
`/proc/meminfo` and tells a program about memory it cannot have. LXCFS exists to fix exactly that:
its README says it makes procfs files "container aware". Shape 3 answers both readers at once. The
machine line is labeled as the machine, and the "yours" line is the limit that binds.

The page keeps the one property option 2 lacked. A figure delivered by a page someone was granted
can be withheld; a figure nobody holds cannot. Granting it by default costs one manifest field, and
the default is the owner's to change.

## Refused

- Option 1 alone. It is silent about the box, which is the confusion calef named.
- Option 2, a machine-wide figure no capability gates. It would be the first ambient fact about
  memory, and there would be no way to withhold it from a login that should not see it.

## Prior art

Each was offered from memory and then read against a primary source on 2026-09-26. Corrections are
stated.

- Mach (XNU). `mach_host.defs` declares `host_statistics` and `host_statistics64` on `host_t`, the
  host name port, which `mach_host_self` returns "by default". Control routines in
  `host_priv.defs` (`host_reboot`, `vm_wire`, `host_processors`) take `host_priv_t`, as does
  `mach_zone_info_for_zone`, the zone (slab) view. Confirmed as offered, with one caveat: the older
  OSF Mach 3 manual page describes `host_statistics` as taking the host control port, so the
  unprivileged read is XNU's choice rather than Mach's from the start.
- Fuchsia. `ZX_INFO_KMEM_STATS` needs a resource handle of kind `ZX_RSRC_KIND_SYSTEM` with base
  `ZX_RSRC_SYSTEM_INFO_BASE`, and it includes slab figures. `ZX_INFO_TASK_STATS` needs a process
  handle with `ZX_RIGHT_INSPECT`. Confirmed, with one correction: the per-task view takes a process
  handle, not any task's handle.
- Genode. The foundations book says each PD session "corresponds to a bank account", and that
  Genode "solely arbitrates the access to such resources". Per-component quota is confirmed. The
  claim that system-wide figures reach a component by the parent's routing policy was not
  confirmed from a primary source: core's `platform_info` ROM exists, but its contents were not
  read.
- Linux cgroups v2. `memory.current` is "the total amount of memory currently being used by the
  cgroup and its descendants"; `memory.max` is the hard limit. Confirmed. This is the "yours" line
  Linux has and `free` does not print.
- seL4. The FAQ: the kernel "has no heap", and memory for kernel objects is provided by user-level
  code through untyped capabilities. So the kernel has no memory statistics to report. Confirmed.
- Plan 9. `cons(3)`: `/dev/swap` "holds a text block giving memory usage statistics", reached
  after `bind #c /dev`, so visibility follows the namespace. Confirmed.

Shape 3 is closest to Mach's split, with Fuchsia's discipline on the machine view: readable by
default, but through a handle rather than ambiently.

## How reversible it is

Moderately. The method and the page are wire facts two programs agree on, which is why their
number, layout and name stay provisional until a lane proposes them. The default grant is policy
and can change. What cannot be taken back is the claim, once shipped, that a login sees the box.

## What it unblocks, and what changes shape

- `free` and `vmstat` can be built.
- `slabtop` becomes part 1 asked per object type, since kernel objects are carved from regions
  their holders own (milestone 14 (kernel objects from untyped) removed the slab).
- `tload` becomes a line in `top`'s summary, not a program.
- Swap figures, if the proposal on branch `proposal/swap` lands, follow the same two-line shape: a
  machine line from the page and a "yours" line from the region.
