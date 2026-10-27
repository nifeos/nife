# What a capability can name

*Name: provisional, minted 2026-10-23 (UTC) by comments/929-kernel-capabilities.*

`kernel/src/cap.rs` is the kernel's half of the capability model: the set of nouns a process can
be handed, the capability table's size, and the checks that keep the kernel's rights vocabulary and
the ABI's in step. The table itself is `crates/capability`, which is pure logic. The model is §10
(process model: capability-based, microkernel), and [capabilities](capabilities.md) is the
introduction. This note carries the design the file's doc comments link to.

## The nouns

Each variant of `Object` is a decision. What follows is why each is there.

- `Rendezvous`: an IPC endpoint, by generational name in the scheduler's registry, stale-safe like
  a `ThreadId`. `WRITE` lets the holder `SEND` and `READ` lets it `RECEIVE`, so handing out the two
  ends with opposite rights makes a one-way pipe. The console is a userspace server reached this
  way, so the kernel knows nothing about a UART on any path a user program can take.
- The endpoint's badge rides on the capability, not on the endpoint. The kernel delivers it to a
  server's `RECEIVE_CAP`, so a server can tell apart clients sharing one endpoint. `0` means
  unbadged, which every capability is until `abi::rendezvous::BADGE` stamps one. This is seL4's
  model: a badge is set once on an unbadged capability and never altered, and two clients can hold
  differently badged views of one rendezvous. §148 (a supervisor restarts by asking) refused `BADGE`
  as a thread-identity method so it stays free for this. The field fits without growing `Object`,
  because `PageFrame` is already that wide.
- `MemoryRegion`: raw physical memory the holder retypes into pages and objects. Invoking it grows
  the holder's address space out of its own budget, and the kernel allocates nothing to do it.
- `Irq`: a hardware interrupt, by INTID. The holder can `WAIT` for it and `ACK` it once the device
  is serviced. The kernel's handler does nothing device-specific: it masks the line and turns the
  interrupt into a message. A driver owns an interrupt without owning any privilege.
- `PageFrame`: a contiguous run of physical pages, by base address and count. IPC carries control
  and shared memory carries data. A process retypes a frame out of its own region, maps it, and
  hands it (or a read-only view, since delegation narrows) to a peer, which maps the same pages.
  The address is the identity, and nobody can forge one, because the only ways to get a `PageFrame`
  are to retype it or be handed it. See [frames](frames.md).
- `DeviceFrame`: one device's MMIO page, mapped device-typed (nGnRnE on aarch64: uncacheable and
  unreordered). It is distinct from `PageFrame` because normal cacheable memory would let the CPU
  cache and reorder register accesses. The kernel alone knows device addresses, so it mints one per
  known device and hands it to the progenitor, which delegates it to the driver it builds.
- `Reply`: a one-shot reply channel to a blocked caller, minted at a `CALL` rendezvous and named by
  the caller's thread id. It is nameable no other way, so "one reply, to this caller, exactly once"
  is a kernel guarantee rather than a server convention. See [IPC naming](ipc-naming.md).
- `AddressSpace`, `ThreadControlBlock`, `Notification`, `Timer` and `Process` are objects a process
  builds from its own region, each by generational name, so a capability that outlives its object
  resolves to nothing rather than to a stranger.
- `Virtio`: a virtio device's transport. The device has no IOMMU, so the kernel keeps the two
  DMA-critical powers, programming the queue's ring addresses and ringing the device, and checks
  every descriptor against the driver's own DMA region before the device sees it.
- `PortRange`: x86 I/O ports, by base and count. Port I/O has no page in front of it, so the MMU
  cannot grant or deny it. The TSS I/O permission bitmap enforces it instead: a holder may execute
  `in` and `out` on its range from ring 3, and a thread that holds none may touch no port. The
  capability must carry `WRITE` (calef, 2026-10-05 UTC; milestone 768 (a read-only port range
  grants nothing)), because the bitmap cannot grant `in` without `out`. The range granularity fits a
  16550 UART, eight consecutive ports. It exists only on `x86_64`, because the other architectures
  have no port space. That is not a parity gap, and compiling it only there keeps it off the
  other dispatchers that `script/fastpath-footprint` measures. `(u16, u16)` fits the existing payload.
- `Reboot`: the authority to restart the machine. It names no device, because PSCI, SBI SRST and a
  PC's reset register are all "the machine". The kernel mints exactly one, at boot, into the
  progenitor's table; no retype makes one.

A `PageFrame` count is `NonZeroU64`. Every dispatch path derives an end address from the count, and
`page_frame_map`'s tail check computes `(count - 1) * PAGE_SIZE`, so a zero would underflow: a
debug build panics and a release build wraps. The type removes the branch at no cost, since the
niche keeps `Option<Object>` the same size. `page_frame_run_len` is `const`, so a caller passing a
literal gets the refusal at compile time. The count rides on the capability, not on `MAP`'s
arguments, so the syscall's argument shape is the same for a run as for one page. §102 (a Frame names a run of pages) is the ruling.

## What a slot costs

A slot is 32 bytes: `Object` is as wide as its widest variant, `PageFrame`, at 24 bytes, plus the
rights. §102 priced the slot count on 24 bytes a slot. At 64 slots a table is 2,048 bytes of slots,
and the option §102 priced at 12 KiB a thread for 512 slots would be 32 KiB. `sched::MAX_THREADS`
is 256, so every whole-machine figure §102 quoted is larger again. The refusal does not change; the
size assertions in `cap.rs` exist so the number a reader quotes is the one the compiler agrees
with.

The table is 64 slots, calef's ruling of 2026-10-04 (UTC) for milestone 754 (the capability table
grows to 64 slots), chosen over moving holdings into a new process after a gpu-and-keyboard boot
reached 31 of 32. The table lives in its thread's TCB page, cost 1,032 bytes a thread more than 32 slots, and no
static memory; the page-fit
assertion in `crate::thread` is the check. The free-slot word is a `u64`, so 64 is also the ceiling
the type allows; going further needs a wider word or a second one. Sizing a table per process is
milestone 778 (capability tables sized per process).

A full table fails silently. A progenitor that fills its table dies in `user_mode_runtime::trap`
before a console exists to carry a word, and `script/test` never boots the real progenitor, so
`script/swish-check` is the gate that does. That is why the size is a measured peak plus headroom.

## The peak, recorded

`CAPABILITY_TABLE_PEAK_MEASURED` is 37, measured on 2026-10-19 (UTC) on `script/swish-check`'s
aarch64 leg, on the boot that runs the full script for milestone 892 (a std program at the prompt
holds the network and a name resolver). The spawn service is then building `jig install` and holds
the copy of the boot terminal it places as that job's standard input. The same boot without that copy peaks at 36.
A boot with no NVMe disk peaks two lower, because the disk tree's endpoint and client-window pool
are held for the life of the boot. [The peak trace](capability-peak-trace.md) itemizes what sits on
the peak.

The record is checked rather than the ceiling, which would only fire on a boot that had already
died. When the console line fires, measure, update the record, and re-read the headroom before
raising the slot count. Every slot held for the life of the boot is one slot on the peak.

`report_peak` runs in the scheduler's idle loop, the one place reached after every phase of a boot
and by definition not busy. It waits for the mark to stay still for 16 idle passes. With a window of
one, an aarch64 interactive boot printed six lines on its way up, because the progenitor blocks on
IPC several times while it builds the login stack and each pause looked like an ending. At 16 it
prints one. The window is a coalescing window, not a margin on the measurement: getting it wrong
costs an extra line or a later one, and it cannot make the peak wrong, because the peak never
decreases.

The test kernel counts and does not print. The guest suite's transcript is a gate's input, and a
third writer in the middle of it would split a test name. The interactive boot's record would also
be the wrong yardstick for the suite's much larger workload.

## Two rights vocabularies, one meaning

`abi::rights` is the word that travels in a syscall register, from the progenitor's grant tables to
every `SEND_CAP` and `CAP_INSERT` in `components/` and `fixtures/`. `capability::Rights` is what the
kernel means by one. They are two arrays of numbers in two dependency-free crates that cannot see
each other, so `cap.rs` compares them at compile time.

The failure this closes is a right added to one crate and not the other. A bit in `abi::rights`
missing from `Rights::ALL` is a right userspace can name that `Rights::from_bits` masks to zero, so a
delegation asking for it succeeds and confers nothing. A bit in `ALL` with no ABI name is a right the
kernel honors that no caller can ask for, which is how a hand-listed set drifts into granting more
than any manifest says. The syscall path narrows with `a1 as u32`, so a right at bit 32 or above
would be truncated to nothing on the way in; an assertion stops anyone defining one.

## What a budget's rights mean

A retype mints the budget's rights (calef's ruling (a), 2026-10-07 UTC, PR #1798; milestone 824 (a
retype mints no right its budget lacks)). What a budget carries is therefore what everything carved
from it carries.

A leaf's budget is every right but `GRANT`. With `WRITE` alone a leaf could not wait on its own
notification, receive on its own rendezvous or join its own process. On the region itself `READ` and
`BIND` are consulted by no method, and `ENUMERATE` only by `MemoryRegion::USAGE`, which answers what
the holder's own budget was spent on, so the extra rights confer nothing over the region. `GRANT` is
the one left out: a leaf cannot lend its budget or anything it carves from it. A builder that must
lend what it makes gets a budget with `GRANT`.

The root budget the progenitor receives carries `Rights::ALL`, so it is where every right on every
object comes from: the progenitor's processes need `BIND` to hand a `std` program its own process.
`ALL` rather than a list, because a list stops being full the next time a right is added. Rights
only narrow below the root: a `SPLIT` child inherits its parent's, through the `mint_child` the
`split_never_widens_rights` proof covers, and `CAP_INSERT` narrows again. `ENUMERATE` flows down
every split, so the progenitor can hand a job budget's holder a view narrowed to `ENUMERATE` alone,
which is `free`'s "yours" line. A holder that could already spend, split and destroy the region
learns nothing new by asking what it spent, the argument §114 (`pmap` gets its listing) made for
the address-space object.

## BUGS

- `report_peak` reports the highest any table reached, not whose table. Finding the owner means
  walking every thread under the scheduler lock, which the idle loop does not do.
