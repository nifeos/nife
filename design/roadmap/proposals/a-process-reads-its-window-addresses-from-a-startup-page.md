---
status: PROPOSED
raised: 2026-11-10
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# A process reads its window addresses from a startup page

Raised by the lane `lane/window-addresses-from-handoff`, asked by the architect to move the
addresses of every component's memory windows out of the component's source and into what the
spawner hands it. The lane surveyed the tree, found that the change needs a carrier the tree does
not have, and stopped before building one. This file holds the survey and the decision it waits
on. It turns nothing on.

Name: provisional. The slug, and every name below, is the lane's.

## The problem

A component finds each of its memory windows (a shared page, a device register page, a DMA region,
a transfer buffer) at a virtual address compiled into it. The spawner that maps the window compiles
in the same number. Two costs follow.

1. Two components can never share one address space, because nothing stops their numbers from
   colliding; `address_space_map`'s own `BUGS` section says pair pages are checked for membership
   and not for collisions within one process.
2. Each address is an agreement between two programs. Where both sides read one constant from a
   contract crate, that agreement is code. Where each side writes its own literal and a comment says
   "must match", it is prose, which §311 (a contract is code, not prose) refuses.

## The survey

Counted on the tree at the lane's base, with the commands in the pull request body.

- 36 of the 70 components in `components/src/` hold at least one literal window address
  (`pair_page(0x...)` or `runtime_window(0x...)`); `swish` counts once.
- At least 10 more read an address from a contract crate: `driver_process_contract` (the e1000e,
  DesignWare Ethernet and DesignWare mobile storage windows), `hot_swap_contract`,
  `login_contract`, `line_editor`, `std_runtime_contract` and `grant_plan`.
- 38 comments in `components/src/` and `kernel/src/user/` say an address "must match" one on the
  other side. Each is a pair held in prose. One example: the NVMe data plane is at
  `0x0098_0000` in both `kernel/src/user/non_volatile_memory_express_service.rs` and
  `components/src/non_volatile_memory_express.rs`, each with its own constant.
- The kernel spawners in `kernel/src/user/*_service.rs` place windows in 14 files, and the
  userspace builder `system_initializer` places 27 more in `src/layout.rs`.
- Addresses are reused on purpose across programs that never share a space: `0x0090_0000` is the
  DMA region of the e1000e, DesignWare Ethernet and virtio network servers, the entropy server and
  the keyboard driver.

### What a process is handed today

A spawn (`kernel/src/user/entry.rs`'s `Spawn`) hands a process three argument words, its
capabilities in slots, and its mappings. There is no startup structure beyond those three words.
The `Handoff` types in `driver_process_contract` are packings of the three words, one per driver,
and they carry no interface identity, version or magic: changing one is a change between two
programs that link one crate, not an identity bump.

The three words are full for the drivers that hold the most windows:

| driver | arg0 | arg1 | arg2 |
|---|---|---|---|
| NVMe | doorbell stride, blocks per transfer, ring entries | DMA physical base | namespace size |
| e1000e | role tag and station address | DMA physical base | listen grant |
| DesignWare Ethernet | role tag and station address | DMA physical base | listen grant |
| DesignWare mobile storage | role, clock, bus width, queue depth | window's first sector | window length |
| xHCI | DMA physical base | DMA pages and register mask | free |

So four of the five drivers cannot be told where their windows are without a new place to say it, and the fifth has one word for a data plane and up to sixteen register pages. Smaller components
(the keyboard driver, the credentialer, the disk tree) leave words free, but packing an address
into whichever word is free would mint one more wire format per component and would still leave
the drivers unconverted.

### What the tree already decided

§15 (the native ABI) considered a startup page, seL4's `BootInfo`, and deferred it: right tool, no
requirement yet, because the parent builds every child and knows its layout. It named the trigger
as a loader starting programs it did not build. The request here is a second trigger: programs
that may share one address space.

§111 (inert configuration is a validated page) chose a read-only page for configuration, and
`std_runtime_contract` already places such pages for std programs (`CONFIG_PAGE`, `ARGS_PAGE`,
`CLOCK_PAGE`) at fixed addresses in the runtime band.

`address_space_map` is a versioned contract (interface 2, version 1). `PROCESS_PAGES`, the band
the kernel maps into every process unasked, is full: its sixteen pages are the per-thread
current-CPU slots (`current_cpu_contract::THREAD_PAGE_SLOTS`).

## The decision

Where does a process learn its window addresses?

Recommended: **A, a startup page.** The spawner writes one read-only page listing each window it
mapped, as `(kind, address, pages)` entries, and maps it at one fixed address in the runtime band.
A component reads `startup_page::window(KIND)` instead of a constant. One address stays fixed, the
page's own, as Linux keeps one (the initial stack pointer) for its auxiliary vector. Each `kind` is
a number in the contract crate both sides link, so the agreement is code. The kernel's spawners and
`system_initializer` both write the page.

Its cost: a new contract crate with its own interface identity (an id minted once), one fixed page
added to `address_space_map` (a question for its version), and one page mapped per spawn. A page
alone in its tables costs 1,245 ticks per spawn (notes/benchmarks/spawn-el0.md); placed beside the
runtime pages it costs a leaf. Each component changes a constant into a lookup, and its spawner
adds one entry per window.

The options:

- **A. A startup page,** as above. Reopens §15's deferral of `BootInfo`.
- **B. The component maps its own windows.** The spawner grants each window as a frame capability
  and the component maps it with `address_space::MAP_INTO` where its own allocator says, as a
  Fuchsia process maps the VMOs in its bootstrap message and a Genode component attaches its
  dataspaces (both recalled, not reread). No address is fixed at all. It widens every component's
  authority (a capability over its own address space with write rights, so it may remap its own
  pages), spends a capability slot per window, and moves mapping and its failure paths
  into every component.
- **C. A fourth entry word** pointing at a table the spawner places anywhere. It changes the
  process-entry contract on all three architectures, and a userspace spawner cannot pass it without
  a change to the spawn method's arguments, which is the syscall surface.
- **D. Keep fixed addresses, and make every pair code.** Each of the 38 prose pairs becomes one
  constant in a contract crate that both sides link, with a compile-time check that one process's
  windows are disjoint. It fixes the second cost above and not the first.

Why A over each:

- Over B: A changes no component's authority and adds no capability, and a component reads a table
  where B has it map, check and unwind. B is the more capability-native shape; it is refused here
  for the authority it adds, not for its work.
- Over C: A touches neither the entry contract nor the syscall surface.
- Over D: D leaves the collision, which is the half of the problem the request names first.
- At equal cost: A is still chosen over B and C. Against D it is chosen because D does not solve
  the problem; D is less work, and that is the only argument for it.

Reversibility: the page's layout is read by every converted component, so once several read it, a
change is a coordinated change across all of them. The interface id, once minted, is never reused.
Both are why this waits for a ruling.

## What is blocked until it is answered

Every conversion. Under A the work is three steps, each green on main alone:

1. Additive: the contract crate, the page written by every kernel spawner and by
   `system_initializer`, with no reader yet; a host test that each spawner's entries match the
   mappings it makes.
2. Components convert in batches, each reading `window(KIND)` and dropping its constant; the
   drivers first, since they are the ones the three words cannot serve.
3. A ratchet in `script/lint` that counts literal window addresses in `components/src/` and only
   lets the count fall, ending at zero.

Under D, step 3 alone applies, counting prose pairs instead.

Nothing drives a shared address space in the tree yet: no two components are placed in one today.
The architect's request is the driver.
