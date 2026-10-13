---
status: NOT-STARTED
raised: 2026-08-25
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 167. Handing a computed page to a userspace-built child: closing the x86_64 timebase page's delegation gap

A process built through the userspace ELF loader (`crates/supervision_protocol::build_child_space`)
cannot reach the real x86_64 timebase page that a kernel-built process gets automatically.

No hardware dependency; this is capability plumbing.

## Why the kernel-built path works

`kernel/src/user.rs`'s `load()` (and the hand-built spawns: `spawn_init`, and every
`spawn_<program>`-shaped kernel test harness) call `AddressSpace::map_physical` directly on the
`AddressSpace` struct they are constructing. This is privileged kernel code with unconditional
physical-memory access: it just maps the kernel's pre-computed timebase frame
(`x86_timebase_page_phys()`, cached in a kernel-internal static, filled in once from `CPUID` leaf
`0x15` or PIT calibration, see `arch::x86_64::timer`) into the new process's address space. No
capability is involved, because the kernel does not need permission to touch its own memory.

## Why `build_child_space` cannot do the same thing

`crates/supervision_protocol::build_child_space` runs in userspace, inside whatever process calls
it (`root_supervisor`, `spawner`, and any other builder role), and builds every page of the child it
constructs by `Untyped::RETYPE`ing a fresh frame out of its own budget
(`retype_page_frame_from(build_ut)`). It has no mechanism for mapping a specific, pre-existing physical
frame it does not itself own, because in this capability model a process can only map what it holds
a capability for, and nothing today mints or grants a capability naming the kernel's timebase frame
to any userspace process.

**Current mitigation, already built and documented (PR #476), so this degrades safely rather than
faulting.** `build_child_space` retypes a fresh, zeroed placeholder frame instead and maps that at
`counter_frequency_protocol::PAGE_VA`. `counter_frequency_protocol::TimebasePage::hz()` reads a zeroed page as `None`
("unknown"), never as a fabricated rate, and `user_rt::cntfrq()` falls back to its old hardcoded
`1_000_000_000` in that case. The honest gap is recorded in both crates' own `BUGS` sections:
`crates/counter_frequency_protocol`'s (*"A process built by `supervision_protocol::build_child_space` reads a
placeholder, not the kernel's real number... closing it needs a capability handed from whoever built
the calling process, forwarded through every generation of the supervision tree"*) and
`crates/supervision_protocol`'s own comment at the `build_child_space` call site making the identical
point. This milestone is where that closing happens.

## What it needs, three real pieces

1. **Give the timebase frame a real capability**, not just a kernel-internal physical address.
   Today it is raw kernel memory with no `Frame` object wrapping it at all (note: `Frame` is the
   current, correct name in this tree; DECISIONS §113 decided it should eventually become
   `PageFrame`, but that rename is not yet built, so a lane picking this up should use `Frame` and
   not assume the rename has landed). The kernel needs to mint one via `Untyped::RETYPE`, the same
   mechanism `notes/frames.md` documents for turning kernel-owned memory into a delegatable
   capability.
2. **Grant that capability to whichever userspace builder processes need to pass it on**
   (`root_supervisor`, `spawner`, and anything else that constructs children via
   `build_child_space`) as part of *their own* endowment at spawn time. This is a
   spawn-protocol/manifest change (`grant_plan`, `spawnproto`), not an x86-only one: the grant has
   to travel through the same machinery that hands these processes everything else they hold, even
   though the timebase page itself is x86_64-specific (aarch64 reads `CNTFRQ_EL0` directly and
   RISC-V reads a device-tree constant, so neither will ever need this particular grant; the
   *mechanism* for "hand a builder process a capability it can pass on to children" would be new,
   shared machinery those two architectures simply never exercise).
3. Extend `build_child_space`'s own signature to accept that capability and map it into
   whatever child it is building, via `Frame::MAP` (the two-step retype-then-map protocol
   `notes/frames.md` documents), then update every call site (`root_supervisor`, `spawner`, and any
   other current callers of `build_child_space`).

## Why it matters

Any program built via the userspace loader on x86_64 currently gets a silently-wrong `cntfrq()` (the
hardcoded 1GHz fallback) rather than the real calibrated rate. That matters for anything doing real
timing math over a process boundary this milestone doesn't reach: `timetable`'s own scheduling
logic was the one that first found the *kernel-built* side of this gap empirically (a page fault, not
a read of the call graph), so a userspace-loader-built consumer of real timing arithmetic is a
plausible next place this surfaces the same way.

## What this does not decide

The exact shape of the new spawn-protocol grant (a dedicated new manifest field, or reuse of an
existing generic-capability-passthrough mechanism if one already exists) is an implementation
decision for whoever picks this up. Check `grant_plan`/`spawnproto` for precedent before assuming a
new field is needed; this milestone names the gap and the mechanism to close it with, not the wire
shape.

## BUGS

- Unbuilt. Everything above is the plan; nothing in this milestone is built yet.

## Index row

PR #476 (milestone 161's `cntfrq` follow-up) found and documented, but did not close: `supervision_proto::build_child_space` (the userspace ELF loader `root_supervisor`/`spawner` use)
has no capability naming the kernel's real x86_64 timebase frame, so a child it builds gets a
zeroed placeholder and `cntfrq()` falls back to a hardcoded 1GHz. Needs a real `Frame` capability
for the page, a spawn-protocol grant path to hand it to builder processes, and a new `build_child_space` parameter to map it into children.
