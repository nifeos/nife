# HongMeng, and what its numbers say about strict isolation

*Name: provisional, minted 2026-11-23 (UTC) by lane/hongmeng-prior-art.*

HongMeng is Huawei's production microkernel. Its OSDI 2024 paper is Chen et al., *Microkernel Goes
General: Performance and Compatibility in the HongMeng Production Microkernel*, OSDI '24, pages
465 to 485. This note read the whole paper from usenix.org on 2026-11-23 (UTC). Every page number
below is a proceedings page, and every section number is the paper's own.

HongMeng matters to nife for one reason above the rest. It began with every service isolated in its
own address space (Section 4.3, p472), then measured what that cost on phones, and relaxed it where the cost
was too high. nife holds the strict end of that range today. HongMeng's figures are the best
published evidence of what the strict end costs under a general-purpose load, because nife runs no
such load and seL4's published figures cover IPC alone.

HongMeng is closed source. The section on what cannot be checked says what follows from that.

## What the paper says HongMeng is

The core kernel holds the scheduler, the serial and timer drivers and access control, and nothing
else (Section 1, p466; Section 3.2, p469). It is "implemented primarily in a confined subset of C", 90,000 lines,
and the services that run on it total over a million lines (Section 6.1, p476). The paper does not say what
language the services use. It reports semi-formal verification of some components, and ASIL-D and
CC EAL 6+ certifications (Section 1, p466). It runs on "tens of millions of devices": routers, cars and
phones (abstract, p465; Section 6.1, p476). One code base serves all three, and a build-time configuration
places each service (Section 6.1, p476).

## Where the cost of isolation showed up

Section 3.1 (p467 to 468) measures the load before it designs anything. Four of its findings set
the terms for everything after.

1. IPC frequency depends on the device: 0.6k calls a second on routers, 7k in cars, 41k on phones.
   Phones also make 61k system calls a second.
2. Phones take 27k anonymous page faults and 5k file-mapped faults a second.
3. A file system and a memory manager that each keep their own page-cache state cost 2 times
   Linux's latency on a cache miss, 16% of app startup and an extra 50 MB of memory.
4. Checking a capability on every anonymous fault makes the fault 3.4 times slower than Linux.

A phone or car also needs over 700 drivers, against fewer than 20 for a router. The paper estimates
a rewrite at over 5,000 person-years (Section 3.1, p468).

## Point by point

### Capabilities, and address tokens beside them

HongMeng keeps capabilities for access control (Section 3.2, p469). For kernel objects that a service
co-manages with the kernel, it adds address tokens (Section 4.4, p472 to 474). Each such object sits on its
own physical page, and mapping that page into a service is the grant. A read-only mapping lets the
service read the object directly. A write to a read-only object goes through a new `writev` system
call that the kernel checks. The kernel reads what services write only through trylocks, and
sanity-checks it. Static analysis keeps read-write objects free of pointers (p473).

Figure 6 (p473) gives the latency in cycles. A token read costs 6 and a token write 10. The kernel
path for a write to a read-only object costs 815. HongMeng's own capability path costs 866, seL4's
526 and Fiasco.OC's 820.

The paper names two losses (Section 4.4, p473). A token cannot restrict operations once granted, and tokens
support no chain revocation. It argues that centralized resource management gives each object one
owner, so chain revocation is rarely needed.

nife names everything through a per-process capability table in kernel memory, with no ambient
authority (§10 (process model: capability-based, microkernel),
`design/decisions/0010-capability-microkernel.md`). Revocation is coarse by choice:
reclaiming a memory region kills every object in it, and generational names make every outstanding
capability stale (§16 (object revocation), `design/decisions/0016-object-revocation.md`). nife has no derivation tree
either. HongMeng reached the same coarse answer for its tokens from the other direction, and its
deployment is some evidence that the coarse answer holds up.

nife already has the read half of a token. The kernel writes pages that a program maps read-only:
the machine statistics page (`notes/machine-statistics.md`) and the current-CPU page
(`notes/current-cpu-page.md`). Each one is a contract crate with a fixed layout, granted by
capability. nife has no read-write token and no kernel path that
reads state a service wrote.

### Isolation classes and coalescing

Section 4.2 (p471) defines three isolation classes.

- IC0 has no isolation, so a call is an indirect function call. The Linux ABI shim is the only IC0
  service in deployment.
- IC1 runs services in kernel space, each in a hardware domain: ARM watchpoints give 4 domains and
  Intel PKS 16. Binary scanning, pointer authentication as control-flow integrity, and a secure
  monitor guard the boundary. The paper concedes IC1 adds attack surface.
- IC2 is an ordinary address space per service.

Figure 4 (p471) measures a round trip on a Raspberry Pi 4b, in cycles: IC0 to IC0 18, IC1 to IC0
272, IC1 to IC1 752, IC2 to IC2 1439, seL4 1376, Fiasco 2883. IC1 halves the IC2 cost. IC2 runs
slightly slower than seL4, which the paper puts down to fine-grained locking.

Section 4.3 (p471 to 472) goes further. On phones only, the file system and the memory manager run
coalesced into one failure domain, with one shared page cache. Table 3 (p472, Kirin9000 big core)
gives a page fault at 7092 cycles separated, 5290 coalesced with separate caches, 3785 with a shared
cache, and 3432 on Linux. Memory drops from 190 MB to 120 MB. The two versions exist side by side,
written by hand. Two rules bound the trade: IC1 holds no third-party code, and IC0 holds only the
shim (p472). Isolation still costs 20% of file-mapped fault handling inside IC1 (p471).

Cars and the trusted execution environment run every service in IC2. Phones put the process
manager, memory manager, file system and native driver container in IC1, and coalesce the file
system with the memory manager. The Linux driver container and the power and frequency governors
stay in IC2 (Section 6.1, p476 to 477).

nife has one class, IC2. Every service is a user process (§10; `kernel/src/user/*_service.rs`
spawns them). Its kernel is Rust, with 82% of `kernel/src` outside `arch/`
(`notes/l4-lessons.md`, row 16). §323 (the binary interface, and the names of the layers) says that where a service runs is a kernel's own choice, which
no program can observe (`design/decisions/0323-the-binary-interface-and-the-names-of-the-layers.md`).
A coalesced build is therefore a kernel choice under §323, not a change to the binary interface.

### IPC

HongMeng's IPC is synchronous RPC by thread migration (Section 4.1, p470 to 471). The kernel switches the
caller straight into the service, changing the stack pointer, the instruction pointer and the
protection domain, and skipping the scheduler. A service registers a handler and a pool of execution
stacks. Hot services, the process manager, memory manager and file system, get stacks bound in
advance. An adaptive pool backs the rest, and a call that returns to a service already on the chain
reuses its stack. A reserved pool lets an out-of-memory path still reach the memory manager. The
root caller pays the energy and memory of the whole chain. Arguments go in registers, and more in
shared memory. The paper measures privilege and address-space switching, with the cache and TLB
pollution they cause, at half of the IPC cost (p470).

nife's IPC is a synchronous rendezvous on an endpoint. A message is three words in registers and a
five-word mailbox in the TCB, and bulk data goes through shared frames (`notes/l4-lessons.md`, rows
3 to 5; `notes/inter-process-communication.md`). nife does not switch directly. `ipc_send` fills the
receiver's mailbox, wakes it onto the run queue and returns, and the sender keeps running
(`kernel/src/sched/rendezvous.rs`; `notes/l4-lessons.md`, row 14). Every thread owns 6 pages of
kernel stack plus a guard (`kernel/src/thread.rs`, `STACK_PAGES`). Milestone 188 built three
cheaper cuts and refused a hand-written fastpath (`design/roadmap/0188-ipc-fastpath.md`). §95 (a hand-written IPC fastpath, and whether it can stay proven) keeps
the fastpath gated on a measurement (`design/decisions/0095-a-proven-ipc-fastpath.md`).

nife's measured round trip is 442 ns against seL4's 96 ns on one core of the same HVF guest, about
4.6 times (`notes/benchmarks.md`). HongMeng's IC2 sits within 5% of seL4. The gap between nife and
seL4 is IPC mechanism, not isolation, and HongMeng's design names the mechanisms that close it.

### Kernel paging

HongMeng replaces distributed user-level pagers with one memory manager (Section 4.5, p474). The memory
manager pre-allocates PCache pages into a process's address-space range. On a fault the kernel maps
from them directly and appends an OPLog entry that the memory manager reads later. Without a
pre-allocated page it falls back to IPC. Figure 8 (p474) gives the result: fault latency falls by 72%
(read) and 33% (write) on the Pi 4b, and 75% and 55% on the Kirin. A pager round trip costs about 140
ns on the Pi 4b. The paper leaves seL4 out, since seL4 has no demand paging by default.

nife has no demand paging. A user fault kills the thread, or goes to its supervisor
(`kernel/src/arch/aarch64/exceptions.rs`, `user_fault`). A process maps pages it has retyped from a
memory region it holds, and the kernel allocates nothing (`notes/memory-regions.md`). HongMeng's figures
say a fault handled in the kernel from pre-supplied pages costs little more than Linux's. They do
not say what a capability check on that path costs nife, because nife has none.

### The Linux driver container, and twin drivers

HongMeng runs Linux drivers unmodified inside a Linux Driver Container (Section 5.2, p475 to 476). The
container runs mainline Linux as a user-space runtime in IC2, in the line of LKL, UML and SawMill.
A small base layer supplies a virtual timer, a virtual interrupt controller and a linear map. Over
700 drivers run this way. Drivers that issue `smc` need binary rewriting, and the interrupt
controller driver needed porting. The container supports Linux 4.4, 4.19 and 5.10, and the move from
4.19 to 5.10 took under 100 changes. An SMMU confines DMA (p476). The paper gives avoiding license
contamination as one reason to keep Linux drivers isolated (p475).

For speed, a twin driver in IC1 takes the I/O data plane, while the container keeps the control
plane. UFS storage is the one example. Figure 10 (p476) shows twins 140% faster than the container
alone at 4K.

nife writes its drivers in Rust as crates (`designware_ethernet`, `designware_mobile_storage`,
`xhci`, `e1000e`, `non_volatile_memory_express`, under the nifeos organization) and runs them as
user processes in `components/src/`. An IOMMU confines their DMA (`notes/iommu.md`). nife also
splits control from data. The NVMe admin process holds the controller registers, while the data plane
holds only the rings, one transfer window per client and the doorbell page
(`components/src/non_volatile_memory_express_data_plane.rs`). The JH7110 Ethernet driver splits the
same way (`components/src/designware_ethernet_control.rs` and `designware_ethernet_transport.rs`).
The split runs the other way from HongMeng's. HongMeng gives the data plane more privilege to make it
fast. nife gives it less, to confine it.

### Linux ABI compatibility

The shim in IC0 turns each Linux system call into IPC by number, and native calls bypass it (Section 5.1,
p474 to 475). It holds the central file-descriptor table, and a poll list shared with services
through tokens, so `fork` and `poll` work. The file system service handles `ioctl` and `fcntl`.
HongMeng passes the Android CTS and VTS suites. One failure came from programs that depended on
`epoll`'s ordering, which the paper files under Hyrum's Law (p475). Where applications are custom,
POSIX libraries replace the shim, so the shim is optional (p474).

Table 5 (p477) shows the cost against an optimized Linux 5.10 on Kirin9000. `fork` takes 1280 µs
against 323, `exec` 1305 against 336, and thread creation 54.3 against 28.6. Context switches run
32% faster than Linux, and networking 21% faster on average. In real workloads, parallel copying of
memory areas cut `fork` from 150 ms to 60 ms, against about 30 ms on Linux (Section 6.2, p477).

nife has no Linux ABI. §10 rejects `fork`, paths and uids and makes a POSIX layer additive, in user
space. Its C library starts from relibc (`notes/c-library.md`). §323 calls a user-space Linux
server over a capability kernel a personality, a component of the library, and says it changes no
kernel's binary interface. It also notes that implementing Linux's binary interface does not bring
in the GPL, while taking Linux code would.

## What nife could take

Three things, in order of use.

1. The cost model. Section 3.1, Figure 4, Table 3 and Table 5 together are the best available
   measurement of strict isolation under a general-purpose load. They say where the cost sits: IPC
   frequency, page faults across the file system and the memory manager, and state kept twice. A
   phone-class load is not nife's load, but the order of magnitude is the honest prior. Any future
   nife claim about what its isolation costs should start from these figures.
2. Direct switch with stacks bound in advance, for hot services. HongMeng's IC2 round trip sits
   near seL4's, and nife's sits at 4.6 times seL4's. The two missing L4 rows in
   `notes/l4-lessons.md`, direct switch and the event kernel, are the two HongMeng's IPC answers.
   Its stack pool and reserved reclaim pool also answer how migration copes when memory runs out.
3. Kernel-handled faults from pages a manager supplied. PCache fits a kernel that allocates nothing,
   because the pages belong to the manager. A single memory-manager service that also owns the page
   cache avoids the double bookkeeping HongMeng measured at 50 MB. nife needs demand paging before
   any of this applies.

Three smaller items follow. Read-only kernel pages could become a general grant shape, and nife
has two already. A call chain could charge its root caller. And Section 7 (p478) finds that a big
kernel lock stops scaling near 61k system calls a second.

## What nife should not take

- IC0 and IC1. Both move services into kernel space. IC1 rests on watchpoint and PKS domains, binary
  scanning and pointer authentication, which nife's Kani proofs do not cover. It would put service
  code inside nife's trusted base, and §10's first two gains would no longer hold for that code.
- Coalescing with a shared page cache. It merges two failure domains, and HongMeng keeps two
  versions of each service by hand to offer it. If nife ever wants it, §323 allows it as a separate
  kernel's choice. The default kernel should not carry it.
- The Linux Driver Container. Linux drivers are C under the GPL, and they fail the §46 (thin primitives or whole subsystems) bar on its
  first condition, a safety argument at every `unsafe` use (`design/decisions/0046-dependency-rule.md`).
  §323 records that taking Linux code would bring in the GPL. The container answers HongMeng's
  5,000-person-year problem, and nife does not have that problem: it targets a few boards whose
  drivers it writes.
- Read-write tokens without fine-grained rights. HongMeng accepts that a granted token cannot be
  narrowed. nife's least-authority claim rests on narrowing, so a read-write object needs a right
  bit per field group, or it stays a method call.
- The Linux shim inside the kernel. §323 already places a Linux personality in user space. HongMeng's
  `fork` at 4 times Linux, with coalescing on, is evidence for §10's refusal of `fork`.

## What cannot be checked

HongMeng is closed source, and the paper ships no artifact. So every number above is the authors'
own, on hardware nife does not hold, against a Linux build that the paper calls optimized without
publishing it (Section 6.2, p477). The source of the following is out of reach:

- the 90,000-line count and the language of the services;
- what the semi-formal verification covered, and against which model;
- whether IC1's scanning and control-flow integrity hold up, which the paper asserts without a
  security evaluation;
- the static analysis that keeps read-write tokens free of pointers;
- how many of the 700 drivers run unmodified, as opposed to the count that needed rewriting;
- the deployment count, which no third party reports.

The Figure 4 seL4 row ran on a Pi 4b and gives no seL4 version or configuration. nife cannot repeat
it without that. The figures that are checkable in kind, such as the IPC round trip against seL4,
nife can measure on its own boards with sel4bench (`notes/benchmarks.md`).

## Open questions

- Should nife measure its own fault and file-mapping path under a load like Section 3.1's before
  it claims a cost for its isolation?
- Does the direct-switch row in `notes/l4-lessons.md` now have enough evidence behind it to become a
  milestone, with stacks bound in advance as HongMeng does them?
- Would a single memory-manager service that owns the page cache fit §16's region ownership, or
  does it need a new authority?
- Is a read-only kernel page, with a contract crate, a general grant shape worth stating once, rather
  than one decision per page?
