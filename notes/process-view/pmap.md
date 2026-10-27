# `pmap` from the prompt: what it costs, and the options

Milestone 126 (the `procps` package: who else is running) cannot be BUILT while `pmap` from the
prompt is undecided, and nobody had priced it. This page prices it. It gives options and no winner,
because they touch the syscall surface and §142's retention rule; calef's ruling is at the end, and
the decision now lives in milestone 602 (`pmap` from the prompt). Written 2026-09-26 (UTC) by the lane `proposal/126-pmap`, at base `484f3ebe2`.

*Name: provisional, for the stem `pmap`.
Naming is calef's; `script/names --unratified` lists each stem.*

## What is being decided

How a person at the shell prompt gets a listing of what another running job has mapped. Section 5 of
`notes/process-view/remaining-procps-programs.md` (pull request #1349, not yet on `main`) framed it as two
changes the tree has not made:

1. A non-empty retention. A spawner keeps an `ENUMERATE`-narrowed view of its child's address space.
   §142 (what a spawner retains over a child after `START`) made retention a declared field on
   `ChildEndowment` and declared it empty.
2. A lifecycle change. `ThreadControlBlock::CONFIGURE` removes the space from the registry that
   `address_space::LIST` reads, so every live space reads as empty.

That framing is one route. Another route needs neither change.

## The premises, checked

Both are true. Three details change what they cost.

### The lifecycle premise holds, and it has a second edge

`sched::configure_thread_control_block` calls `user::take_user_address_space`, which is
`USER_SPACES.lock().remove(name)`. The space moves into `Thread::space`. `address_space_list` in
`kernel/src/syscall.rs` resolves the capability's name through `user_address_space_root`, finds
nothing, and answers `DONE` with no entries. The syscall arm then deletes the caller's slot.

The same registry backs `address_space::MAP_INTO`. Today a `WRITE` copy of the capability that
outlives `CONFIGURE` is inert for both methods. Keeping the name resolvable re-arms `MAP_INTO` on
every such copy, which would let a builder map pages into a running child. So the lifecycle change
is not only "keep it listable". It must also say, per method, what a bound space still accepts.

### The retention premise holds, and the spawner is not the shell

`crates/supervision_protocol`'s `Retention` has two variants: `Nothing` and `ThreadControlBlock {
reason }`. There is no variant for an address space.

The spawner of a prompt job is the progenitor (`crates/system_initializer`), not `swish`. The shell
sends a spawn request and the progenitor calls `build_child`. So "the shell retains a view" means
the progenitor retains one per live job and hands it over when asked. The progenitor's measured peak
is 23 capabilities against `CAPABILITY_TABLE_SLOTS` of 32
(`kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED`). Nine slots are left since calef raised the table
from 24 on 2026-09-27; when this was written there was one.

### The program and the kernel method already exist

`abi::address_space::LIST` is built, gated by `ENUMERATE`, and proven in `kernel::user::pmap_tests`.
`crates/pmap` and `components/src/pmap.rs` are built and ratified. What is missing is a live source
of authority for the program to be handed. Both files record that gap in their `BUGS`. `pmap <tid>`
also needs an operand, which section 5 files under milestone 47 (navigation and naming).

## What the tree already does in the analogous case

`ps` is the analogue, and it did not retain anything per child. `Manifest::domain` makes the
progenitor place its supervision endpoint `deaths` in slot 7, narrowed to `ENUMERATE`
(`system_initializer`, `placed_buf[..] = (DOMAIN_SLOT, deaths, ENUMERATE)`). One capability covers
every job, because §26 (the fault endpoint) made the endpoint one per supervisor. `ps`, `pgrep` and
`top` hold it today.

calef's 2026-09-26 ruling that the process view is the supervision domain fixes the scope. Its
section is on `maintainer/126-followups` (pull request #1359), not yet numbered on `main`. A viewer
with `ENUMERATE` on an endpoint sees the threads that endpoint directly supervises, and nothing
else. Membership is `capability::survey_includes`, and a Kani proof holds the view and `REAP` to one
scope.

§142 named the supervision endpoint (its R1) as the likely successor for authority over a live
child. It deferred R1 because R1 would act on a live thread, crossing the line that §32 (a
supervisor may collect a corpse without being able to build one) draws at the corpse. Listing
mappings is looking, not acting, so it does not cross that line. It does widen what `ENUMERATE` on
that endpoint reveals.

§114 (`ENUMERATE` extends to the address-space object) required an audit of every holder before a
method made the bit live. Any option below that makes `ENUMERATE` reveal more owes the same audit.

## Prior art, read 2026-09-26

### Linux

`proc_pid_maps(5)`: "Permission to access this file is governed by a ptrace access mode
PTRACE_MODE_READ_FSCREDS check". `ptrace(2)` gives the check's order. Access is always allowed in
the same thread group. Otherwise the target's real, effective and saved IDs must match the caller's
filesystem IDs, or the caller must hold `CAP_SYS_PTRACE`. A target that is not dumpable is refused
without that capability. The LSM, Yama for example, decides last. The name is ambient (a pid) and
the authority is a check on the caller's identity at open.

### Fuchsia

`zx_object_get_info` with `ZX_INFO_PROCESS_MAPS` takes a process handle and returns a depth-first
walk of the address space, VMARs and mappings. The page contradicts itself on the right. The topic
entry says "handle type: Process, with ZX_RIGHT_READ". The Rights section says the handle "must be
of type ZX_OBJ_TYPE_PROCESS and have ZX_RIGHT_INSPECT". Either way the authority is a handle to the
process as a whole. Whoever launched the process holds that handle. How a tool at a shell comes by
one was not read here.

### seL4

No invocation lists a VSpace's mappings. The aarch64 VSpace methods are cache maintenance only
(`Clean_Data`, `Invalidate_Data`, `CleanInvalidate_Data`, `Unify_Instruction`), read from
`libsel4/sel4_arch_include/aarch64/interfaces/object-api-sel4-arch.xml`. A page offers `GetAddress`,
"Get the physical address of the underlying frame." A seL4 system that wants a map keeps the
bookkeeping in userspace, in whatever server did the mapping. That last sentence is recalled, not
read.

### What the three say about this choice

Linux and Fuchsia both scope the listing to the whole process. Neither scopes it to a narrowed view
of the address space alone. seL4 has no kernel answer at all. None of the three routes the authority
through a supervision relationship, so option B below has no direct precedent. Fuchsia's job handle
with `ZX_RIGHT_ENUMERATE` (quoted in that ruling) is the nearest: a parent-scoped right to name
children.

## The options

### A. Build both changes

The progenitor keeps an `ENUMERATE`-only copy of each job's address space, and a bound space stays
resolvable for `LIST`.

The parts:

- A `Retention` variant for an address space, with a reason, like `ThreadControlBlock { reason }`.
  Userspace only and reversible.
- A narrowed copy taken before `CONFIGURE`, since `CONFIGURE` deletes the caller's slot.
- A kernel lifecycle rule. Either the registry keeps a bound entry that resolves through the owning
  thread, or `Thread` holds the space by name and the registry keeps it. Both must stop resolving at
  thread death. Both must refuse `MAP_INTO` on a bound space, or re-arm it deliberately.
- A walk that cannot outlive the space. `address_space_list` reads the root and drops the lock
  before walking. A bound space can die mid-walk, which today's unbound spaces rarely do.
- Registry size. `MAX_USER_SPACES` is 32 and `sched::MAX_THREADS` is 256. The registry holds only
  unbound spaces today. Holding every live one may need it raised. The live count is unmeasured.
- Slots. One retained capability per live job in the progenitor, against one spare slot. This forces
  `CAPABILITY_TABLE_SLOTS` up, which milestone 230 (the shell check is red) last raised.
- A way to ask for job N's view. Either a spawn-request field on the shell's wire, or the progenitor
  mapping a tid to a slot. The first is a wire format.

What it makes true: `pmap` holds exactly one space, and nothing else about the job. That is the
narrowest grant any option offers. It costs the most, in parts and in slots.

### B. Ask through the supervision domain

A new method on `Rendezvous`, say `LIST_MAPPINGS(tid, cursor)`, gated by `ENUMERATE`. The kernel
checks `survey_includes` for the named tid and walks `Thread::space` directly. `pmap` declares
`Manifest::domain` and reads slot 7, the way `ps` does.

The parts:

- One method number on `abi::rendezvous` (7), a syscall arm the size of `SURVEY`'s (about 35 lines),
  and a scheduler function. `survey_supervised` is 39 lines today.
- No retention, no registry change, no slots. The registry premise stops mattering.
- A `grant_plan` variant for `pmap` with `domain: true`, and milestone 47's operand.
- The same walk-lifetime question as A, read under the scheduler's lock rather than the registry's.

It cannot ride `SURVEY`'s record selector. calef ruled on 2026-09-21 that a new per-thread fact is a
new record value. A mapping list is a second walk inside the first, with its own cursor, and the
three argument registers do not fit it.

What it costs that is not code: `ENUMERATE` on a supervision endpoint stops meaning "name the
members" and starts meaning "name them and read their layouts". `ps`, `pgrep` and `top` hold that
bit today and would gain the power. That is §114's audit shape again. The alternative is a fifth
rights bit, and `Rights::ALL` is `0b1111`, so that is an ABI change to `abi::rights`. It also makes
the domain, not the space, the unit of consent. A viewer sees every member's map or none.

### C. A scalar instead of a map

A `SURVEY` record, say `PAGES`, giving each member's mapped page count. This is exactly the shape
the 2026-09-21 ruling set for a new per-thread fact. `ps` and `top` gain a size column with no new
method and no new holder. It does not give `pmap`. It answers "how big is it", which is most of what
a person reaches for `pmap` to learn. It composes with every other option, including D.

The cost is one record constant, which is a wire value, and a count the kernel either walks for (one
entry per page, `revoke::list_mapping`) or keeps. Neither is measured.

### D. Refuse `pmap` from the prompt

`pmap` stays a program proven in `kernel::user::pmap_tests` and reachable from nowhere else. Its
`BUGS` already say so. Milestone 126's block records the refusal and can be marked BUILT.

What a user loses: seeing which pages a running job holds. That is how a person finds a leaking
heap, a shared page two jobs both map, or a device window a driver kept. `free` and `vmstat` (pull
request #1360) give machine totals, not per-job ones. Option C recovers the size; nothing recovers
the layout.

### Considered and refused

- Snapshot at build. The progenitor lists the space before `CONFIGURE`, while its own capability
  still resolves, and keeps the answer. Refused because it reports the image, not the process: any
  page mapped after `START` is missing, so it is wrong in the case `pmap` exists for.
- Retain the thread capability and add a look method to it. This is §142's R2 for looking. Refused
  for §142's reason: it puts an any-state verb on a capability whose three verbs are embryo-only,
  and it costs a slot per child like A.
- The child reports its own map. Refused because the subject vouches for itself, and every program
  would have to implement the protocol.
- A separate namespace object for processes. The same ruling refused it on 2026-09-26.

## The table for calef

| | A: retain the space | B: ask the domain | C: a size record | D: refuse |
|---|---|---|---|---|
| New method number | none (`LIST` exists) | one, on `Rendezvous` | none; one record value | none |
| Kernel lifecycle change | yes, and `MAP_INTO` must be settled | no | no | no |
| Progenitor slots | one per live job, 1 spare | none (slot 7 reused) | none | none |
| Who gains power | nobody but `pmap` | every `ENUMERATE` holder on a domain, or a new rights bit | `ps`, `top` gain a size | nobody |
| Unit of consent | one space | the whole domain | the whole domain | n/a |
| What the user gets | the map | the map | the size | nothing new |
| Overturns a record | extends §142 past `Nothing` | none; widens §114's bit to a new object | none | none |
| Reversible | no: lifecycle and a wire field | no: a method number | mostly: one additive value | yes |

## The seven questions

1. Other options and why they lost: the four refusals above, each with its reason.
2. The analogous case: `ps` over `Manifest::domain`, one capability for every job. It points at B.
3. Prior art: Linux, Fuchsia and seL4 above, read on 2026-09-26. One seL4 sentence is recalled and
   marked.
4. The premises: true, with two corrections. The spawner is the progenitor, not the shell. Keeping a
   bound space resolvable re-arms `MAP_INTO` as well as `LIST`.
5. Cost: slots, registry size and method counts are measured above. Line counts for B are the
   `SURVEY` analogue's. The live count of address spaces and the walk-lifetime fix are not measured.
6. Reversibility: A and B each spend something irreversible. Nobody has acted on either yet. `ps`,
   `pgrep` and `top` have acted on today's meaning of `ENUMERATE`, which B would change.
7. Same cost for both: A gives the narrower grant, one space and nothing else. B gives fewer moving
   parts and matches the tree's analogue. That is a real design fork, not an effort argument, and it
   is calef's.

## The ruling

calef, 2026-09-26 (UTC), on #1365: *"take pmap out of milestone 126, and pick among A to D later."*
`pmap` from the prompt is now milestone 602 (`pmap` from the prompt), provisional number, whose
open decision is the table above. The choice among A, B, C and D is deferred, not refused.

## At the prompt, once it can be reached

Nothing at the interactive prompt can grant `components/src/pmap.rs` an address space. `ps` reaches
the shell because `Manifest::domain` tells `system_initializer` which live supervision endpoint to
place. There is no such manifest field or wiring for an address space. Since §249 (a running address
space stays nameable) a builder can keep a copy, so the gap is the wiring, and the design for
handing a narrowed view to a third party. `kernel::user::pmap_tests` proves the program end to end
against a real space, and this is what it prints there:

```text
$ pmap
              VA  PERM
      0x00400000  r-x
      0x00500000  rw-

$ pmap        pmap: this address space may be mapped into, but not looked at: no ENUMERATE
```
