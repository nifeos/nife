---
status: PROPOSED
raised: 2026-11-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A process reads its window addresses from a startup page

Raised by the lane `lane/window-addresses-from-handoff`, asked by the architect to move the
addresses of every component's memory windows out of the component's source and into what the
spawner hands it. Ruled 2026-11-10 (UTC): option A below, a startup page. The ruling covers the
page's new interface id and reopens the deferral in §15 (the native ABI).

Name: provisional. The slug, and every name below, is the lane's.

Reuse: the startup page follows seL4's `BootInfo` (§15 (the native ABI) names it) and the read-only
pages `std_runtime_contract` already places. A search of `crates/` and `vendor/` for an auxiliary
vector or boot-information reader found two, neither a window table: relibc's, which reads Linux's
auxiliary vector, and `machine_discovery`'s, which reads the loader's handoff to the kernel.

## The problem

A component finds each of its memory windows (a shared page, a device register page, a DMA region,
a transfer buffer) at a virtual address compiled into it. The spawner that maps the window compiles
in the same number. Two costs follow.

1. Two components can never share one address space, because nothing stops their numbers from
   colliding; `address_space_map`'s own `BUGS` section says it checks pair pages for membership
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
- Programs that never share a space reuse addresses on purpose: `0x0090_0000` is the
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

So the spawner has no room to tell four of the five drivers where their windows are. The fifth, xHCI, has one free word, room for its two window addresses. Smaller
components (the keyboard driver, the credentialer, the disk tree) leave words free as well, but packing an address
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

Ruled 2026-11-10 (UTC): A, a startup page. The spawner writes one read-only page listing each
window it mapped, as `(kind, address, pages)` entries, and maps it at one fixed address in the
runtime band. A component reads `startup_page::window(KIND)` instead of a constant. One address
stays fixed, the page's own. Each `kind` is a number in the contract crate both sides link, so the
agreement is code. The kernel's spawners and `system_initializer` both write the page.

Its cost: a new contract crate with its own interface identity, one fixed page added to
`address_space_map`, and one page mapped per spawn. A page alone in its tables costs 1,245 ticks
per spawn (notes/benchmarks/spawn-el0.md); placed beside the runtime pages it costs a leaf. Each
component changes a constant into a lookup, and its spawner adds one entry per window.

The options it was chosen from:

- A. A startup page, as above.
- B. The component maps its own windows. The spawner grants each window as a frame capability
  and the component maps it with `address_space::MAP_INTO` where its own allocator says. No address
  stays fixed at all. It widens every component's authority (a capability over its own address
  space with write rights, so it may remap its own pages), spends a capability slot per window, and
  moves mapping and its failure paths into every component.
- C. A fourth entry word pointing at a table the spawner places anywhere. It changes the
  process-entry contract on all three architectures, and a userspace spawner cannot pass it without
  a change to the spawn method's arguments, which is the syscall surface.
- D. Keep fixed addresses, and make every pair code. Each of the 38 prose pairs becomes one
  constant in a contract crate that both sides link, with a compile-time check that one process's
  windows are disjoint. It fixes the second cost above and not the first.

Why A over each:

- Over B: A changes no component's authority and adds no capability, and a component reads a table
  where B has it map, check and unwind. B is the more capability-native shape; it lost on the
  authority it adds, not on its work.
- Over C: A touches neither the entry contract nor the syscall surface.
- Over D: D leaves the collision, which is the half of the problem the request names first.
- At equal cost: A still wins over B and C. It wins over D because D does not solve the problem; D
  is less work, and that is the only argument for it.

Reversibility: every converted component reads the page's layout, so once several read it, a
change is a coordinated change across all of them. The interface id, once minted, is never reused.

## Programs from outside the tree

The page is the only address in the public contract. It sits at one fixed address and carries an
interface id and a version, as the other wire contracts do (`environment_contract`,
`address_space_map`). A program walks its entries, takes the kinds it knows, and skips any kind it
does not know. So a later kernel may add kinds, and may move a window between releases, without
breaking a program built against an earlier page. The version changes only when the entry format
itself changes.

The other options serve an outside program less well:

- B asks every program to hold a capability over its own address space, authority that a program
  which only reads its windows has no use for.
- C hands the program a pointer to a table, in a register this contract does not have to spare; the
  table still needs a format, so C is A's page with a second, harder delivery.
- D helps only code built in this tree, against the same contract crate. For anyone else it turns
  every window address into a public promise that no release may move.

One point stays open. A program whose windows land at addresses the kernel chooses must not assume
where its own image sits either, or two programs in one space still collide on their images. Today
every program does assume it. `crates/user_mode_runtime/link.ld` links every component at
`address_space_map::IMAGE_BASE`, all three targets in `targets/` build with
`"relocation-model": "static"`, and `x86_64` adds `code-model: small`, which keeps every symbol
under 2 GiB. The loader accepts only a static executable: `crates/elf/src/parse.rs` refuses an
`ET_DYN` image with `Error::NeedsRelocation`. So the page frees the windows, and a shared address
space also needs position-independent images and a loader that relocates them. Neither is in this
work.

## Prior art

Each claim below names the source that states it.

- Linux, the auxiliary vector. It sits just above the argument list and the environment on the
  new process's stack, as type and value pairs. `getauxval(3)` returns 0 for a type it does not
  find (with `ENOENT` since glibc 2.19), so a reader tolerates an entry that is absent.
  `AT_SYSINFO_EHDR` gives the address of the vDSO page, a window the kernel places and the process
  learns at start. (man7.org, `getauxval(3)`.)
- seL4, `seL4_BootInfo`. The kernel hands the root task one structure describing its world: the
  ranges of capability slots holding its image frames and paging structures, the empty slots, and
  the untyped memory, with each untyped's physical address, size and device flag
  (`libsel4/include/sel4/bootinfo_types.h`). The root task's entry receives a pointer to it as its
  first argument (`sel4runtime`'s `src/start_root.c`). It is the closest design to A: a table the
  kernel writes and the program reads before anything else.
- Windows, the process environment block. A thread's environment block holds a pointer to the
  process environment block, whose documented fields include `Ldr`, the loaded-module list, and the
  process parameters (Microsoft Learn, `TEB` and `PEB`). Microsoft documents both as structures that
  may change between versions and asks programs to use functions instead.
- Fuchsia, the bootstrap message. The loader writes a message into a new channel and passes that
  channel's handle in the first argument register of `zx_process_start`. The message carries a
  handle table, each handle tagged by an info entry, and the essential handles include the
  process's own address-space region (fuchsia.dev, program loading). It is the closest design to B.
- Older Unix and Plan 9, fixed layouts. Seventh Edition Unix put the text segment at location 0
  (`a.out(5)`, magic 411). Plan 9's `a.out(6)` starts text at a page-aligned address and puts the
  stack at the top of the image; its `segattach(2)` lets a process name an address or pass 0 and
  take the system's choice. The fixed layouts are the closest designs to D.

Where nife differs. Linux, seL4 and Fuchsia each find their table through a register or the
initial stack; nife's three start words are already in use, so the page sits at an address fixed
by `address_space_map` instead. Fuchsia hands the process its address-space handle and lets it map;
in nife the capabilities a process holds already say what it may touch, and the page adds only
addresses, never authority. A Windows program reads modules and parameters from its block; nife's
page holds only window addresses.

## The work

Three pull requests, each green on main alone:

1. Additive: the contract crate, the page written by every kernel spawner and by
   `system_initializer` on all three architectures, with no reader yet; a host test that each
   spawner's entries match the mappings it makes.
2. Components convert in batches, each reading `window(KIND)` and dropping its constant; the
   drivers first, since they are the ones the three words cannot serve.
3. A ratchet in `script/lint` that counts literal window addresses in `components/src/` and only
   lets the count fall, ending at zero.

Nothing drives a shared address space in the tree yet: no spawner places two components in one
today. The architect's request is the driver.
