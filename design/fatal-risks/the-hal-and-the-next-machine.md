---
risk: 9
color: green
updated: 2026-09-23
---
# Appendix to risk 9: The HAL is a fiction, and an architecture costs a restructure rather than a port

*An appendix to [`9-the-hal-is-a-fiction.md`](9-the-hal-is-a-fiction.md) (risk 9). That entry is the claim of
record, and it is written so that a reader can decide what to work on next without opening this
file. This one exists to be verified or challenged: it holds the evidence, the dates, the numbers,
the corrections and the refusals behind the verdict, at the length they need rather than the length
the six-pager has. Where a study has its own home in `notes/` this page links it rather than copying
it. Name provisional (`design/fatal-risks/` and this file's stem); naming is an architect's.*

### The claim, calef's, 2026-08-30

*"another proof/disproof of the nife thesis is actual functioning on the three silicons. If we can't
get it to run on one, that would also likely kill the effort."*

Sharpened, because the ISA count is not the fatal part. An OS that runs on two of three
architectures is still an OS. What would be fatal is what a failure would reveal. That adding an
architecture requires changing the kernel rather than adding a directory under `arch/`, which is
exactly what DECISIONS §4 (kernel shape, with two cheap rules)'s rule 1 and §19 (architectural
parity is a tenet) claim it does not.

Widened 2026-09-23 from architectures to machines, and the ruling is calef's. He raised it about
rented computers: *"A nife that runs on one cloud platform but not another is also its own form of
risk."* Asked whether that was a tenth entry or a wider ninth, he widened this one. So the claim
above now reads on two grains rather than one. An architecture is aarch64, riscv64, x86_64. An
implementation is a particular machine of one of them: xenon, some provider's metal, a hypervisor's
idea of a PC. (Both words are this entry's, provisional, and neither is ratified.) Two x86_64
machines from different vendors are the same silicon with different firmware, a different way of
delivering the boot, and different tables handed to the kernel at entry. A nife that boots on xenon
and not on a rented server falls through the old wording entirely. And it would fail for exactly the
reason this entry names: the abstraction was not as real as the port made it look.

Three reasons that belongs here rather than in an entry of its own, which is what the ruling turned
on.

### It is the same claim, asked twice

Architecture and implementation are two grains of one question: is the seam real, or does each new
machine cost a restructure? Splitting them into two entries would put one question's two halves on
two lists and let each look answered by the other.

The implementation grain is the earlier warning. A HAL that is a fiction shows up first as "boots on
our board, not on theirs", long before it shows up as an architecture costing a restructure. By the
time an ISA is expensive, the cheap evidence has already been available for months and nobody was
reading it.

And it is the grain that can actually be bought. A Jetson TX1's silicon twin is not for sale, and a
second VisionFive 2 answers nothing a first one did not. Rented x86_64 and aarch64 metal is
available by the hour, from several vendors with different firmware. So the widened claim has an
experiment the narrow one does not, which is the second property the rule at the top of this file
demands.

The experiment for the widened grain, which has not been run: a second machine of an architecture
nife already boots on, riding on milestone 225 (run the soak on radon, argon and xenon). It is not
specified further here, because a lane is pricing rented metal right now against that milestone and
against risk 4, the per-crossing cost, and both want the same rental. So this question costs a boot
rather than a purchase, and what the pricing will conclude is not known yet. What a result would
mean is worth fixing in advance, because all three outcomes are informative and only one of them
looks like news. If a second x86_64 machine needs a change outside `arch/x86_64/`, this risk moves
toward red at a grain the 2026-09-17 run never touched. If it needs a change inside `arch/x86_64/`
that xenon did not, that is the `AlreadyMapped` shape again, a machine-specific fix behind the seam,
and it is the cost this entry calls healthy rather than fatal. And finding no difference at all is a
result, recorded rather than shrugged at: it would be the first evidence anyone here has that the
port is a port and not one machine's configuration.

The status is asymmetric. riscv64 already disproves the strong form: the VisionFive 2 booted the
full tour on three harts on 2026-08-14, the strongest evidence in the tree that the HAL is real.
aarch64 is the development ISA and its board (the Jetson TX1, milestone 127 (the seL4 machine, so
identical silicon referees the comparison)) is well documented. x86_64 is where the risk lives,
because it is newest, not because x86 is hard. Milestone 161 (the x86_64 kernel port: bring up the
HAL's third architecture) was unfinished when this was written and is `BUILT` since 2026-09-19
(below). Milestone 177 (wire the graphical terminal stack into the real interactive boot)'s text
says x86_64 has no real interactive boot entry point at all. And 166 and 167 are each a piece of the
same unfinished edge.

Two of those closed, 2026-09-01 and 2026-09-02, and this entry did not notice for ten days. It used
to cite milestone 164 (x86_64 userspace can't build `aes`: no SSE, no scalar fallback) as the reason
x86_64 has no `fs_server`. That milestone is `BUILT`: the whole of it turned out to be one build
flag (`--cfg aes_force_soft`, which selects `aes`'s portable software backend), and x86_64 userspace
now builds `aes`, `redoxfs_server` and `mkfs`, with the last two in the x86_64 archive.
Milestone 165 (x86_64 PCI enumeration) is `BUILT` too. The risk is not weakened by that so much as re-sited:
what is left on this edge is the boot entry point and the orchestrator, not the toolchain, which is
a shorter list and a different kind of work.

The third claim above is a premise rather than a status, so no gate will ever catch it. This entry
still says milestone 177's text has x86_64 with no real interactive boot entry point at all, and
reads that as meaning the graphical stack is the only route to a shell. That does not follow.
`swish` never talks to a UART on any architecture. It talks to a console server over an endpoint.
And what actually blocks x86_64 is that §121 (what a device capability is when the device has no
page) leaves no userspace holder for that endpoint. Whether a kernel thread may answer there instead
is `design/decisions/0149-kernel-served-console-endpoint.md`, `PROPOSED` since 2026-09-09. If 
§149 (may the kernel answer on an endpoint) is decided yes, 177 stops being a prerequisite and milestone
182 reaches a shell over serial, which is also what a bench session needs. If it is decided no, the
sentence above stands as written. Either way this risk's decisive experiment below is unaffected,
because milestone 87 is about the boot entry and not about the shell.

### The verdict of record

RUN, 2026-09-17. GREEN, and this is the verdict this entry was missing. The sharpened claim is
falsified: adding x86_64 did not require changing the kernel outside a new `arch/` directory. What
follows is the evidence already gathered here, read together for the first time rather than left as
a narrative with no stated conclusion.

The decisive experiment was milestone 87 (the x86_64 bare-metal machine), and it RAN on 2026-09-17.
The result is green.

```
nife machine: x86_64, 4 processor(s), 17119 MiB, 100 Hz nife self-test: 5 of 5 passed
```

nife runs on all three declared architectures on real hardware. *(Corrected 2026-10-06: on
bare metal, two of three; aarch64 has run only under HVF on patagonia, which is virtualization.)*
Transcript
`bench/xenon-2026-09-17/first-light-095500.log`; milestone 87's block has the detail.

What the experiment was actually testing, and why this answers it. This entry's own sharpening says
the ISA count is not the fatal part: what would be fatal is a failure revealing that adding an
architecture requires changing the kernel rather than adding a directory under `arch/`. It did not.
The x86_64 port reached a passing self-test on its own firmware with the boot entry, the mapper and
the discovery seam living under `kernel/src/arch/x86_64/`, which is what DECISIONS §4 rule 1 and §19
claim. Two boots were needed, not one. The defect between them was machine-specific and fixed
inside `arch/x86_64/mmu.rs`: `AlreadyMapped`, a fill that mapped device ranges cacheably
because the firmware's map does not describe the MMIO hole. That is the shape this entry predicts
for a healthy HAL, not the one it fears.

The port has a measured cost, not just a passing result, and it is the number this risk asked for.
`notes/x86-port.md`'s "What had to change above `arch/`" section counted it directly. Making the
whole kernel compile for the third architecture took 42 compiler errors, every one of them "this
`arch::` name does not exist yet." The diff outside `arch/` was three small, named things.
`crates/paging` needed no change at all (`Ia32e` is sixty lines behind the existing `PageFormat`
trait). `drivers/ns16550.rs` gained one type parameter, because the same 16550 is reached by MMIO on
two architectures and by I/O ports on the third, which is a `RegisterSpace` implementation rather
than a second driver. And `console.rs`, `user.rs`, `drivers/mod.rs` and
`kernel/src/user/fs_service.rs` gained `cfg` arms "in the same places they already had two." The
note's own conclusion: "That is the whole diff above `arch/`. A new ISA was a new directory." That
is the claim this risk exists to test, quoted rather than summarized, and it is what "GREEN" above
is based on.

### Two things this does not claim

The completion criterion was ruled by calef on the day to be the self-test rather than a byte over
serial, because a byte was printed on 2026-09-04 while the milestone plainly was not done. a reader
should take "the self-test passes" as the claim and nothing wider. And userspace was not reached on
that boot: the measured-boot gate refused the handover because `xtask::uefi_image` built the kernel
before the archive, so the kernel vouched for the previous one. That is a defect in this project's
build ordering, fixed the same day and verified under OVMF, and it is the second time that same
ordering defect has reached a bench.

A third thing it does not claim, and the verdict above did not say so when it was written hours
earlier today (2026-09-23). Every number in it comes from one machine per architecture, and for one
of the three not even that. x86_64 is xenon, a Core i5-7500T OptiPlex, on one firmware. riscv64 is
radon, one VisionFive 2. aarch64's silicon is patagonia's own cores under Apple's hypervisor
(notes/hvf-leg.md) plus QEMU, because argon has never booted nife at all, which the multicore
entry's own list says in as many words. The 42 compiler errors, the untouched `crates/paging` and
the one type parameter measure what a third architecture cost. They say nothing about what a second
machine of an existing architecture costs, because no second machine has been booted, and until the
widening above was written there was no wording under which anyone would have noticed the gap.
### This does not overturn the verdict

The measurement is real, it answers the question the sharpened claim asked, and re-reading it
changes not one of the numbers. What was unstated was its scope, and a verdict that states its scope
is stronger than one that leaves it to be discovered by the next reader.

What remains on this edge is no longer first light. It is the two-core defect under firmware, the
boot entry's remaining work, and the orchestrator, all of which are schedule rather than
restructure.

Milestone 161 turned BUILT on 2026-09-19, and that does less to this risk than the status word
suggests. What closed it was four follow-on items, none of them first light. 2 MiB and 1 GiB leaves
in `crates/paging`, adopted by all three architectures' direct maps through the same `PageFormat`
seam. A fourth architecture would implement two more trait methods rather than change the walk,
which is this entry's claim holding again. `cpu_start` counting a started core as absent,
reproduced at 26 of 40 four-core QEMU boots, gone in 80 of 80 after, and fixed in
`arch/x86_64/mod.rs` rather than in portable code. Last, `CR4.PGE` and `PCIDE`, measured and left
off. None of it touched the kernel outside `arch/` except `crates/paging`, which every architecture
shares. xenon has still not been asked to bring four cores online with the fix.

Milestones 177 and 182 turned BUILT on 2026-09-19, and the paragraph above is now settled the way it
predicted. §149 was decided: yes, a kernel-served console endpoint. Milestone 182 (x86_64's own
interactive-boot entry point) then reached a shell over serial on x86_64. `script/swish-check` has a
third leg that boots the UEFI image a customer's stick carries, types 60 of its 64 lines at the
prompt and reads the answers. Milestone 177 closed separately. And its defect is the interesting
half for this risk: the graphical boot hung because two userspace drivers each sent a one-time
report that the boot code had stopped receiving. So both sat in a blocking send. That is a wiring
mistake in a capability protocol, not an architecture-shaped cost, and it was identical on aarch64
and riscv64, which is this entry's claim holding rather than bending. So the sentence above that
reads "x86_64 has no real interactive boot entry point at all" is retired. And what remains on this
edge is what the 2026-09-19 entry for milestone 161 says remains: the orchestrator. And xenon
confirming four cores with the counting fix.

It is not the free hour this file first called it, and the correction is calef's, 2026-08-30, asking
why it should outrank finishing milestone 16 (real hardware + IOMMU-backed driver isolation).
`notes/x86-port.md`'s own `BUGS` says why: *"PVH is a hypervisor protocol and no real firmware
speaks it. Milestone 87's OptiPlex will need a UEFI stub or GRUB's Multiboot."* The kernel boots
under QEMU by PVH, and the OptiPlex's firmware does not speak it, so first light needs a boot entry
path that does not exist yet. It is bounded (the note says the 32-bit trampoline carries over
unchanged, because GRUB enters the same way; only the header and the `ebx` contract differ) and it
is a lane rather than a bench session.

Journey 3 (the same story, on real silicon, on all three architectures) is the full-strength
version, and it settles risk 6 along the way.

A citation this entry owed and had not paid, closed today (2026-09-23). Everything above verifies
rule 1 for the kernel. It never checked whether the rest of the tree keeps the same discipline. And
`notes/architecture-list-sweep.md` (2026-08-27) is the sweep that asked exactly that question,
tree-wide, and it is milestone 186 (derive the architecture list, and close what it does not
reach)'s worklist. Milestone 186 is `PARTIAL` (seven of the eleven closed on 2026-09-23, including
finding 9 below; `script/stack-depth-check` is the one left). None of what it found is inside
`kernel/src/arch/`. And none contradicts the port's own accounting above. Every one of its eleven
silent gaps is a script, a CI leg, or a userspace driver's own hand-rolled `#[cfg(target_arch)]`
arms. Rule 1 names those by its own text ("all architecture-specific code"), and its own citation
path (`kernel/src/arch/`) never covered them. Ten of the eleven are hardcoded two-item lists in
tooling that a third architecture walked past silently (`script/bench`'s own `EXAMPLES`, a CI bench
leg, a stack-frame checker, `deny.toml`'s target array). One was a live defect, a panic handler with
no x86_64 arm, since fixed.

The userspace half of that sweep is the shape this risk actually fears, one layer up from the
kernel. Finding 9 is five driver `barrier()` functions (`crates/virtio`, `gpu_driver.rs`,
`keyboard_driver.rs`, `net_transport.rs` and, found rereading the table for this entry rather than
by the original sweep, `components/src/entropy.rs`) with an `aarch64` arm and a `riscv64` arm and no
`x86_64` arm. So on x86_64 each compiled to an empty body: not a build failure, a silently missing
compiler fence. `components/src/non_volatile_memory_express.rs` has all three arms, because the NVMe
driver was x86_64's own reason for existing (DECISIONS §86 (whether an NVMe driver can leave the
kernel, and what capability would let it)), which is the control case. When a driver is built *for*
the new architecture the third arm arrives with it. And when it predates the architecture it does
not. Closed 2026-09-23 by milestone 186 into one function,
`user_mode_runtime::virtio::virtio_ring_barrier` (name provisional), whose fourth-architecture arm
is a `compile_error!`; [`notes/architecture-list-sweep.md`](../../notes/architecture-list-sweep.md)
has the eleven findings, the classification and what is still open.

Read against the widening at the top, those five functions are the concrete instance of what it
names. The failure was not a restructure and not a build error. It was silence: five functions that
compiled, linked, shipped and did nothing on a machine nobody had run them on, found by somebody
rereading a table rather than by any gate. A platform difference presents the same way. A machine
whose firmware leaves a bit set that xenon leaves clear, or whose memory map is shaped differently,
does not announce itself; it produces a kernel that works everywhere anyone has tried it. One honest
cost, recorded here rather than left implied: parity is a multiplier on every other risk on this
list. Every driver, benchmark, proof and bring-up is three times the work. The tree's own evidence
says the multiplier is smaller than it sounds once the HAL is right, which is what riscv64
demonstrated. But if the project ever needs to buy time, dropping to two architectures is the
largest single lever available and it should be a decision rather than a drift.
