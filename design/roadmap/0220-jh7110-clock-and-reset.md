---
status: BUILT
raised: 2026-09-01
built: 2026-09-04
---
# 220. This kernel drives no clock or reset controller, and the first real device will need one

Milestone 159 (a real hardware entropy source: the JH7110's TRNG) names this as the most likely
reason a first bench boot of its driver fails.

two lines off a serial console. See "The bench settled the premise" below.

**Status deliberately did not move on 2026-09-04, and this block's last three sections are that
lane's report of why**. The shape is that of milestone 218 (every boot of the VisionFive 2 needs a
human typing four commands into U-Boot), for work that is written and has never run on the
board. Milestone 159 chose the same shape for the same reason on its own first two passes.
The
driver, its host tests, the absence-path kernel test and the bench procedure all landed;
**QEMU's riscv64 `virt` machine has no clock or reset controller of any kind**, so no phase of
this runs end to end in an emulator. `PARTIAL` names a phase that does. `NOT-STARTED` is not right
either, and it is less right than it was. It is still the closer of the two words this vocabulary
has, and the gap is written out here rather than folded into a token that would overstate it.

**In brief.** nife has never programmed a clock or a reset line. Every device it has driven came up
already running: QEMU's virtio devices, the PL011 and NS16550 consoles, the PLIC. Real SoC
peripherals do not, and the JH7110 is the first case in reach.

Linux's own JH7110 TRNG driver takes two clocks and a reset line before it touches a register.
nife's driver takes neither, because nothing in this tree can supply them. So the most likely
outcome of milestone 159's first bench boot is not a driver bug: it is a register window that reads
as nothing because the device is held in reset or its clock is gated.

## Why it is minted before it is known to be needed

Milestone 159's lane deliberately did **not** build this on spec, which was right: a driver written
against a datasheet for a device nobody has seen answer is exactly the shape this project's
`HARDWARE` gate exists to defer. What it did instead is make the question answerable in one boot.
Its `hw entropy` line carries a raw `(STAT << 32) | ISTAT` bring-up diagnostic, and that number
separates the two failures a bench session otherwise cannot tell apart:

- **All zeros**: the register window read as nothing. Gated clock, undeasserted reset, or a wrong
  base address. That is this milestone.
- Anything else: the device answered and the sequence is wrong. That is milestone 159's.

So this block exists so that a single observation at the bench routes to a milestone rather than to
a person's memory of a conversation.

## What it would need

A clock and reset generator driver for the JH7110, and a decision this block does not make about
where such a thing lives in a capability system. That is the interesting question and it is why
this is not simply a small driver:

- A clock controller is a shared resource that many drivers need, so it is a service rather than a
  library, and every device driver becomes its client.
- It is also authority: whoever can gate a clock can stop a device, and whoever can assert a reset
  can interrupt one mid-transaction. That is a capability worth naming carefully.
- The alternative is that the kernel does it during bring-up and no userspace program ever holds
  that authority, which is simpler and less general, and may be right for a demonstrator.

## The bench settled the premise, 2026-09-04

It is a measurement now. Two boots of radon, byte-identical:

```text
hw entropy  : FAILED: JH7110 TRNG at 0x1600c000 (tree says starfive,trng, status disabled):
              report 0x524e475550, bring-up diagnostic 0x0000000000000000,
              draws 0/0 bytes, first-all-zero true, draws-differ false
```

Transcript: `target/board/radon-2026-09-04-trng-bringup.log`. The diagnostic is the raw
`(STAT << 32) | ISTAT`, so the whole register file read back as zeros, which is the outcome
milestone 159's own table routes here rather than to a defect in that driver.

Two independent signals agree. The device tree marks the node `status disabled` and the
register window reads nothing. Milestone 239 (radon's device tree does not describe the TRNG, so a working driver never runs)
deliberately *reports* `status` without acting on it. That same tree calls the S7 monitor
core `okay` and gives it an MMU it does not have. Here the tree and the hardware say the same
thing, so the agreement is corroboration rather than circularity.

The mechanism that made this attributable was built before the answer was known, which is the
part worth keeping: 159's lane declined to write this driver on spec and instead made one boot
separate the two candidate failures. It cost a diagnostic word and it saved a bench session.

## What was built on 2026-09-04, and what it proves

- `crates/jh7110_clock_and_reset`, pure logic, 20 host tests. The STG domain's register arithmetic (one
  32-bit word per clock at `base + 4*index`, enable bit 31; 32 resets to a word at `0x74`, watched
  at `0x78`), the TRNG's three-step bring-up plan, and the device-tree query that finds the
  controller. No pointer is dereferenced anywhere in it.
- The identifiers, resolved from two published trees that disagree on every name. Mainline
  Linux calls them `JH7110_STGCLK_SEC_AHB` (15), `JH7110_STGCLK_SEC_MISC_AHB` (16) and
  `JH7110_STGRST_SEC_AHB` (3). The vendor U-Boot radon actually runs calls them
  `JH7110_SEC_HCLK` (205), `JH7110_SEC_MISCAHB_CLK` (206) and `RSTN_U0_SEC_TOP_HRESETN` (131),
  numbered flat across all five domains; rebased on their group starts (190 and 128) they are 15,
  16 and 3. Both name the same window at `0x1023_0000`. The rebase is a host test, not a
  paragraph, so an upstream renumbering fails the build instead of surprising somebody at the
  bench. Every URL and fetch date is in the crate header.
- `kernel/src/drivers/jh7110_clock_and_reset.rs`, the twenty lines that store to a register, and the
  argument for why they are in the kernel (below).
- A `hw clock` line in the riscv64 boot tour, printed before `hw entropy` because it decides
  how that line should be read, and reporting the *before* words as well as the after ones. That
  is the field that can refute this milestone: clocks already enabled would mean the TRNG's zeros
  have some other cause.
- **`notes/jh7110-clock-and-reset.md`**, the bench procedure, with a table mapping every
  observable outcome to which milestone it belongs to. Written the way `notes/x86-uefi-boot.md`
  was, by a lane that could not reach the machine.

What CI actually exercises is the absence path, and saying so is the point. QEMU's `virt`
board names no clock controller, so `memory::init` records no window, nothing is mapped, and
nothing is stored. `no_clock_window_is_mapped_where_there_is_no_jh7110` pins exactly that, because
the dangerous failure is silent in the other direction. `jh7110_crg::discover` deliberately never
fails to produce an address (falling back to the constant both trees agree on), and a caller that
took it unconditionally would store to `0x1023_0000` on every board this kernel boots.

## Where it lives, and why the kernel holds it

The block left this open as the interesting question. It is answered by reusing DECISIONS §86
(whether an NVMe driver can leave the kernel, and what capability would let it) rather than
inventing a second argument. The kernel keeps the admin plane, EL0 gets the data path, and no
new system call surface is added. Three things make a clock controller a stronger case for that
split than NVMe was:

- **Granting it would widen a driver's authority, not confine it.** One 64 KiB window holds the
  clocks and resets for USB, both PCIe root ports, the DMA engine and the security block.
  Milestone 159's whole demonstration is a driver holding one page of one device's registers and
  two endpoints; handing it this would trade the narrowest authority in the tree for the widest.
  The reset is documented **shared** upstream: the same line resets the PL080 DMA at
  `0x1600_8000`, which is why nothing here offers an assert.
- It is one-shot setup, off every hot path. The performance argument that makes EL0 attractive
  for a data path does not exist for three register writes at boot.
- Zero new system call surface, which is what makes it a decision a lane could take: nothing two
  programs agree on changed, so an EL0 clock service remains available to a later milestone that
  has evidence for it.

## Follow-on

- **Milestone 376.** Nothing turns a device back off: this tree can now enable a clock and release
  a reset and cannot do either in reverse, so a driver that dies leaves its device clocked forever.
  The mechanism is small and the authority question is not, which is why it carries a `DECISION`
  gate rather than a `BUGS` line alone.
  `design/roadmap/0376-nothing-turns-a-device-back-off.md`.
- **Recorded.** Parent clocks are not programmed. The STG domain's own bus clock comes from the
  SYSCRG at `0x1302_0000` and nothing here touches it; Linux's clock framework walks parents
  automatically and this does not, relying on firmware having left the bus clocks running. It is
  the first thing to suspect if the enable bits do not read back, and it is recorded beside the
  feature in `notes/jh7110-clock-and-reset.md`'s `BUGS` with the rest of the honest scope.
- **Recorded.** The deassert poll is an iteration count and not a duration, so a slower board
  scales the bound silently. Same `BUGS` section.
- **Done.** The bench line that produced this diagnosis also found that the entropy service sends
  `READY` while holding zeros, which was milestone 159's defect and not this one. Fixed on
  2026-09-04 by pull request #726 while this lane was running: `entropy_proto::readiness` now
  decides that word from the bytes rather than from the handshake, and all three backends ask.
  It mattered that it landed first: once the clock works the symptom disappears and the defect
  would not have.
- **Recorded.** A third instance of `script/fastpath-footprint`'s flat entry set moving under an
  inlining flip, and the first in the *shrinking* direction. Adding a dependency to the `kernel`
  crate made LLVM fold `system_call::dispatch` into the aarch64 exception handler, so the gate reported
  `system_call_entry` 35% smaller while the code a system call fetches was identical. Closed the way the
  tree already closes this, with `#[inline(never)]` and the reasoning beside it, and written up in
  `design/roadmap/0368-a-flat-entry-set-counts-bytes-no-system-call-fetches.md`, which owns the
  mechanism question.
- **Refused.** A general JH7110 clock driver covering all five domains and every clock. The
  milestone's own `BUGS` named unbounded scope as its main risk and the two ends differ by an
  order of magnitude. The arithmetic here is general enough that a second domain is a table entry
  rather than a rewrite, so building the other four before anything needs them would be work with
  no reader.
  This refusal is milestone 474 (design/roadmap/0474-a-general-jh7110-clock-driver.md), which carries
  it with the condition that would change it.
- **Recorded.** The crate name `jh7110_crg` is provisional and unratified, as milestone 159's
  `jh7110_trng` and `Bus::Jh7110` already are. It carries its two refusals in its own header,
  beside the name, and shows up on `script/names --unratified`, which is a worklist rather than a
  wall. An unratified name never blocks anyone's build, so this stays a standing state and not a
  thing to wait on.

## BUGS

- Nothing here has run on hardware. Every offset, bit position and identifier is transcribed
  from published source; what is proven is that two independently published trees agree with each
  other and that the arithmetic is right. An emulator cannot do better, because QEMU's `virt`
  machine has no clock or reset controller to model. The honest scope, and what each bench outcome
  would mean, is `notes/jh7110-clock-and-reset.md`.
- It says nothing about argon or xenon, which have their own bring-up assumptions that nothing
  has tested either.
- Scope is unbounded as written, and the 2026-09-04 pass took the small end deliberately: the
  STG domain only, and within it only the two clocks and one reset the TRNG needs. The other four
  domains and their hundreds of clocks are untouched and unmodelled.

## Index row

Linux's JH7110 TRNG driver takes two clocks and a reset line before touching a register; nife has
never programmed either. The premise stopped being a prediction on 2026-09-04: radon's TRNG
register file read back all zeros on two byte-identical boots, and the vendor tree independently
marks the node disabled. A driver was written 2026-09-04 and radon was powered off with no bench
session, so the token says what the outcome is rather than what was built. It holds the STG domain's
arithmetic, the TRNG's three-step plan resolved identically from mainline Linux and the vendor
U-Boot, a kernel-resident controller with §86's argument for why, and a boot-tour line whose *before* words can refute the whole premise. QEMU's `virt` has no clock controller, so CI
exercises only the absence path. The block carries the bench procedure and what each outcome would
mean
