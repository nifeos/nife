# The progenitor, and loading a program from userspace

*(Milestone 19d. `kernel/src/user/entry.rs` `spawn_elf_child_builder`, and the builders and children
in `fixtures/` it enters, `report_child_builder` and `report_child` first. The loader described here lived in that file until milestone 96 (one init)
gave the tree one of them; it is `crates/supervision_protocol`'s `build_child` now, and every caller
reaches it there. What the loader does is unchanged, and the steps below are still the steps.)*

*This note said, until milestone 266, that the archive entry named `init` meant a different program
on each architecture and pointed at [milestone 166 (one boot loader)](../design/roadmap/0166-init-boot-parity.md)
as the open question. That is settled: the entry is `progenitor` on all three boards
and it is one program, `components/src/progenitor.rs`. Where the text below says init it is
describing milestone 19d, when the role and the demo catalog were the same binary.*

## The one thing 19d moves, and why it matters

Until 19d, when a program ran on nife the kernel read its file and set it up: it parsed
the ELF (the standard "here is a program: code here, data there, start at this address" format),
copied the pieces into memory, and started it. That parser lived inside the kernel.

Parsing a program file means processing bytes an attacker may have crafted, and a bug in a parser is
where exploits live. A bug in a parser *inside the kernel* is the worst kind: it compromises the
trusted core the whole §14 (project's) thesis rests on. So 19d moves the parser out, into an
ordinary confined program where a parser bug is that program's problem, confined by the same
capability walls as any workload.

That program is the progenitor: the first process, the one the kernel starts and from which
every other descends. Older records call it init.

## What still loads the progenitor (the honest residue)

Something has to load the *first* program, so the kernel keeps exactly enough loader for one: it
loads the progenitor through `boot_progenitor` and nothing else. The progenitor loads every
*other* program.
"The kernel loads exactly one program" is not a slogan we rounded up to; it is literally one call
site. (19d.2 removes the kernel's other loaders, the ones that wire up the console and shell
services today, by moving that wiring into the first process.)

## How the progenitor loads a child (the loader, in userspace, through the verbs)

`spawn_elf_child_builder` hands a builder three things: a building untyped budget (slot 0), a
report endpoint (slot 1, with `GRANT` so it can endow a child), and the whole initrd. The initrd is
mapped read-only at `INITIAL_RAMDISK_VA` so the builder can read the ELF. Its length arrives in `x1`.

`build_child` then does, entirely through the milestone-19 granular verbs, what the kernel's
`map_segments` used to do privileged:

1. `RETYPE_OBJ(ADDRESS_SPACE)`: a fresh address space out of init's budget.
2. For each ELF segment, page by page: `RETYPE` a frame, and `page_frame::MAP` it read/write into
   init's *own* scratch window to fill it (zero it, free `.bss`, then copy the segment's bytes).
   Then `MAP_INTO` the child at the segment's own virtual address with the segment's permissions
   (executable code via the `MAP_CODE` mode 19d added). Then `cap_delete` the frame cap so the
   16-slot capability table recycles the slot. A loader retypes hundreds of frames, so slot
   recycling is why `SYS_CAP_DELETE` exists.
3. A stack frame, mapped read/write.
4. `RETYPE_OBJ(THREAD_CONTROL_BLOCK)`, `CAP_INSERT` the report endpoint as the child's slot 0, `CONFIGURE` (entry
   from the ELF, the child's stack top, the address space), `START`. `START` carries the child's first
   three registers: `x0` is the role (which of the multi-role binary this instance is), and `x1`,
   `x2` are data the child needs before it can run. See "The argument to START" below.

The child is a second instance of the same multi-role binary, entered at the `CHILD` role. It
SENDs one word through the capability init granted it, and exits. Receiving that word is the proof:
init parsed a real ELF and produced a running thread, and the kernel never looked at the child's
bytes.

## The argument to START (milestone 19e)

Through 19d, `START` handed the child exactly one word, its role in `x0`, and that was enough: every
child was pure code selected by identity. A *least_authority_demo* breaks that. It computes `n*n`,
and `n` has to reach it before it runs. So 19e widened `START` to carry `x0`, `x1`, `x2`, the way a
function call carries arguments, and the loader passes the role in `x0` and the input in `x1`.

The plumbing is one value carried through the whole thread-creation path. `START`'s three
arguments land in `Thread::start_args` (`kernel/src/thread.rs`); `arm_for_start` writes them into
the faked switch frame as `x21/x22/x23`; the EL0 trampoline (`context.s`
`user_entry_trampoline`) moves those into `x0/x1/x2` right before dropping to the child's entry.
The child sees them as the arguments to its `_start(x0, x1, x2)`.

init's spawn service (the shell's `run <n>`) is the payoff. The shell SENDs `n`, and init builds a
least_authority_demo endowed with a result endpoint and `START`s it with `n` in `x1`. The
least_authority_demo squares it and SENDs the answer straight to the endpoint the shell is waiting
on. init only builds the pipe; it
never sees the number. The kernel test `the_least_authority_demo_builder_passes_its_demo_an_argument` proves the
argument survives the crossing: the least_authority_demo reports `n*n`, not `n` and not garbage.

## Two hardware details a userspace loader must respect

- **The instruction cache is not coherent with the data cache** (aarch64). init writes a child's
  code as ordinary data; the CPU's instruction fetcher has never heard of those bytes. So when
  `MAP_INTO` maps a page executable, the *kernel* makes it coherent (clean to the point of
  unification, invalidate the I-cache) for that physical page. Without it, the child fetches
  whatever was in the frame before the program was written into it. This is the same
  `sync_icache` the kernel's own loader always did; 19d moved *when* it happens.
- **W^X across two address spaces.** init keeps a code frame writable in its own scratch window
  while the child maps it executable. A trusted loader mapping pages writable to fill them is the
  standard shape (seL4's does the same); the child's mapping is never writable, so the child
  cannot rewrite its own code.

## The initrd is an archive now (milestone 19f.1)

Through 19d/19e the initrd *was* one ELF: the kernel parsed the whole blob as the init program, and
init, to load a child, parsed that same blob again (children were roles of the one binary). 19f
turns the blob into a nifefs archive, the same named-file format the virtio disk uses, so one
parser serves both the RAM archive and the disk. `cargo xtask` packs it (`initial_ramdisk_aarch64`, renamed
from `mkinitrd` 2026-08-27); it holds one
entry today, `progenitor`.

Two readers changed, each in its own domain:

- The kernel (`boot_progenitor`) reads the superblock, looks up the `"progenitor"` entry
  (`"init"` until milestone 266), and loads *that*
  as the ELF. This is the same honest residue as before ("something has to load the first program"),
  now naming that program through a fixed archive index instead of assuming it sits at offset 0. The
  kernel gains a nifefs read, which is proportionate to the ELF parse it already does for init and
  is bounded (a 512-byte superblock, count capped at 15). The milestone tour and the kernel-wired
  demos load a program the same way, through `user::program(name)`.
- **A builder** parses `INITIAL_RAMDISK_VA` as a nifefs archive and looks up its child by name,
  rather than treating the whole blob as one ELF.

Why the archive rides *beside* the kernel (handed in via the device tree) rather than baked into
init's own image: this is the QNX-IFS / old-Linux-initrd lineage, and it keeps the "delivery, not a
rebuild" property. You can swap userspace without rebuilding the kernel. seL4, our nearest relative,
instead embeds a cpio of the app ELFs into its root task; we chose the delivered-archive side so the
RAM boot and the eventual disk boot share one parser.

### The archive carries stripped ELFs (2026-07-30, milestone 23)

**The initrd is reserved RAM**: `memory::init` tells the frame allocator to keep its hands off it,
so every byte in the archive is a byte the running system does not have, for the whole boot. That
was free until it was not.

A debug build is almost entirely debug information. `rust_swappable` is 720 KB, of which 3 KB is
`.text` plus `.rodata` and the other 717 KB is `.debug_*`. Twenty-odd programs like that made a 26
MB archive out of well under a megabyte of code, on a 128 MB machine. Milestone 23
(capability-routed component) added five programs, and the archive went to 30.7 MB. A *later,
unrelated* test then stopped being able to find a contiguous eight-megabyte run for init's building
budget: `no building budget for init`. It came in a test that had nothing to do with the change,
which is the usual signature of a resource the whole suite shares. (That message reads `no building
budget for the progenitor` today; it is quoted here as it read on the day.)

So `initial_ramdisk_aarch64` (`mkinitrd` before 2026-08-27) now strips each ELF
(`llvm-objcopy --strip-debug`) before packing, and the archive is **4.3 MB**. Nothing lost
anything. `crates/elf` parses program headers only and has no section-header code at all, so no
loader on either side of the boundary could ever see a debug section. The kernel prints a raw `pc`
on a fault, and symbolizing it is done offline against the unstripped binary still sitting in
`target/`.

Two deliberate choices. `--strip-debug` rather than `--strip-all`, so the symbol table survives for
anything that later wants to read it out of the archive. And a missing `llvm-objcopy` is a hard
failure rather than a quiet fallback to unstripped bytes. The measured-boot digest (phase B.1 of
DECISIONS §26 (the fault endpoint)) is taken over exactly these bytes. A build that packed different
bytes depending on which tools were installed would be a build whose trust root meant something
different on each machine.

## The programs leave hello (milestones 19f.2 to 19f.6)

Through 19f.1 every program was a role of one multi-role binary, `hello`. Milestones 19f.2 to
19f.5 lifted the system's programs out of it one at a time, each its own ELF in the archive. Each
is loaded by name and started with `x0 = 0`, because a standalone binary needs no role selector.
The four are `least_authority_demo` (`components/src/least_authority_demo.rs`), `console`
(`components/src/console.rs`), `input` (`components/src/input.rs`) and `swish`
(`components/src/swish.rs` then, the directory `components/src/swish/` since). After 19f.5, hello
contains none of the system's programs, and init builds the whole interactive system from those
four binaries. The path this was proven on was then
called `initboot`; milestone 296 (the `initboot`
feature was `shell`) deleted that feature, so read `initboot` in the record as today's
`shell`.

Each split was proved by breaking the old call sites and fixing them, and by a test that still
passes: `a_spawned_least_authority_demo_computes_and_reports`,
`the_least_authority_demo_builder_passes_its_demo_an_argument` and
`the_console_server_builder_brings_up_a_console_that_prints_for_it`. The finished stack was proved by piping keystrokes
into QEMU's serial: `run 9` came back as `9*9 = 81`, from four separate programs plus init.

One wrinkle surfaced and is still a limitation: a line burst-piped before the prompt appears loses
its first character, because the input driver arms its RX interrupt a few instructions after it
starts. A user typing after the prompt never hits it.

Milestone 19f.6 then lifted the runtime the five binaries had copied verbatim
(`invoke`/`send`/`receive`/`exit`) into `crates/user_mode_runtime`, once the split was complete
and the shared surface was known rather than guessed. The panic handler and the device helpers
stayed out of it, each for a stated reason.

Each step, its call sites, its proof and those reasons are in
[`progenitor-and-loading/the-binary-split.md`](progenitor-and-loading/the-binary-split.md).

## Where the claim lives (milestone 295 (retire `components/src/builder.rs`))

**The claim is "userspace, not the kernel, composes a process".** The progenitor makes it at the
scale of a whole system, and the default riscv64 boot reaches it too, per milestone 268 (every
architecture boots the same way) item 4, so there is no separate `builder` making it in miniature.

**Split the claim in two, because only one half moved cleanly.**

*Userspace composes a process.* Carried by the progenitor on every architecture that runs one
(`boot_progenitor`, one function on all three architectures since milestone 166),
on the boot a card performs. On `x86_64` the progenitor composes the system but its console
server and input driver stop on first use, because a ring-3 process cannot reach port I/O (DECISIONS
§121); how a shell gets a console there is DECISIONS §149.
The progenitor composes the console server, the line
discipline, the input driver and `swish`, and a person can then type at the result.

*...from an authority you can count on one hand.* The progenitor does not carry this half: it is granted the NS16550 and the UART's interrupt line as well,
because it is building a system rather than demonstrating a floor.

What carries it is `fixtures/src/process_composition_witness.rs`. It was built for
§185 (what carries the claim that userspace composes a process from an authority you can count on
one hand), as milestone 404 (composing a process from two capabilities is proved for two verbs and
no more). It holds two capabilities, a memory region in slot 0 and a report
line in slot 1, plus the archive mapped read-only. From those it reads `least_authority_demo` out of
the archive by name, mints a rendezvous and builds the child through
`supervision_protocol::build_child_space`. It maps a frame of its own into the child's space and is
refused the same address twice, then configures and starts the thread and receives its answer.
`system_tests::user::tests::a_process_composed_from_two_capabilities_runs_in_the_space_it_built`
asserts every step on all three architectures under `script/test`. That is more than `builder` ever
had: nothing on a pull request executed `builder` (milestone 406 (nothing on a pull request boots
the riscv64 tour, so its userspace step is unasserted),
`design/roadmap/0406-nothing-in-ci-boots-the-riscv-tour.md`).

Until then the fixture was `address_space_witness` and stopped where milestone 19b (run a real
workload) stopped, with nothing running in the space it built, while
`system_tests::user::tests::a_process_can_build_start_and_run_a_child_thread` drove the whole
sequence from the kernel's side. The gap was a join rather than a hole, and the block of milestone
295 (retire `components/src/builder.rs`) carried it as its handoff until 404 closed it.

## What is not here yet

**Resolved since this was written.** The kernel's own pre-init service wiring is gone: §28 retired
`shell_service` as a boot path and milestone 41 (dead code) deleted it and `input_service` outright,
because they had no caller in any configuration. Every interactive build now reaches userspace through `boot_via_progenitor`, so the
kernel has one way in rather than two.

## What `supervision_protocol`'s loader holds to

`crates/supervision_protocol` is the tree's only userspace ELF loader. These are the choices its
doc comments link here.

The scratch window is where a builder maps each frame it fills for a child: 64 MiB, 16,384 pages, at
the bottom of `address_space_map::RUNTIME_WINDOWS` and well under the progenitor's initial ramdisk
window. `std_runtime_contract`'s three pages sit inside it, but only in a `std` program's space, and
no `std` program builds children. A window rather than a cursor that only climbs, because the kernel
takes scratch pages back for the builder. A scratch page maps a frame retyped from the child's
region, and destroying that region revokes every mapping of its pages (§13 (capability revocation
and untyped reclamation)). So `map_scratch` wraps at the window's end and asks the kernel, page by
page, which addresses are free. The size holds every page one builder can have built and not yet
reaped, and its page tables are a budget a builder can state. The progenitor checks both at compile
time.

The kernel's page table is the free list. `PageFrame::MAP` refuses a mapped address whole, with
`BadPointer`, so a refusal means the page is taken and the next is tried. The kernel gives the same
error for a misaligned or kernel-half address, and neither can occur for a page of the window. Any
other refusal returns at once with the cursor unmoved. A page that stays mapped for good costs one
refused probe per lap. `design/roadmap/0604-the-builders-scratch-cursor-is-bounded.md` has the
progenitor's arithmetic.

A child at the interactive prompt gets three times the default stack
(`system_initializer::CHILD_STACK_PAGES`), because the shell's redirection path runs in those
children and its frames grew twice under measurement. The stack is a field so each caller states it.

`CHILDS_OWN_SPACE`, `CHILDS_OWN_PROCESS` and `CHILDS_OWN_THREAD` are sentinel slot values rather
than typed fields, and that is a marked exception. A typed `Option<u64>` field cost the progenitor
352 bytes of stack: debug `boot` keeps about twenty-two endowments in its frame, each 16 bytes
larger, which pushed the install path below `kernel/src/progenitor_stack.rs`'s floor. No capability
slot is that high, so a sentinel cannot name a real slot by accident.

On x86_64 and riscv64 every child gets a timebase page, and the loader writes it from the builder's
own. A child needs
the number, not the kernel's frame, and the builder already has the number mapped read-only. A
builder that does not know the rate writes a zeroed page, and the child then refuses rather than
inherit a number nobody measured (calef, 2026-09-21: only accurate numbers, never hardcoded ones).

A failed build gives back every capability slot it took: the address space on the way out of
`lay_out_child`, the TCB on the way out of `endow_child`, and a frame by whoever retyped it. The
memory is the caller's to give back by destroying the region it built from.
