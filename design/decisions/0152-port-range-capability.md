---
status: DECIDED
raised: 2026-09-15
decided: 2026-09-15
ratified_by: calef
---

# 152. The port-range capability: object and method semantics on the syscall surface

calef, 2026-09-15, ratified the object, its one method, and the x86-only scope together. He ratified
them when milestone 299 (the x86 port-range capability) landed the working driver behind them. This
section records the one thing milestone 299 puts on the capability surface. The reason is that a new
object type there is calef's to ratify (§10 (process model), §16 (object revocation)). The *move
fast on what can be undone* tenet also files the syscall surface under the irreversible category.
**The provisional names for the object, its method, and their fields are not ratified here.** They
stay on the worklist (`script/names`); calef takes those separately. DECISIONS §121 (what a device
capability is) decided *that* x86 gets a port capability and *why* (the reversal at the end of that
file). This decides *what the object is and what invoking it does*, the part §121 named but left for
the milestone.

## What it names

`Object::PortRange(u16, u16)` names a contiguous run of x86 I/O ports, `(base, count)`. It is the
device-capability kind for the one class of device with no page: x86's legacy port-I/O hardware,
reached only by `in`/`out`. The MMU cannot grant or deny that hardware, because there is no page
table in front of the port space. It is the honest analogue of `Object::DeviceFrame`, which names a
device's MMIO page and is enforced by the MMU. `PortRange` names a device's ports and is enforced
by the CPU's TSS I/O permission bitmap.

- The granularity is a range, because a device occupies a run of consecutive ports. A 16550 UART
  is eight: COM1 is `0x3F8..=0x3FF`. A capability names what the hardware names. It is the
  port-space twin of a `DeviceFrame` naming a page, or a `PageFrame(base, count)` naming a run of
  frames.
- `(u16, u16)`, because a port number is 16 bits (`in`/`out` address exactly 64 Ki ports) and a
  count never exceeds that. The pair fits the capability enum's existing 16-byte payload, so the
  object does not grow a capability-table slot.
- It is minted by the kernel, once, at boot, the way a `DeviceFrame` is. Only the kernel knows a
  machine's device ports. The progenitor is handed COM1's `PortRange(0x3F8, 8)` with `GRANT` and
  delegates it to the console and input drivers it builds.
- The object is `x86_64`-only, unlike `DeviceFrame` (which every architecture has, because MMIO is
  universal). Port I/O exists on no other architecture, so omitting it there is not a parity gap
  (§19 (architectural parity is a tenet)). It is the absence of the hardware. Compiling the variant
  only where it exists also keeps a new enum arm off the other two architectures' syscall
  dispatcher. The IPC round trip's instruction-footprint bound is measured against that dispatcher
  (`script/fastpath-footprint`). A variant present everywhere grew riscv64's dispatcher past that
  bound for hardware it can never name.

## What invoking it does

Almost nothing, on purpose. Enforcement is at the context switch, not on the syscall path, so a
holder never invokes the capability to *use* the ports. It executes `in`/`out` directly. On
switch-in, the kernel writes the holder's ports into the current core's TSS bitmap, so those
`in`/`out` are permitted from ring 3 and no others (the lazy write; see §121's 2026-08-25
refinement and `notes/benchmarks.md`). A thread that holds no `PortRange` may touch no port at all.

The one method is `PortRange::REVOKE` (`abi::port_range::REVOKE`, method number 1), the take-back a
live driver replacement needs. It has exactly `DeviceFrame`'s asymmetric semantics, and for the
same reason. It deletes every `PortRange` capability naming this range from every other thread and
clears the cached grant. So those holders fault on their next `in`/`out`, while the caller keeps its
own. It needs `GRANT` (you were trusted to lend the ports on, so you may take them back). The
asymmetry is forced. The kernel mints a port capability once, at boot, so a symmetric revoke would
strand the device forever. No other method exists; `MAP` has no meaning for an object with no page.

## What it does not do

- **It does not replace `DeviceFrame`.** This is a *second* device-capability kind, for the one
  class with no page. PCI/PCIe devices, whose registers are memory, are unchanged and keep the
  mapping-based capability on all three architectures.
- **It does not add a syscall number.** `REVOKE` is a method on the existing `SYSCALL_INVOKE`
  surface (its name spelled out by the ruling of 2026-11-11 (UTC)), within the established
  capability model. That is the bar §10/§16 set for a new method.
- It does not give a way to *acquire* a port from userspace. A `PortRange` is minted by the kernel
  and delegated (`CAP_INSERT`), narrowing rights, exactly like `DeviceFrame`. There is no
  retype-into-a-port, because a program cannot conjure hardware it was not handed.

## Names, ratified 2026-09-15

calef ratified the names in one pass, walking them by exposure. Two are renames the lane's first
draft carried. This section records them as performed, so a reader meets the ratified name only:

| name | what it is | note |
|---|---|---|
| `PortRange` | the object variant | twin of `PageFrame`/`DeviceFrame` |
| `abi::port_range` | the ABI method module | `snake_case` of `PortRange`, mirrors `page_frame` |
| `port_range::REVOKE` | the one method | matches `page_frame::REVOKE`'s take-back |
| `port_range_grant` | the per-thread field | **renamed from `port_grant`**, for the shared `port_range` stem |
| `set_port_range_grant` | the arch setter | **renamed from `set_port_grant`**, same reason |
| `revoke_port_range` / `_from_others` | the revoke.rs functions | verb, matches the revoke family |
| `X86_COM1_PORT_BASE` / `_COUNT` | COM1's ports (`0x3F8`, 8) | arch prefix, standard `COM1`, `PortRange`'s fields |
| `outb` / `inb` | userspace byte port I/O | the universal x86 term, the `elf`/`pci` case |
| `port_range_cap` | the capability constructor | shares the stem |
| `port_out` / `recv_then_port_out` | hand-assembled test fixtures | describe what each tiny program does |

The stem `port_range` was chosen to keep the family greppable as one string, the argument that
renamed `port_grant`/`set_port_grant`. These are internal symbols nobody types, so brevity buys
nothing. The shared stem buys a `git grep port_range` that finds the object, module, method, field,
setter and constructor together.

## BUGS

- **A thread caches one port range, not a set.** The grant the context switch reads
  (`Thread::port_range_grant`) is set at the one choke point a `PortRange` enters a thread
  (`thread_control_block_insert_cap`). It holds the *last* range inserted. Every real consumer
  holds exactly one (a console driver holds COM1), so this is within the design's pinned consumer.
  But a thread handed two disjoint ranges would reach only the second. Widening it to a small set
  is a future change with an obvious shape. It is not built because nothing needs it.
- A `PortRange` delegated to an already-running thread by `SEND_CAP` is not cached. The grant is a
  creation-time fact, set when an embryo is endowed, the same posture `cycle_counter_grant` takes.
  Runtime delegation to a live thread would leave the switch installing the old grant until the next
  insert-through-the-choke-point. No consumer does this.
- Revocation reaches one core, and since 2026-09-17 that is a window rather than a non-issue. This
  entry used to end *"the stale-bitmap window a multi-core machine would have does not exist"*, on
  the premise that x86 runs a single core because `smp::bring_up_secondaries` refuses there.
  `smp::seat_cpus_from_acpi` made that premise false, and the conclusion inverted with it. Milestone
  313 (the security audit)'s security audit found the reset still core-local while the tour boots
  two cores under OVMF and four on xenon. So a revoked holder running on another core keeps that
  core's TSS bitmap until its next context switch, at most one tick. During that window its
  `in`/`out` succeed against a capability that no longer exists.

  The audit accepted the window rather than fixing it, with its reason. The window is bounded, it
  cannot reopen, and no consumer holds a port on two cores today. The limitation is recorded where
  a reader meets the code, in a `BUGS` section at
  `arch::x86_64::segments::revoke_installed_port_grant`. The closing move is
  `design/roadmap/proposals/a-port-revoke-that-reaches-every-core.md`.

  What this entry got right is the part worth keeping. The core-local reset was written as the
  step a future SMP x86 would broadcast by IPI, the shape of the TLB shootdown this tree already
  has. So the generalization is a broadcast rather than a redesign. That held. Only the sentence
  claiming the window did not exist was wrong. It was wrong because a fact about another subsystem
  changed underneath it.
