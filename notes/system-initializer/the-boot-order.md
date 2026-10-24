# The system initializer's boot order and wiring

An appendix of [the system initializer note](../system-initializer.md): the reasons behind the
order `boot_steps.rs` builds the interactive system in, and the wiring of each step. Measurement,
the chain the kernel's trust root starts, is in [trusted-init.md](../trusted-init.md); supervision
and reaping are in [supervision.md](../supervision.md). The page budgets and how each was counted
are in [budgets.md](budgets.md), and the builders the spawn service calls after the boot are in
[children.md](children.md).

## The boot order is the capability table's budget

The progenitor builds every boot component from one capability table, and most of the boot's order
exists to keep that table's peak at or under `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED`, the
boot's measured peak, well below the table's size, `kernel::cap::CAPABILITY_TABLE_SLOTS`. Every
boot prints `capability slots: N of M at peak` (`kernel::cap::report_peak`), and
`script/swish-check` fails a boot whose peak goes above the recorded one. A build that runs out of slots inside `build_child` fails with no route to
a person: the boot reaches userspace and prints nothing.

Three facts drive the order.

- A kernel grant occupies its slot from the moment the progenitor starts until it is deleted. So
  a grant raises the baseline under every peak that comes before its deletion, not only the step
  that uses it.
- Every object the boot retypes lands in the first free slot. Once anything has been retyped, "slot
  N holds something" no longer means the kernel put it there, so every probe of a kernel grant
  (`is_granted`) is read before the first retype.
- The early peak is the terminal plumbing: six objects retyped before the console runs (the
  console's request, reply and shared page, the terminal endpoint and the shell's two pages). The
  shell's build is the next peak, and the login block is the highest.

From those, the steps in `boot_steps.rs` run in this order.

1. The capabilities the kernel granted for the test roles (`for_test_roles`) are deleted first.
   Only their slots matter, and nothing interactive reads them.
2. The device services come next: the graphical verdict, the firmware screen probe, entropy and
   the network. The virtio-rng and virtio-net trios are kernel grants, and this is the one point
   where their slots and the terminal plumbing's six are never live together. Each trio is
   released, built or not, before the terminal endpoint is retyped. A kernel-built entropy service
   is one slot, granted only when the trio was not.
3. The terminal stack: the console, the system log service and the line discipline. The console's
   request, reply and shared page are deleted once the console and `line_editor` hold their own
   narrowed copies.
4. The keystroke sources. Once `input` holds its narrowed copies, the UART's device and interrupt
   are deleted. That is a slot and an authority: a progenitor that kept them could hand the UART to
   anything it later builds.
5. The spawn channel and the supervision endpoint are retyped only now. Held through the three
   driver builds, they would raise the early peak.
6. The shell is built but not started. Its budget copy and its window-0 slice are deleted at once,
   and the configuration page is placed at a named slot after the build. The machine statistics
   page goes to the shell before the login block, and the progenitor keeps no copy, because a page
   held across that block would raise the highest peak.
7. The terminal's sink adapter, after the shell. Built earlier, it would hold its slots across the
   shell's build, which is a peak.
8. The login stack, after the shell has given back its transient capabilities.
9. The giveaway.

Steps 7 and 8 come before the giveaway for a second reason. They are system components, and the
root untyped is what the system is built from. Built after it, they would come out of
`PROGENITOR_RESERVED_PAGES`, the scratch budget sized for page tables, and the cost would surface
later as a child failing to map a scratch page.

Inside the login block, anything that lives past it is placed after the build that peaks there,
never in the build's `caps`. The run-unvouched endpoint is retyped and placed after `login`'s
build, into the slot the loader's transient address space has just given back. The durable
window's page is placed the same way. The identity provisioner's window-0 page is sliced and mapped
into its space only after the build has given back its request frame. The blobs `login` is handed
cost no slots: `supervision_protocol`'s `fill_and_map` holds one frame capability at a time.

## The stack the boot runs on

`boot`'s frame is live for the whole boot, because the spawn service runs inside it. The
progenitor's stack is `PROGENITOR_STACK_PAGES` (`kernel/src/user/progenitor_boot.rs`), and it has
overflowed when the boot held more locals. So every step is `#[inline(never)]`, and its temporaries
are gone before the spawn service starts.

The graphical stack's programs are measured inside `graphical_terminal_grants` and the builders, not
in the measured pass, for the same reason: three more `elf::Elf`s in `boot`'s frame overflowed it
under `package install`. Hashing three small programs twice costs less than the stack does.
`build_grant` is out of line for the spawn service's frame: an install runs `package_archive`'s
parser beneath it.

## What the measured pass reads, and what a refusal costs

`open_archive` reads the measurement table, `measured_boot::PROGRAM_MEASUREMENTS`. The kernel
vouched for that entry before it started the progenitor, so the table is worth what the progenitor
is worth. Bytes that are not UTF-8 become the empty table, which vouches for nothing: the console is
then refused and the boot stops. That is the direction a measured boot must be wrong in.

The package catalog and the index sources (`package_index::SOURCES_ENTRY`) are archive entries
above the table, so they are measured like programs. Each reads as empty when the table refuses it
or the archive has none, and empty vouches for no package and names no source.

A component the table refuses is treated as a component that is not there. What that costs is
decided by what the program was for.

| component | absent or refused |
|---|---|
| `console`, `line_editor`, `terminal_supervisor` | the boot traps, before anything can print |
| `input`, `swish`, `job_undertaker` | refused: `refuse_unvouched` says so, then halts; absent: the boot traps |
| `system_log` | the kernel keeps printing for itself |
| `terminal_sink_caretaker` | a second stream finds an empty slot |
| `entropy`, `net_stack`, `domain_name_resolver` | no random source, network or name resolution |
| `gpu_driver`, `display_terminal`, `keyboard_driver` | no `graphical_terminal` session |
| the four login programs | no login path |
| `fs_subtree_caretaker` | no directory grant; `rm` is refused |
| `fs_nameset_caretaker` | a name-set grant is refused, never widened to the subtree |

`job_undertaker` is required because without it the bounded job pool fills and the prompt stops
spawning. `refuse_unvouched` runs once the console and the line discipline are up, the earliest
point a person can read the progenitor and the latest at which nothing unmeasured has been built.

Whether the file server enforces subtree grants itself is read from the note in its own measured
bytes. Bytes the table refuses, a server with no note and an unreadable note all answer `false`,
so every doubt ends in a caretaker.

The programs the shell can spawn are measured in the same pass, before the giveaway, because the
announcement is the only thing the progenitor says. Only an entry the archive has can be a refusal:
a program the initial ramdisk never packed was never spawnable.

`login` needs program bytes it cannot read itself. The archive is reserved RAM that no capability
names, and `build_child` can map a page only through a `PageFrame` capability. So `login` is handed
copies as blobs: the subtree caretaker's image, the measurement table, and `user_timetable_keeper`
with `timetable`. The last two travel together or not at all, and without them `login` answers
`SCHEDULE` as a plain login. No program a scheduled job runs travels with them: the job runs what
the live activation generation names (Fork 8, ruled D by calef on 2026-09-27, #1377).

## The device services

The boot is the same minimal UART system on every machine, and graphics is launched from the prompt
(calef, 2026-09-30: *"I don't want graphics at boot and won't for a long time"*). A gpu's four
grants come together or not at all, probed with `is_granted`. When the table vouches for
`gpu_driver` and `display_terminal`, the spawn service keeps the four for every session it builds,
and the shell holds none. A keyboard's three are kept only beside them and only with a vouched
`keyboard_driver`; without one, a session reads keystrokes from the boot's line discipline. Any
grant no session can be built from is released at once. A firmware screen's terminal pair and the
gpu's grants never coexist, because the kernel wires one or the other.

A virtio device is probed by reading its transport: `invoke` answers `NoSuchSlot` on an ungranted
slot, and there is no `START` word left to say whether it was granted. The kernel grants each trio
whole or not at all. The DMA page carries its own physical base at `abi::virtio::DMA_PHYS_OFFSET`;
the progenitor maps the page, reads the base, gives the mapping up and delegates the frame.

The entropy service answers `READY` only after it has fetched a first bufferful from the device, so
the wait means a client that asks will be answered. The request endpoint is kept for the life of
the boot: `credentialer` gets a view of it, and the spawn service endows one to every child whose
manifest declares entropy. The progenitor is the only process that can hand a program a random
source, as it is the only one that can hand it a clock.

`net_stack` gets exactly what `kernel::user::virtio_service::wire_net_server` gives it, slot for
slot, because that is the endowment every network test proves. Its listen grant is
`NO_LISTEN_GRANT`: nothing at this prompt has asked to listen, and a listen grant is a policy
someone has to choose. The boot blocks until DHCP answers, because the lease is the stack's first
message and it serves nobody before sending it. A NIC with no DHCP server therefore never reaches a
prompt, which the crate's BUGS records. A stack the kernel built on an `e1000e` is taken the same
way, lease and all. The name resolver asks the lease's name server, needs entropy for its query
ids, and serves nobody until the spawn service grants a job a zone.

## The terminal stack and the shell

On aarch64 and riscv64 the UART is a page, so the console and `input` map it. On x86 COM1 is a port
range with nothing to map: each holds it as a `PortRange` capability, which the kernel's TSS I/O
bitmap reads, and neither invokes it by slot. With a screen beside the UART, the console's slot 2 is
the screen terminal's endpoint and slots 3 and 4 are its batching pair, a notification and a timer.
The notification is bound to the console's thread between `build_child` and `start_child`, because a
running thread cannot bind itself. The pair lets the console paint once per window rather than once
per write, and the console degrades to one paint per write without it. The console's endowment is
built as arrays with a count. The screen is a runtime fact and the port range a compile-time one, so
one literal per case would be one more place to get the slot order wrong.

`system_log` is built right after the console, because until it attaches the kernel prints for
itself and can splice with the console's bytes. Its ring grants are deleted whether or not it was
built, so none is carried across the login block.

The shell's slot 4 is the file service when the boot has one, `WRITE` without `GRANT`, which is all
`>` and `<` need. Its clock is last whatever else is granted, so the shell is told the slot in its
third start word. The clock and the configuration page are `READ` without `GRANT`: the shell can
read them and hand them to nothing it spawns. So manifests decide which children see them, as §43
(reading the clock is a page, setting it is a page you may write) has it for the clock. The shell
is built but not started, because the giveaway's negative control is printed through its output
page.

## The login stack

The login stack runs only on a boot with a filesystem, working entropy and all four programs. Its
order is a chain of things that must exist before something waits on them.

1. The password is drawn first. With no entropy there is no password, and the whole block is
   abandoned rather than provisioning something weaker.
2. `credentialer` is built and started, holding its view of the entropy endpoint.
3. `identity_provisioner` provisions the credential and the demo identity's home. The plaintext is
   staged in a page the progenitor maps briefly and wiped whatever the outcome.
4. The store is sealed whatever provisioning did, because `credentialer` must leave its first phase
   before `VERIFY` answers anything. A login against a partial store then denies cleanly.
5. Only now is `credentialer`'s readiness report read. It sends `RPT_READY` after `provision()`
   returns, which is after the seal, so a receive before the seal would never wake. A service that
   failed at start-up answers with its own failure word instead.
6. `login_audit_receiver` is started before `login`, so `login`'s blocking audit send has a
   receiver before anything can reach `login`.
7. `login` is built. It holds `WRITE` on the file service's page and not `READ`, because the
   progenitor holds only `WRITE | GRANT` and a writable mapping checks `WRITE` alone.

`login` holds the run-unvouched endpoint with `GRANT` and gives each session `WRITE` (§219 (how the
shell names an installed program to the spawner), gate D2). The boot prompt holds it with `WRITE`
alone, so the shell can present it and hand it to nothing. That one placement in `give_away_budget`
makes the unauthenticated boot prompt the owner's console. Ruling 1 of §221 (the boot prompt is the
owner's console) decided it. §221 as amended on 2026-10-06 (UTC) ends that console, and the
amendment is unbuilt: milestone 806 (concurrent login sessions) lists it. BUGS below records the
gap.

## The giveaway

The progenitor carves three bounded budgets off the construction root and deletes the root:
`PROGENITOR_RESERVED_PAGES` for itself, `JOBS_BUDGET_PAGES` for the prompt's jobs, and
`IMAGE_POOL_PAGES` for images run by path. The job pool is separate so that its watermark moves for
jobs only. A scratch page table carved between a job's split and its reap would sit above that job.
The LIFO return of §16 (object revocation: reclaim the objects a process built) would then find
the job is not the top and give back nothing.

It then proves the root is gone from the inside, on the two primitives that build things. Both must
answer `NoSuchSlot` (-1) rather than `NotPermitted` (-3): the capability is gone, not narrowed.
`script/swish-check` reads the sentence it prints. The shell's output page is given up with `UNMAP`
after that line, because `PageFrame::REVOKE` would take it from the shell too.

## BUGS

- The boot prompt holds the run-unvouched capability without a login. §221 as amended on 2026-10-06
  (UTC) says no console is the owner's without one. Nothing here builds that yet; milestone 806
  carries it.
