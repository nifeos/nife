---
status: PROPOSED
raised: 2026-11-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: riscv64 and x86_64 silicon; a core cycle counter a bench boot can read
specific_machine: none
needs_person: yes
---
# The placement curve: what moving one component across the kernel boundary costs, in cycles

Raised by lane proposal/the-placement-curve, at the architect's request. It replaces a heavier idea
that would have built four whole operating systems from one set of components.

Reuse: the kernel bench (`kernel/src/bench/`, `cargo xtask bench`, `script/board-image --bench`).
The cycle counters of milestone 74 (cycle counters) and milestone 309 (unhalted core cycles on
`x86_64`). The repeat discipline of milestone 168 (a multi-tasking workload benchmark). The
manifest readers `capability_manifest_note` and `program_asks`, and the in-memory disk in
`vendor/redoxfs/src/disk/memory.rs`. No new dependency.

## Background

nife runs its services as processes. A file server, a block driver or a network stack each lives in
its own address space, and a client reaches it by a synchronous call through the kernel. §10 (the
capability-based microkernel process model) chose that shape for fault isolation and least
authority, not for speed. Its own table says where the price sits: capabilities as a naming model
cost about nothing, and servers in userspace carry "the entire cost". It adds that the real bite is
cache and TLB pollution, not the trap itself.

The tree has never measured that price on its own code. Risk 4 (the per-crossing cost) ran its
decisive experiment on radon and held, but its entry lists one bound that no instrument here can
lift: "One kernel model, so there is no ratio". Every bench row compares nife with itself or with
Linux, never the same component inside and outside the kernel.

The placement curve is that ratio. Take one component's code unchanged. Run it as a process, as
today, and run it as a kernel-side thread behind the same runtime API. Then place components one at
a time and record cycles per request at each step. The curve shows what each crossing costs once
real code, with a real cache and TLB footprint, sits on either side of it.

A second, separate measurement asks what each component can reach. It is a static census, run on
no machine, and it answers §82 (ambient authority is the problem)'s open line: "Nothing measures
grant width."

## Recommendation

Build the smallest first experiment below, on radon, and let its decision rule say whether the rest
of the curve is worth building. Build the authority census beside it, since it costs no machine
time and its result stands whatever the curve shows. Keep every kernel-side placement behind a bench
feature that no shipped image enables, as `disk_throughput` and `network_bench` already are.

## What is measured, and how

The unit is core cycles per request, read on bare metal. QEMU's `-icount` mode counts guest
instructions, and the committed baselines (`bench/baseline-*.txt`) are in those units. An
instruction count cannot see a cache miss or a TLB refill, which is the cost §10 says dominates. So
an emulator run proves the build works and nothing about the curve.

- radon, riscv64, first. `kernel/src/arch/riscv64/pmu.rs` reads cycles through the SBI PMU, milestone
  168 produced its numbers on this board, and `script/board-image --bench` already builds a bench
  image for it.
- xenon, x86_64, second. `kernel/src/arch/x86_64/pmu.rs` reads `IA32_PERF_FIXED_CTR1`, and xenon's
  boot logs show it found (`bench/xenon-2026-09-17/first-light-095500.log`). Its module `BUGS` still
  says the counter was never read on silicon, and I found no kernel bench run on xenon in cycles. The
  xenon bench boots that exist are staged UEFI sticks (`cargo xtask disk-throughput`,
  `cargo xtask network-bench`); the curve needs the same for the bench suite.
- aarch64 stays out until argon boots. It has run only as a guest.

The kernel-side form has two depths, and the curve names which one each point uses.

1. A kernel thread, IPC kept. The component runs in the kernel's address space and still receives
   its requests through a rendezvous. A runtime crate offers the `user_mode_runtime` API
   (`crates/user_mode_runtime/src/syscall.rs`, `invoke`), with each call entering the kernel's
   capability dispatch as a function call and checking the table as before. This isolates the trap,
   the address-space switch and the cache and TLB effects. The name of that crate is the architect's;
   this proposal mints none.
2. A direct call. The client calls the server's handler with no rendezvous. That needs each
   component's handler split from its serve loop. `redoxfs_server` already has one
   (`Server::handle` in `redoxfs_server/src/dispatch.rs`); I did not check which others do.

The ends of the curve already exist as rows. `ipc_rtt` and `call_reply` in `kernel/src/bench/ipc.rs`
time a round trip between kernel threads, and the EL0 rows in `kernel/src/bench/el0.rs` time one
between processes, with `ipc_rtt_el0_cycles` reading core cycles directly
(`notes/benchmarks/kernel-bench-harness.md`). Their difference is the bare crossing with no work
behind it. The curve asks how much a real component adds to that difference.

## The smallest first experiment, and its decision rule

One component, two placements, on radon, one hart.

- Component: `redoxfs_server`, serving a file system held in memory. A small build of its binary
  replaces the IPC-backed disk (`IpcDisk` in `redoxfs_server/src/bin/redoxfs_server.rs`) with
  `DiskMemory`. That takes the block driver and every device out of the first step, so no DMA, no
  interrupt and no IOMMU difference rides along.
- P0: the server as a process, as today. P1: the same library as a kernel thread, IPC kept.
- Client: `fs_test_client` as a process in both, doing warm reads of one 4096-byte page.
- Repeats: median of 21 per boot, five boots per placement, release userspace, the way milestone 168
  ran.

Call the bare crossing difference D_ipc, from the `ipc_rtt` rows taken in the same boot. Call the
component's difference D_fs, cycles per request at P0 less those at P1. The ratio is D_fs over
D_ipc.

The rule, a provisional maintainer decision written before any number exists, so a result cannot
choose its own threshold:

- Flat. D_fs is under three times the spread across the five boots, or under 2% of P0's cycles per
  request. Record a null result, build no further placements on this machine, and keep the census.
- Crossing only. The ratio is at most 1.5. The cost is the crossing, and the existing IPC rows
  already predict the rest of the curve. Build further placements only for the written result.
- Footprint. The ratio is above 1.5. Real code amplifies the crossing, as §10 predicts. Build the
  curve along the file path and the network path, then repeat it on xenon.

The bounds change only by a new dated decision made before results arrive.

## The second measurement: what each component can reach

A static census over every shipped program, run on the host. It reads each program's manifest note
(`crates/capability_manifest_note`) and its asks (`crates/program_asks`) and prints, per program:

- what the manifest grants: each capability's kind and rights;
- what an ambient-authority policy would endow it with: the union of what its user may reach, under
  POSIX discretionary access control, cited from the standard and not invented here;
- what a kernel-side placement reaches: the whole kernel address space and every object, which is
  the honest answer for any code linked into the privileged image.

The census and the curve stay separate measurements. Placement changes both at once, though, so the
written result sets them side by side: each point on the curve buys its cycles with a known widening
of authority.

## Confounds

1. Instrument blindness. Instruction counts miss cache and TLB cost, so only silicon counts.
2. Per-core counters. `ipc_rtt_el0_cycles` refuses to run with more than one core online, while the
   file system rows `fs_read` and `fs_throughput` (`kernel/src/bench/filesystem.rs`) run only with
   `--real --smp`. The first experiment therefore needs its own single-hart row, which the memory
   disk makes possible.
3. Code placement. Moving radon's kernel text by a few bytes moves the null syscall by up to 8 ticks
   (7%), per risk 4's entry. Compare placements across several builds, not one.
4. Untuned components. A slow component hides boundary cost behind a large denominator. The NVMe
   server keeps one command in flight, and radon's board images carried debug userspace until
   milestone 797 (release userspace in board images). Use release builds and state each
   component's known limits beside its point.
5. Uneven glue. The kernel-side runtime is new code and can be tuned more than the trap path, or
   less. Freeze it before measuring and publish its size.
6. Not a textbook microkernel. Device control planes already live in the kernel (for example
   `kernel/src/e1000e.rs` and `kernel/src/non_volatile_memory_express.rs`). P0 is nife as it is,
   and the written result says so.
7. DMA confinement. Once a device path joins the curve, a kernel-side driver loses its IOMMU or
   virtio validation. That is a third variable on the same axis; report it, do not net it out.
8. Kernel-thread address space. I did not check whether switching to a kernel thread on radon or
   xenon changes the page-table root or the address-space identifier. The answer decides what P1
   saves, so the first lane reads it before measuring.
9. A strawman ambient policy. The census models a policy that capability advocates wrote. Define it
   from POSIX, cite it, and publish the model beside the numbers.

## Prior art

Every item here comes from memory, and none was read for this proposal. Dates and figures are
unverified.

- L4Linux against Linux (Härtig and others, SOSP 1997): Linux run as a server on L4 came within a
  few percent of native Linux on AIM, while MkLinux on Mach did far worse. §10 cites the result as
  the cleanest comparison that exists. It compares two whole kernels, not one component moved.
- Rump kernels (Kantee, NetBSD's "anykernel"): the same NetBSD driver source runs in the kernel, in
  a userspace process or on a hypervisor, through a small hypercall layer. This is the nearest
  match to "same code, different placement". I do not recall a published per-component cost curve
  from that work.
- LibrettOS (around 2020): NetBSD rump drivers run both as a library OS and as multiserver
  processes. I am unsure of its benchmarks and of whether it moved components one at a time.
- Genode: the same components run over several kernels (seL4, NOVA, Fiasco.OC, Linux). It varies
  the kernel under fixed placement, the transpose of this question.
- Minix 3: drivers as user processes, restarted after a crash. Its papers report the overhead of
  that design; I do not remember whether they measure a driver both in and out of the kernel.
- seL4 and Fuchsia: capabilities with servers in userspace. Their published numbers are IPC
  primitives, the anchors of this curve, not the curve itself.
- Capsicum (Watson and others, 2010): capabilities on a monolithic kernel. It is the reference for
  the census's other column, since it confines processes that otherwise hold ambient authority.
  The tree has a reading in `design/capsicum-and-the-retrofit-question.md`.

Where nife differs: its components already take every resource as passed-in state, which §4 (the
kernel shape) rule 2 required from the start, and one choke point carries all authority (`invoke`).
So the move costs a runtime and a window seam, not a port. I know of no published system that
measures the curve one component at a time on the same code; I may be wrong.

A concrete example of the result's shape. Today a warm file read crosses twice: client to file
server, and file server to block server. With the block server kernel-side, the read crosses once
into a process; with both kernel-side, it never leaves the kernel. Three points, the same code at
each, cycles per request on one axis.

## The seven questions

1. Other options. Four whole operating systems lost: one per cell of capability or ambient
   authority, and microkernel or monolithic. Two of those cells need ambient authority, which §10
   forbids on main. And §10 predicts the cost of capabilities against ambient authority is about
   zero, so that axis would likely show only noise. Comparing nife with Linux alone lost too: it
   varies every component at once.
2. The tree's analogous case. Milestone 8 (the console driver leaves the kernel) moved a kernel-thread
   server out to a process (`notes/capabilities/ipc-endpoints.md`), and `disk_throughput` already
   runs its client in the kernel's boot thread against a process server. Neither timed both
   placements of one server.
3. Prior art. Above, all from memory.
4. The premise. §10 states the cost lives in the boundary and in cache pollution, and also that "We
   will never measure it". This proposal measures it and does not edit §10. A result that
   contradicts its table goes to the architect as its own proposed amendment.
5. Cost. The effort guesses below; the machine time is one radon evening with a person for the
   first experiment.
6. Reversibility. The code is a bench feature and can go. The published number cannot: see the doors.
7. Same cost both ways. Yes. The census and the single-component experiment are also the cheaper
   options, but they win on what they isolate, not on effort.

## Doors

- Two-way: the kernel-side runtime, the bench feature, the memory-disk build and the census tool.
  The maintainer decides these and records them as provisional.
- Two-way, and the architect's by name: the name of every crate, program and bench row this mints.
- One-way: a published cycle figure or a paper. A number a stranger quotes cannot come back, so its
  text goes to the architect before any submission.

## Effort, guessed

All three figures are guesses with no measurement behind them.

- The first experiment: 5 to 8 lane-days, plus one radon evening with a person at the board.
- The census: 2 to 4 lane-days.
- The full curve along the file and network paths, on two machines: 15 to 25 lane-days.

## What it depends on

- Components must read their window addresses from the `Handoff` and not from constants. Today
  they use fixed addresses, such as `DATA_PLANE_VA` in `components/src/non_volatile_memory_express.rs`
  and `BLK_PAGE` in `redoxfs_server/src/bin/redoxfs_server.rs`. In one shared address space those
  collide. Another lane is making that change, and this work waits for it.
- Another lane is listing what should leave the kernel to restore the microkernel. Its list decides
  which control planes the curve can place. This proposal does not repeat that list.

## The written result

One figure per machine: cycles per request on the vertical axis, components placed kernel-side on
the horizontal, with the bare-crossing anchors drawn in. Beside it, one table per program: granted,
ambient, kernel-side. The claim it can support is narrow and checkable: on this silicon, with this
code, moving one component across the boundary costs this much, and buys this much authority. That
is a workshop-length paper, and its text and numbers go to the architect before it leaves the tree.

## Not in scope

- An ambient-authority operating system. §10 forbids ambient authority on main, so the census
  models the ambient policy and builds none of it.
- A whole monolithic nife. The curve places components one at a time and stops where its rule says.
- Changes to the syscall surface, `design/decisions/` or the shipped image.
- The list of what leaves the kernel, and the window seam: both belong to other lanes.
- aarch64, until argon boots.

## What is not verified

- Nothing was built or run.
- Whether a kernel-thread switch changes the page-table root on radon or xenon.
- Which components besides `redoxfs_server` separate their handler from their loop.
- That `redoxfs_server` builds over `DiskMemory` without further change.
- Every prior-art claim.

## Index row

Nothing in the tree compares one component inside and outside the kernel. This measures, in cycles
on silicon, what placing each component kernel-side costs, beside a census of what each can reach.
