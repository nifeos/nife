# ISA discovery: reading the machine instead of assuming it

Until milestone 60 this kernel ran on what the target triple implied plus exactly one runtime
measurement. Nothing read `riscv,isa-extensions`, nothing read `mmu-type`, and on aarch64 the only
`ID_AA64*` field anyone touched was the `PARange` that `TCR_EL1.IPS` needs. That is fine on an
emulator, which is configured to be whatever you ask for. It stops being fine on a board.

The shape is one record per architecture, populated once at boot, printed at boot, in
[`crates/machine_discovery`](../crates/machine_discovery) with the kernel halves in
`kernel/src/arch/*/isa.rs`.

## What the boot says now

RISC-V, on QEMU `virt` with `-smp 4`:

```
  isa         : rv64i m a c f d h zicsr zifencei sstc zicbom
              : 4 hart(s), mmu sv57 declared and sv39 in use, satp.ASID 16 bits measured
  firmware    : OpenSBI 0x10007, SBI 3.0, TIME IPI RFENCE HSM
```

aarch64, on QEMU `virt` with `-cpu cortex-a72`:

```
nife
  exception level : EL1
  cpu             : Arm part 0xd08 r0p3
                  : 44-bit PA, 48-bit VA available, 16-bit ASIDs, granules 4K 64K
```

Without discovery, everything on those lines is either unknown to the kernel or assumed.

## The three tiers, and the one RISC-V does not have

The roadmap entry names three ways to learn what a machine is, in decreasing order of how much you
should like them. A firmware claim is a device-tree property. A targeted measurement writes a
value and reads back what stuck. Trap-and-detect executes it and catches the illegal-instruction
fault. We do the first two and none of the third.

Building both ISAs at once exposed a tier the list is missing, and it is the reason the two halves
of the crate look nothing alike.

aarch64 has a tier 0: the CPU describes itself. `MIDR_EL1` and the `ID_AA64*` space are
architected, mandatory, and read straight off the part in front of you. Not hearsay, and not a
measurement anyone had to design. So the aarch64 half is a decoder: three `mrs` reads and a
handful of shifts.

RISC-V removed that tier deliberately. `misa` exists, is coarse, is permitted to read as zero,
and cannot name a multi-letter extension, which is every extension ratified after 2015. So the
architected answer is a property firmware wrote into the device tree, and the RISC-V half is a
parser. It keeps its tier-2 probe (`satp.ASID`) even now that the tree answers, because a claim
and a measurement are different things and when they disagree the machine wins.

That asymmetry is worth stating plainly rather than smoothing over with a trait: the same question
has genuinely different best answers on the two architectures.

## How many call sites actually vary

The entry's own effort note said the unknown was how many call sites genuinely need to branch on a
discovered fact, and measuring that honestly was part of the deliverable. The answer is four, and
two of the entry's four candidates turned out not to be among them.

| Candidate | Verdict |
|---|---|
| ASID width | **Real, both ISAs.** `crates/address_space_identifier` hands out 255 numbers on the stated assumption the hardware can tell them apart. riscv64 measures it (`probe_asid_bits`); aarch64 reads `ID_AA64MMFR0_EL1.ASIDBits`. |
| Sv39 versus Sv48 | **Real, as a refusal.** riscv64 stops on an `mmu-type` narrower than Sv39; aarch64's twin is the 4 KiB granule, which `TGran4` may refuse and every page table here depends on. |
| `TCR_EL1.IPS` from `PARange` | **Real, and it predates this milestone.** The one place the kernel already read the machine, now read once into the record. |
| TLB flush strategy | **Varies nowhere.** The unconditional `sfence.vma` in `write_satp` is unconditional by design, and removing it is its own milestone gated on the ASID probe. `Svinval` is recorded and acted on by nothing. |
| IOMMU presence | **Already discovered, elsewhere.** The `smmuv3@` device-tree node and PCI enumeration answer this, and the record would add a second way to ask the same question. |

Plus the requirement checks, which are branches that go exactly one way. RISC-V refuses a machine
that is not RV64, that lacks `m`/`a`/`c`, or whose MMU is narrower than Sv39. It also refuses one
whose firmware says it does not implement one of the four SBI extensions the kernel calls
unconditionally.

A fifth call site was never reached. Four is what the entry predicted the ceiling should be, and
the two that dropped out are the more interesting result. A fact you already discover another way,
and a fact nothing branches on yet, both look like they need a record and do not.

## What the machine corrected, twice

Both after the host tests were green, which is the whole argument for booting the thing.

The SBI spec version is 24 bits of minor and 7 of major. Not the obvious 16 and 16. QEMU's
firmware reports `0x0300_0000`, which is SBI 3.0; decoded as 16/16 that is version 0.0, and since no
conforming firmware reports 0.0 the kernel used it as the signal for "the base extension did not
answer". So the boot line reported firmware that had answered perfectly well as silent, and the test
that asserts OpenSBI is present failed with a message about firmware from 2020. Found by printing
the raw word.

**QEMU `virt` declares `mmu-type = "riscv,sv57"`.** The machine this project has developed on for
two milestones is two whole page-table levels *wider* than the kernel, and nothing anywhere said so.
The worry going in was a board too narrow for us. This is the other direction, and it is invisible
without reading the property, which is the point.

## Three things that would have broken on the board

None of these were hypothetical worries; each is a shape the VisionFive 2 or its firmware has.

**The deprecated `riscv,isa` string.** `riscv,isa-extensions` and `riscv,isa-base` arrived in Linux
6.6 (2023). A vendor tree older than that carries only `riscv,isa = "rv64imafdc"`, so a parser that
reads the modern properties and gives up finds nothing at all. Both forms parse, and
`Isa::legacy_isa_string` records which one answered.

**`g` is an abbreviation, not an extension.** `rv64gc` means `imafdc` plus `zicsr` and `zifencei`. A
parser that looks `g` up in a table finds nothing and reports a machine with no multiplier, which
would then fail the `m` requirement and refuse to boot on a perfectly good core.

**Requiring `zicsr` would refuse the board.** Both `zicsr` and `zifencei` were carved out of the
base `I` extension in 2019, so a string written before that (or by firmware that never caught up)
simply does not list them while the hardware has them. The kernel uses both on every trap and in
`sync_icache`, and neither is in `REQUIRED`, because a check that fails on the machine you are
buying is worse than no check. `m`, `a` and `c` carry no such ambiguity and are what gate the boot.

## Why every CPU node, not `cpu@0`

A RISC-V machine may be heterogeneous, and the JH7110 on a VisionFive 2 is: application cores beside
a smaller monitor core, described as separate `cpu@` nodes with different `riscv,isa` strings. A
kernel that reads the first node and then schedules a thread onto any hart has read the wrong node
some of the time.

So `dtb` grew [`node_props`](../crates/device_tree_blob/src/lib.rs), which answers for every matching node rather
than the first, and the record carries two sets:

- **`common`**, the intersection over every hart that describes itself. This is what "an instruction
  the kernel may emit" actually means.
- **`any`**, the union. Without it the intersection silently hides heterogeneity, and you cannot
  tell "this machine has no FPU" from "one of its harts has no FPU". The boot line prints the
  difference when there is one.

`mmu-type` is taken the same way: the narrowest any hart declares, because Sv57 on one core is no
use to a thread the scheduler might place on the Sv39 one.

The test fixture for this (`crates/machine_discovery/tests/fixtures/mixed-cpus.dts`) is hand-written and says so
in its own header. It is modeled on the shape of a heterogeneous RISC-V SoC; the values are
invented. When the board arrives, dump its real tree and add it beside this one.

## Silence is not a failure

The truthfulness habit here cuts both ways, and getting only one direction right is how a discovery
layer becomes a liability.

A machine missing something the kernel needs is refused, loudly, with the missing thing named. A
machine that simply does not describe itself is not. A device tree with no `riscv,isa` at all
describes a machine that is nonetheless executing the code asking the question, and firmware too old
to implement the SBI base extension cannot be asked what it implements. Treating either as a failure
would refuse to boot on hardware that works.

So `Isa::missing_requirements` reports nothing about what the tree does not mention,
`Isa::described` says how many harts spoke, `Sbi::has_answered` says whether the firmware could be
asked, and the boot line says "the device tree does not describe this machine's ISA" or "SBI base
extension did not answer, so nothing here is verified" rather than reporting zeroes as facts.

## The trap, and why there is no trait

`if isa.has_x()` sprouting across the kernel turns a fact into a hundred branches, and a chip
abstraction built on one board is the wrong abstraction built early. Two guards, both structural:

The record is `Copy` with public fields and exactly one verb, `missing_requirements`, which is
the only thing a call site is meant to branch on. Everything else exists for the boot print.

And there is no trait, no `Cpu` abstraction, nothing shared between `machine_discovery::riscv64`
and `machine_discovery::aarch64` but the module tree. Two records that share no code is the honest
shape when two architectures answer the same question by unrelated mechanisms. The second real board
is what should tell us what the abstraction is, if there is one.

## Naming

calef named the crate `machine_discovery` on 2026-08-23, after the "machine" that Linux ARM and
FreeBSD use for the same device-tree-plus-probe scope. Not `isa`, because it collides with the ISA
bus and undersells the crate's scope. The reasoning and the names it beat are in
[the-crate-name.md](isa-discovery/the-crate-name.md).

## The tables, and a line nothing covers

The four lookup tables (`TABLE`, `SBI_TABLE`, `IMPLEMENTATIONS`, `aarch64::IMPLEMENTERS`) are
arrays with a `const` assertion rather than `match`es, because a duplicated `match` key compiles
silently and a duplicated array key does not. The SBI extension ids are derived from their tags
(`eid("TIME")`). And `bit(n)` is deliberately uncovered by tests, because every caller is a
`const` initializer. The reasoning for each is in
[lookup-tables.md](isa-discovery/lookup-tables.md), for whoever edits the crate.

## The sequel: PSCI and the CPU list (milestone 100)

Milestone 60 read what the *part* is. It left the one subsystem that assumes a *board*: SMP
bring-up. Three facts were compiled in, and all three are stated by the same device tree everything
else was already reading.

| Fact | Where it was hardcoded | Where the machine states it |
|---|---|---|
| The PSCI conduit is `hvc` | `psci_cpu_on`'s `hvc #0` | `/psci`'s `method` |
| The `CPU_ON` function id is `0xc4000003` | a `const` in the same function | `/psci`'s `cpu_on`, or the 0.2 standard when `compatible` claims it |
| The cores are `0..MAX_CPUS` | `bring_up_secondaries`'s loop | `/cpus` |
| RISC-V's counter runs at 10 MHz | `TIMEBASE_HZ`, a `const` | `/cpus/timebase-frequency` |

The fourth row is the one that names the pattern rather than an instance. `arch/riscv64/timer.rs`
carried the comment *"hardcoded until the DTB parse lands"*, and the DTB parse landed with milestone
60; the comment outlived it by two months. aarch64's twin has always read `CNTFRQ_EL0` and asserted
it nonzero, so the two ISAs disagreed about whether the machine gets to say how fast its own clock
runs. That is a rule-5 parity gap, and it was invisible because both answers were 10 MHz on the only
machines anyone ran.

### The two failure shapes were not the same, and the plan had to say so

The core list was a guaranteed silent no-op: cores 4 and up were never started, and nothing
reported it. The conduit was board-specific and untested, and a wrong one would more likely trap
than return a PSCI error.
Why that difference mattered is in [psci-conduit.md](isa-discovery/psci-conduit.md).

### What is read, and what is still assumed

Read: `/psci`'s `method` and `cpu_on`, and every `cpu@` node's `reg`, `status` and `enable-method`.
The decoding is in `crates/machine_discovery` (`cpu_list` for the roster, `aarch64::Psci` for the conduit), so it
is host-testable; the kernel halves are the reads and the refusals, exactly the split milestone 60
set up.

Still assumed, and now checked rather than assumed silently: that a core's hardware id equals its
logical id. Using a hardware id that differs would need three other things to move with it, and none
of them is in this milestone. `cpu::PERCPU`, the secondary stacks and the RISC-V trap stashes are
arrays indexed by logical id. The GICv2 targets an SPI by CPU-interface number, and `send_sgi` by
that same number. The PLIC's S-mode context is `2 * hart + 1`, and the SBI IPI mask is a bitmap of
hart ids. So `bring_up_secondaries` reads the real ids, compares each to its index, and refuses by name
the ones that differ:

```
  smp: cpu 2 has hardware id 0x100, which is not its index.
       This kernel indexes per-CPU state, interrupt targets and IPI masks by
       logical id, so it cannot use that core yet. Not started.
```

A clustered aarch64 board (`MPIDR_EL1.Aff1` in use) is exactly that machine. This turns a guaranteed
silent no-op into a named refusal and a smaller machine, which is the whole point. Making the
kernel actually *use* such a core is a milestone of its own, and it is a bigger one than this was.

### What the boot says now

```
  smp: psci over hvc, CPU_ON 0xc4000003 (the node's own id, from the device tree)
  smp: 4 core(s) in the device tree
  smp: 4 core(s) online
```

On RISC-V the first line reads `sbi hsm hart_start (the only mechanism RISC-V defines)`. The
asymmetry is real rather than a gap. RISC-V has one bring-up mechanism, no conduit to choose and no
function id to look up, because a kernel entered in S-mode has firmware under it by construction.

### The fixture that made the `smc` half testable

QEMU `virt` states `method = "hvc"`, but the same board with `-machine virt,virtualization=on`
states `smc`, so the conduit is not a property of aarch64, of QEMU, or of the `virt` board. A host
test compares the two dumps. It does not make the `smc` call tested: that configuration enters the
kernel at EL2 and this kernel expects EL1, so the half is parsed, not called. The fixture and the
reasoning are in [psci-conduit.md](isa-discovery/psci-conduit.md).

## BUGS

- Discovery does not make the kernel portable, it makes it honest. Knowing an extension is
  missing and doing something useful about it are different milestones. Today the kernel does
  exactly one thing with a missing requirement: it says so and stops.
- A core whose hardware id is not its logical id is refused, not used (milestone 100). See
  above; the reason is four other subsystems that index hardware by logical id.
- The `smc` conduit is parsed and never called. No machine on this laptop boots the
  configuration that would exercise it.
- `cpu_list` reads `/cpus/timebase-frequency` and falls back to the first hart's own. The RISC-V
  binding permits it per hart, and a machine whose harts genuinely differ would be misread. The
  kernel treats the counter as machine-wide regardless, so this is a limitation of the model rather
  than of the parse.
- A `status = "disabled"` CPU node is counted in the intersection. Firmware sometimes describes
  a core the OS will never run on, and including it narrows `common` further than it needs to be.
  That is the safe direction and it is not free: a board describing a disabled core with no FPU
  would make us report no FPU. Left alone until a real board shows the case.
- `VARange` reporting 52 on aarch64 does not mean the kernel could use 52. `ARMv8.2`-LVA needs a
  64 KiB granule, and `ARMv8.7`-LPA2 is a separate feature bit this record does not read. Reported
  because it is what the machine says; acting on it is a milestone, not a branch.
- `OpenSBI`'s implementation version prints as raw hex (`0x10007`). The encoding is
  implementation-defined, so decoding it as `1.7` would be a guess that happens to be right for one
  vendor, which is the sort of guess `implementer_name` deliberately refuses elsewhere.
- Little here has met a real board. Every ISA and CPU-list fixture is QEMU's or hand-written. That
  is the limitation the milestone exists to prepare for, not one it removes. The one fixture from a
  board is `crates/machine_discovery/tests/fixtures/visionfive2-uboot-control.dts`, the VisionFive
  2's U-Boot control tree's PLIC node, captured at the bench on 2026-08-21.

## See also

- [The CPU-model matrix](cpu-models.md), milestone 59, which found that zero call sites needed
  to branch across five QEMU CPU models and that QEMU reports 16 `satp.ASID` bits on every one of
  them, including `sifive-u54`. That is what made discovery worth building anyway: the one place a
  real chip may differ is the one place no emulator can tell us about.
- [ASIDs](address-space-identifiers.md) for what the ASID width is load-bearing for.
- [The counter rate in userspace](isa-discovery/counter-rate-in-userspace.md): how
  `user_mode_runtime::cntfrq` learns RISC-V's rate from a page the kernel maps, and the lesson.
- [The device tree](device-tree.md) for the parser this reads through.
- [The RISC-V port](riscv-port.md) for the SBI calls whose extensions are now probed.
