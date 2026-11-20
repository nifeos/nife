# Security audit, 2026-09-17: userspace confinement, read adversarially

**Kind:** security. **Lens:** userspace confinement, read for the claims this tree makes in public
that are not true, or are true only by accident: the device and network authority minted since
2026-08-17, the claims milestone 307 marked as quotable-but-unreachable, and the two machine classes
(radon, xenon) that booted real silicon in the window. **Findings:** fixed 3, minted 3, accepted 1.

**One confinement claim in this tree was false as stated, and it is fixed.** DECISIONS §12 says a
consumed capability cannot be used again. On `x86_64`, for the one object whose enforcement lives
outside the capability table, it could: a thread that `SYS_CAP_DELETE`d its own `PortRange`
capability kept `in`/`out` access to COM1's ports for the rest of its life, because the grant the
context switch installs is a cached field and the delete cleared the table and not the cache. It
was live rather than theoretical: `system_initializer` deletes its console device capability on
every boot, and on `x86_64` that capability is this object. Nothing outside the holder gained
authority through it, which is why it is a claim failure and not an escape.

**Nothing exploitable was found.** No path widens rights, forges a capability, or reaches a device,
a page or a port the caller was not handed. The thing a reader should carry off besides finding 1
is finding 3: the `paging` crate's `x86_64` decoder reported every user page as not
kernel-executable, `notes/confinement-claims.md` said the hardware made it so, and the hardware
does so only with `CR4.SMEP`, which nothing set. That is not a userspace escape (it needs a kernel
bug first), but it is a sentence this tree published about its own confinement that the machine
contradicted, which is exactly what `design/fatal-risks/README.md`'s risk 7 asks this cadence to find.

## Why this ran, and why this lens

`script/audits` had been reporting a security audit overdue every week since 2026-08-17, and
milestone 311 read the record and said so: 112 milestones and 45 components against triggers of 15
and 8. The two uncountable triggers the script prints as questions were both **yes**: components
took device authority in the window (the JH7110 TRNG driver on radon, milestone 159/220; the
`PortRange` capability on x86, milestone 299), and the system booted two new machine classes (radon,
a VisionFive 2, since 2026-08-14 with real peripherals since 2026-09-04; xenon, an OptiPlex 7050
under its own UEFI, since 2026-09-05).

`design/audit-reports/README.md` names three lenses not yet taken: supply chain, userspace
confinement, the system call surface. Confinement was chosen because the window produced direct evidence
that this tree's confinement claims had been asserted without being testable: milestone 305 found a
kernel test that had answered "U-mode cannot read the kernel" by refusing to look since milestone
41, and milestone 307 found six more rows whose quotable assertion cannot run. So the question here
was not "is there a test", which 307 counted. It was **which claims are true only by accident**, and
the method for that is to break things on purpose and see what turns red, which is how every
finding below with a fix was confirmed.

### What was deliberately not examined

An outside reviewer should start here.

- **The system call surface as a whole, and the IPC model.** `SEND_CAP`, `CAP_INSERT`, `derive`, the
  rendezvous fast path and the reply capability were read only where a device or port object passes
  through them. The 2026-07-15 and 2026-08-17 audits own the rights model; the system call surface is
  the third untaken lens and should be its own audit.
- **The DMA validator and the IOMMU domain builder.** Rows 12 to 17 of the confinement table are
  Kani-proved, milestone 307 read every assertion in them, and their falsification records replay on
  every pull request. Re-reading them here would have been re-running 307's sweep.
- **Components that took *credential* authority in the window** (`login`, `identity_provisioner`,
  `credentialer`, `session_reviver`, `audit_sink`, milestones 130-ish onward). §74's event trigger
  names device and network authority, and that is the population this audit took. A credential is
  authority of a different kind, and DECISIONS §79's password-material ruling is the standing
  review of it; reading it adversarially wants the counterparty-input lens, not this one.
- **The network stack.** `net_transport`, `net_stack` and `network_time_client` all predate the
  window and were audited under the 2026-08-15 lens. `socket_protocol` moved directories on
  2026-09-14 (milestone 175) and changed nothing.
- **The C seam, the vendored RedoxFS engine, and the compositor.** Each has its own negative-control
  catalog and was left to it.
- **The hand-written assembly**, including the `x86_64` `syscall`/`sysret` pair and the `swapgs`
  convention. That is the 2026-07-29 lens, and `trap.s` was not read.
- **Timing.** DECISIONS §139 records the position (a cycle-counter grant buys accountable authority,
  not timing confinement, and `x86_64` keeps an ambient `rdtsc`); it was checked against the arch
  code (`PMUSERENR_EL0`, `scounteren`, `CR4.PCE`) and found as stated. No channel was measured.
- **argon.** The Jetson has not booted nife; the "new machine class" trigger covers radon and xenon.
- **Multi-core races on the port path**, beyond the one recorded in finding 4. The port tests run on
  one core and this audit did not write a two-core one.

## The population, found by reading grants rather than names

Forty-five components landed in the window by count, and the brief said not to trust names. Every
site where the kernel mints a device, interrupt, port or transport capability was read
(`cap::device_frame_cap`, `irq_cap`, `irq_cap_rights`, `port_range_cap`, `virtio_cap_rights` and
their callers in `kernel/src/user.rs` and `kernel/src/user/*_service.rs`), and every
`PageKind::DeviceRegisters` need in `system_initializer`. The result is smaller than the count:

| Authority | Holder | Landed | What was checked |
|---|---|---|---|
| One device-typed page of the JH7110 TRNG (`0x1600c000`, `0x1000` of a `0x4000` window) | `jh7110_entropy` on radon | 2026-08-24, wired 2026-09-01, live 2026-09-04 | The page holds only that block's registers; no IRQ, no DMA page, no second slot; the clock and reset the block needs are ungated by the **kernel** (milestone 220) and the STG CRG page is never mapped into any address space. Nothing to find. |
| `PortRange(0x3F8, 8)`, COM1's eight ports, enforced by the TSS I/O bitmap | the progenitor (`WRITE\|GRANT`), delegated `WRITE`-only to the console and input drivers | 2026-09-15 (milestone 299, DECISIONS §121 reversed) | **Findings 1, 2 and 4.** |
| `Irq(COM1)` (`READ\|GRANT` to the progenitor, `READ` to the input driver) | as above | 2026-09-15 | `WAIT` and `ACK` both require `READ`; `ACK` re-enables at the controller and nothing else. A `READ`-only holder cannot delegate. |
| `Irq(tick)` to the soak fixture's waiter role | `soaker` | 2026-09-01 | A per-group tick line, not the scheduler's; the waiter holds nothing else. |
| Nothing | `interrupt_ignorer` | 2026-09-13 | Holds no capability at all and maps nothing; it exists to be killed. |

Thirty-seven of the 45 counted components are the 2026-09-14 `user/` split moving existing
fixtures across a directory boundary (milestone 175; 72 targets before, 73 after) and hold nothing
they did not hold before. The remaining new components (`pgrep`, `timetable`, `pmap`, `printenv`,
`uptime`, `uuid`, `rmle`, `least_authority_demo`, `mdr`) hold endpoints and filesystem handles,
which is not this lens.

**Lent copies carry no `GRANT`.** This was checked because `DeviceFrame::REVOKE` and
`PortRange::REVOKE` are take-backs that need `GRANT` and delete every *other* holder's capability,
so a driver lent its device with `GRANT` could take it from its own supervisor and, since the kernel
mints these once at boot, from the machine for good. The progenitor delegates the UART page as a
mapping (no capability in the child) on aarch64 and riscv64, and the port range with `WRITE` alone
on `x86_64` (`system_initializer` lines 1116 to 1124, 1240 to 1246). Not a finding, and worth one
sentence because the object is new.

## The two machine classes

**radon (JH7110, no IOMMU).** RAM extent comes from the device tree, and `kernel/src/memory.rs`
honors both the legacy reservation block and every `/reserved-memory` child, so OpenSBI's
PMP-protected region cannot be handed out as untyped. The only userspace device on the board is the
TRNG page above. No component drives a DMA-capable device there, so the software validator that
`SECURITY.md` calls the single point of failure on that board is not on any path a confined
component can reach today; that sentence stays true and stays latent.

**xenon (OptiPlex 7050, UEFI, VT-d).** The loader's `e820_kind` turns only `Conventional`,
`BootServices*` and `LoaderCode` into RAM; runtime-services code and data, `LoaderData`, and every
type the loader has never heard of are reserved, in the direction "claiming less RAM costs
megabytes, claiming more corrupts something" (`uefi_loader/src/handoff.rs`, with a test for the
unknown-type case). Firmware-owned memory does not become untyped. The console is the port range
above, and its cross-core behavior is finding 4. Interrupt remapping is the standing latent claim in
`notes/confinement-claims.md`'s fifth entry: the firmware note found there is no menu control for
it, the DMAR has never been read, and `design/roadmap/0378-read-the-dmar-on-xenon.md` already
proposes reading it. It stays latent because no component holds a DMA-capable device on xenon.

**xenon passed its self-test on real firmware while this audit was running** (milestone 87, closed
2026-09-17 09:55 UTC, `bench/xenon-2026-09-17/first-light-095500.log`, with the line `VT-d drhd at
0xfed90000, root table default-deny, translating`). That turns the second machine-class trigger from
a question into a transcript, and it bears on two things here. The DMAR *has* now been found by the
kernel's own walk on that machine, so the "never read" above is one step narrower: the unit is
located and its root table is installed default-deny; what is still unread is the `INTR_REMAP` flag
and the unit's `IR` capability, which is the half the latent claim depends on and the half the
proposal asks for. And that boot predates finding 3's change, so it carries no `cr4.smep` line:
xenon's Kaby Lake offers SMEP and the next boot there is the first that will say whether the bit
was set, which is the one machine this audit could not check.

*Corrected 2026-09-25 (UTC), from pull request #1275, the rehearsal for milestone 261 (the NVMe
driver leaves the kernel): "the unit is located" should read "the first of the DMAR's units is
located". The kernel carried only the first DRHD. On the OptiPlex 7040, which shares xenon's
register addresses, that unit (`0xfed90000`) covers only the integrated graphics and a second,
catch-all unit covers everything else. So the `IR` bit xenon later reported
(`bench/xenon-2026-09-17/tour-display-225100.log`) is that unit's, and says nothing about the unit
in front of the NVMe. This is inferred from the sibling machine and is unverified until milestone
261's bench evening reads xenon's DMAR. This audit's conclusion stands: it rested on no component
holding a DMA-capable device on xenon, and none did.*

## What milestone 307's six rows actually guarantee

The brief asked, for the six rows whose quotable assertion cannot run, what is actually guaranteed.
Read again with 307's own notes beside the code:

| Row | The sentence a reader quotes | What the live assertion guarantees |
|---|---|---|
| 4 | `get`/`delete` on a deleted slot refuse | the slot is empty (`slots[slot].is_none()`), which the refusals are consequences of |
| 13 | no byte the device touches is outside the region | the two half-inequalities, whose conjunction the sentence was |
| 14 | an accepted descriptor is direct and in-region | `check_descriptor`'s own two calls, which are the same predicate |
| 15 | an oversized batch is refused | the panicking closures the walk would call if it were not |
| 20 | the read-only witness is intact | a death report arrived within 30 s (`wait_for_report`), which a landed store would not produce; the witness bits are checked only on the path where they cannot have changed |
| 24 | no shell named a file in the other's root | the per-shell bitmaps, which any crossing sets a bit in one call earlier |

None of the six is weaker than its claim; in every case the live mechanism implies the quoted
sentence. The one worth a caveat is row 20, and 307 already carries it: the assertion that prints
"read-only witness intact" can fire for everything except a broken witness. Nothing here changes
307's verdicts.

## Findings

### 1. FIXED: a thread that deletes its own port capability keeps the ports

**The strongest finding, and the one to raise if only one can be raised.** `Thread::port_range_grant`
is the per-thread cache the context switch reads to install the TSS I/O bitmap (milestone 299). It is
set at the one choke point a `PortRange` enters a thread (`thread_control_block_insert_cap`) and
cleared by `delete_port_range_caps_impl` on `REVOKE`. `sched::delete_current_cap`, the body of
`SYS_CAP_DELETE`, cleared the table and never looked at the cache. So a thread that dropped its own
port capability kept the ports: the table said the authority was gone and every switch-in reinstalled
it.

**It is live.** `crates/system_initializer/src/lib.rs` calls `cap_delete(g.uart_dev)` on every boot
(lines 895 and 1285, to free the slot). On `x86_64` `g.uart_dev` is the `PortRange`. The progenitor
is privileged and `SECURITY.md` says so, but the mechanism is not the progenitor's: any console
driver that shed its port capability would have kept its ports.

**The claim it breaks** is DECISIONS §12's, row 4 of the confinement table: a consumed capability
cannot be used again. The four `capability` proofs behind that row are about the table, and the
table was correct; the `PortRange` is the first object whose enforcement lives outside it, and the
two records of one authority had one path that updated only one of them.

**Fixed** in `delete_current_cap`: when the deleted capability is the cached `PortRange`, the cache
is cleared under the lock and this core's bitmap is reset after it, so the next `in`/`out` faults
rather than the next-but-one. One field read on the ordinary path (`None` for every thread but a
port holder), so the reply consumption the same function serves is untouched in the case the IPC
round trip measures.

**The test** is `x86_port_tests::a_holder_that_deletes_its_port_capability_faults_on_its_next_port_write`,
the drop-it-yourself twin of milestone 299's two, and its falsification record was replayed by hand
with the sweep's own argument list: red at the named assertion, `left: 2` (`EVENT_EXIT`), `right: 1`
(`EVENT_FAULT`), and green with the fix. Its first draft is finding 2.

### 2. FIXED: the two existing port tests could not fail in the direction they exist for

The first draft of finding 1's test reported after its `out`, like the milestone 299 children do, and
on the unfixed kernel **the run hung** rather than going red: the permitted `out` was followed by a
`SEND` on a rendezvous the test was not receiving (it was blocked on the supervision endpoint waiting
for a fault), the child parked forever, and so did the test. That is `notes/confinement-claims.md`'s
row 26, one object over: a real escape that blocks instead of reporting is not evidence.

Read back into milestone 299's two tests, both have the same shape. The non-holder in
`port_holder_transmits_then_a_non_holder_faults` and the revoked holder in
`a_revoked_holder_faults_on_its_next_port_write` both report after an `out` that is expected to
fault, so a leaked hand-off or a surviving grant would have hung the suite instead of turning either
assertion red. Milestone 299's own BUGS called both properties load-bearing, and neither could have
gone red for the defect it names.

**Fixed in the fixtures, not the claim.** A child whose `out` is expected to fault now exits after it
(`x86_programs::port_out_then_exit`; `recv_then_port_out`'s tail is now an exit), so a wrongly
permitted `out` arrives as `EVENT_EXIT` in the message the test wants `EVENT_FAULT` in. The holder
that must transmit keeps `port_out`, because its word arriving is the positive half. Both tests now
carry a falsification record, each replayed on `x86_64` (the verdicts are in the claims table's new
rows, and the records say which defect: an early return on `None` in `set_port_range_grant` for the
hand-off, and the revoke path leaving the cache alone for the revoke). The first defect tried for the
hand-off did **not** fire and is recorded in that patch: removing only the outgoing holder's re-deny
leaves `iomap_base` past the segment limit, so the CPU consults no bitmap and the non-holder still
faults. The hand-off is protected twice, which is worth knowing about the design.

### 3. FIXED: ring 0 could execute a confined component's pages on `x86_64`, and the tree said otherwise

`crates/paging`'s `x86_64` encoder carries the comment "one execute permission, applying at
whichever ring `U/S` names", and its decoder reports a user page with `XD` clear as user-executable
only. Milestone 307 wrote, of `a_read_only_segment_is_mapped_read_only`'s `!is_kernel_executable()`
assertion, that on riscv64 and `x86_64` "the hardware really does make a user page non-executable in
supervisor mode". True for RISC-V, which refuses a supervisor fetch from a `U` page
unconditionally. **False for `x86_64`**: `XD` is the only execute bit and applies at every ring, so a
user code page is executable at ring 0 unless `CR4.SMEP` is set, and nothing in this kernel set it
(`grep -rn SMEP kernel/` found the word nowhere).

This is a claim stated nowhere, which is what the brief said to look for: no row of the confinement
table says "the kernel cannot execute a confined component's code", and no test asks. It is not a
userspace escape on its own, since a kernel control-flow bug has to come first; it is the thing that
turns any such bug into a full one, and it is a sentence this tree published about what its hardware
does that the hardware did not do.

**Fixed:** `arch::x86_64::init` now sets `CR4.SMEP` on every core whose `CPUID.(7,0):EBX[7]` offers
it, beside the existing `CR4.PCE` close, and prints one line either way, so a machine without it says
on the console that ring 0 can execute ring-3 pages there. Cost: nothing on any path, because SMEP
has no per-access toggle, which is what separates it from SMAP (`stac`/`clac` around every kernel
access to user memory; SMAP stays off, with the reason `mmu::permit_kernel_access_to_user_pages`'s
`BUGS` already records, and aarch64's `PAN` likewise). Nothing this kernel does is forbidden by it:
every kernel page is `U/S`-clear and user code is only ever entered through `iretq`/`sysret`. The
encoder's comment and 307's sentence are corrected rather than deleted, because the citation is the
interesting half. QEMU's `-cpu max` offers the bit and the full `x86_64` boot printed `cr4.smep: set
on core 0`; xenon's Kaby Lake offers it and has not been booted with this change.

**What is not fixed, and is minted (finding 6):** there is no test. A falsification would need ring 0
to fetch from a user page and *recover* from the `#PF`, and this kernel has no fault-recovery path for
its own faults. Until one exists the boot line is the evidence, and a boot line is rung three.

### 4. ACCEPTED: `PortRange::REVOKE` reaches one core, and `x86_64` no longer runs one

DECISIONS §152's BUGS, `sched::delete_port_range_caps_impl`'s closing comment and
`segments::revoke_installed_port_grant`'s doc all said x86 runs a single core today
("`smp::bring_up_secondaries` refuses on `x86_64`"), so the core-local TSS reset was the whole
machine. It stopped being true when `smp::seat_cpus_from_acpi` landed: the tour boots two cores under
OVMF (`notes/x86-uefi-boot.md`). So on a multi-core x86 a revoked holder that is *running on another
core* keeps that core's bitmap until its next context switch, at most one tick, during which its
`in`/`out` still succeed. The cached grant is cleared before then, so the window cannot reopen.

Accepted rather than fixed, with the reason: the window is bounded by the tick, no consumer runs a
port holder on two cores (a console driver holds COM1 and there is one), and the IPI shootdown that
would close it is the shape of the TLB shootdown this tree already has
(`notes/x86-tlb-shootdown.md`), which is a milestone rather than an afternoon. Recorded where a reader
meets it: the two kernel comments are corrected, and `revoke_installed_port_grant` now has a `BUGS`
section. §152 is `design/decisions/` and was not edited; its BUGS entry is now stale by this finding
and the maintainer should update it. The shootdown is proposed in
`design/roadmap/proposals/a-port-revoke-that-reaches-every-core.md`.

### 5. MINTED: row 21's `x86_64` leg is a green test with no evidence it can fail

`a_user_program_cannot_read_a_kernel_address` runs on aarch64 and `x86_64`, and its falsification
record declares `Architecture: aarch64`. `script/falsifications` sweeps a kernel record on **one**
architecture, the one the patch names, and a record's filename is the test's name, so a portable test
can carry one architecture's evidence and no more. On `x86_64` the row is therefore exactly what 305
warned about: a green confinement test that has never been shown able to go red. The aarch64 defect
(map the kernel's `.rodata` `U/S`-accessible) would work on `x86_64` too, since SMAP is off and the
kernel keeps reading its own constants, and nothing can record it.

The confinement table's row 21 now says so beside the "on every ISA" paragraph. The mechanism change
(a per-architecture record, `<name>.<arch>.patch` or an `Architecture:` list) is proposed in
`design/roadmap/proposals/a-falsification-record-per-architecture.md`.

### 6. MINTED: a test that ring 0 cannot execute a user page, and SMAP with a number

Finding 3's fix is a control-register bit with a boot line for evidence. The test that would make it
a claim rather than a setting needs the kernel to survive its own page fault, which is also what
turning SMAP on would need for its own test. Both belong to one lane, and the SMAP half owes the
system-call-path measurement the existing `BUGS` entry asks for. Proposed in
`design/roadmap/0424-a-ring-0-that-provably-cannot-execute-ring-3-pages.md`.

### 7. MINTED, outside the lens: the `x86_64` boot-stack gate fires on a filtered run and not on the suite

Found while gating, and corrected once the full suite had run: an earlier draft of this finding read
the filtered result as the machine disagreeing with CI, and the full suite showed it is the filter
disagreeing with the suite. `language_tests::a_refusal_and_a_success_report_different_numbers`, run
**alone** (`--test a_refusal_and_a_success`), drives the `x86_64` boot stack to **62456 bytes against
`stack.rs`'s 61440 limit**, deterministically (four runs, four identical numbers), on the base commit
`52da4ae4`'s own `kernel/` and `crates/`. The **full** `x86_64` suite on this lane's tree is green in
both boot modes, with the boot stack at 53144 (PVH, 243 passed) and 50272 (UEFI, 214 passed). So the
same test sits about 9 KiB deeper when it is the only one selected. Nothing in this lane's diff moves
either number. It is not a confinement matter and not this lane's to fix; it matters because a
filtered run is how a person reproduces one failure and how `script/falsifications --sweep` replays
every kernel record, and a gate that fires only there would report a red for the wrong reason.
Proposed in `design/roadmap/0425-a-stack-gate-that-fires-only-on-a-filtered-run.md`.

## Is any confinement claim in this tree false as stated?

Risk 7's question, answered directly. **Yes, one, and it is fixed**: §12's "a consumed capability
cannot be used again", as it applies to the `PortRange` on `x86_64` (finding 1). One further published
sentence about what the hardware guarantees was false for `x86_64` and is corrected, with the bit it
depended on now set (finding 3). The headline claim, that a confined component cannot reach past the
boundaries the kernel enforces, was not found false anywhere this audit looked: no new authority
minted in the window widens, leaks or survives revocation on the path a confined component can take,
with finding 4's one-tick window recorded as the exception.

## Method, so the negatives can be judged

1. Read `design/audit-reports/README.md`, `notes/confinement-claims.md` in full, risk 7, and the
   2026-08-17 report for shape.
2. Built the population by grants: every `*_cap` mint site in the kernel, every `DeviceRegisters`
   need, every lent copy's rights; dated each by `git log --diff-filter=A` and discarded what the
   directory split moved.
3. For each device object, asked what the holder can do with the object beyond its stated purpose:
   revoke the lender (no, lent copies lack `GRANT`), reach a neighboring device through the same
   page (no, the TRNG page and the UART page hold one block each), keep it after dropping it
   (**yes**, finding 1), keep it after losing it on another core (**yes for one tick**, finding 4).
4. For each ISA-specific protection the confinement table relies on, asked whether the control bit
   the claim depends on is set: `PXN` on aarch64 (encoder), `U`-fetch refusal on riscv64 (hardware,
   unconditional), `SMEP` on `x86_64` (**not set**, finding 3), `PAN`/`SMAP`/`SUM` (off and
   recorded), `IOPL` (0, and `FMASK` clears it on every `system_call`), the TSS bitmap's deny-all base.
5. For the boards, followed firmware-owned memory from the memory map or device tree into the
   frame allocator, and the device windows into the address spaces that hold them.
6. Broke each new or reshaped confinement test on purpose and replayed it, which is how findings 1
   and 2 were confirmed and how the first hand-off defect was found not to fire.

What this method cannot see: a claim that no document and no test states and that no grant table
hints at. Finding 3 was found by asking "what bit does this row's truth depend on", which is a
question with a finite answer per ISA; nothing here asked it of the DMA path or the IPC path.

## What wants a lane of its own

- **The port-revoke IPI shootdown** (finding 4's proposal).
- **A falsification record per architecture** (finding 5's proposal), which also rescues the aarch64
  twin `the_hardware_says_el0_cannot_read_the_kernels_memory`'s sibling on `x86_64`.
- **Ring 0 provably cannot execute ring-3 pages, and SMAP with a number** (finding 6's proposal).
- **The stack gate that fires only on a filtered run** (finding 7's proposal).
- **DECISIONS §152 (the port-range capability)'s BUGS and `design/fatal-risks/README.md` risk 7 are the maintainer's to update** from
  this report: the single-core premise, and the sentence that one claim was false and fixed.
- **The system call surface** is the remaining untaken lens, and this audit deliberately read none of it
  beyond the four arms that carry a device object.

## Process notes on the mechanism itself

- **Breaking things on purpose found every fix here.** Reading found finding 1 as a suspicion and
  finding 3 as an argument; the replay is what made finding 1 a fact and what found finding 2 at all.
  Milestone 305's headline ("the only instrument that could find it was a falsification") held again.
- **The hang is the shape to look for.** Three port tests, all hang-shaped in their defect direction,
  none of them noticed until one was broken on purpose. A confinement test whose escape blocks is a
  test that cannot go red, and `notes/confinement-claims.md`'s row 26 is not the only instance.
- **The `?` triggers earned their line.** Both uncountable questions were yes, and both led to the
  findings; the count triggers would have reached the same place three weeks later.
