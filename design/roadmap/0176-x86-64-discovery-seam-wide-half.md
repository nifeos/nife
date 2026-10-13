---
status: BUILT
raised: 2026-08-25
built: 2026-08-25
---
# 176. The x86_64 discovery seam's wide half: COM1's IRQ and a CMOS RTC

This is the wide half of milestone 161 (the x86_64 kernel port)'s item 0. Piece 1 is built (below).
Piece 2 needed a decision, ratified 2026-08-26:
[DECISIONS §130 (how the kernel-resident CMOS RTC reaches the userspace clock service)](../decisions/0130-cmos-rtc-delegation.md).
It is built against it (below).

## The four device windows, and which two are this milestone's

The x86_64 discovery seam has four device windows that a tree supplies on the other architectures.
They are the interrupt controller, the RTC, the UART's interrupt line, and the PCIe ECAM range.
**Two of those four are built elsewhere:**

- **The interrupt controller (IO APIC) is built**, milestone 161's own item 2, reached directly by
  `arch::x86_64::irq` rather than through `memory.rs`'s statics.
- **PCI is wired**, and has been since milestone 165's ACPI/MCFG work: `kernel/src/main.rs`
  calls `memory::record_pci_regions` from the ACPI `ecam` path, so `memory::pci_regions()` returns
  `Some` on x86_64.

**Two are real**, and are this milestone's actual scope:

## Built: piece 1, COM1's IRQ wired, 2026-08-25

`Acpi::isa_irqs` already resolved all sixteen legacy ISA IRQs at boot; `isa_irqs[4]` is the console
UART's line. `kernel/src/main.rs`'s boot tour now reads it (`acpi.isa_irqs[4].gsi`) and hands it to
a new `memory::record_uart_irq`, the same-shaped setter `memory::record_pci_regions` already was for
PCI: it fills `memory::UART_IRQ`, the exact static the other two architectures fill from their
device tree at `memory.rs` line 135. `user::uart_irq_and_source()` (already generic across all three
architectures) now reports a real answer on `x86_64` instead of always falling back to the
`UART_RX_INTID` constant; its "device tree" source string, and `memory::uart_irq()`'s matching doc
comment, were generalized to "machine description" since ACPI is now a second source feeding the
same seam. Nothing calls it yet (the x86 console is polled, as the milestone that raised this item
already said), so this is exactly what it was scoped as: filling a seam for a future reader to find
real, rather than routing an interrupt.

`kernel/src/arch/x86_64/machine.rs`'s stale `BUGS` comment was also corrected: PCI has been wired
since milestone 165 and the interrupt controller since milestone 161 item 2, neither is `None`
today, and the `BUGS` section now names only the CMOS RTC (piece 2, below) as the device window
with no seam at all.

## Built: piece 2, a CMOS RTC read by the kernel, 2026-08-25

Sizing this found a real design fork, not a shape to build: the other two architectures' RTCs are
memory-mapped, so their `RTC_REGION` seam is a physical page a driver maps and pokes directly. The
PC-compatible CMOS RTC is two fixed I/O ports, and [DECISIONS §121](../decisions/0121-port-io-capability.md)'s
current (still-PROPOSED) recommendation keeps all x86 legacy port I/O kernel-resident, so the
existing "map the device, let userspace drive it" pattern has no CMOS equivalent. Full writeup, four
options considered, and the decision itself in
[DECISIONS §130](../decisions/0130-cmos-rtc-delegation.md): option 3, the kernel reads CMOS once
at boot and hands the seed to the clock service as a `Spawn` argument, the same way `kind` already
crosses that boundary.

Built exactly against that decision. `kernel/src/arch/x86_64/rtc.rs` (new) is the kernel-side
reader: a dozen `in8`/`out8` pairs against ports `0x70`/`0x71`, the same shape as
`arch::x86_64::timer`'s PIT calibration, with the CMOS update-in-progress flag polled and a
double-read-and-compare so a read never lands mid-update, BCD-to-binary and 12h/24h decoded per
status register B, and the six raw fields turned into a Unix timestamp through the `calendar` crate
this tree already depends on rather than a second copy of that arithmetic.
`clock_proto::rtc::CMOS` (provisional; a new kind, `= 3`) is the wire-format addition: unlike
`PL031`/`GOLDFISH`, which name a register layout the service polls itself, `CMOS`'s reading has
already been taken by the kernel and arrives as data on the wall clock's second `Spawn` argument
(`arg1`), which `clock_service::start` fills only on `x86_64` (`rtc_region()` stays `None` there
forever; §121 forecloses the alternative). `components/src/clock.rs`'s `read_rtc` takes the new kind as
`Some(seed)`, no register, no base address.

`date_tests.rs`, `time_tests.rs`, `clock_tests.rs` and `ntp_tests.rs` all run on `x86_64` now
instead of skipping via `clock_service::machine_has_no_rtc()`, which itself now asks the kernel's
CMOS reader on this architecture rather than the device tree. Four of `ntp_tests.rs`'s six tests
still skip there, but for an unrelated, pre-existing reason found while wiring this up: the test
runner attaches no virtio-rng function on any bus for `x86_64` yet (notes/x86-port.md), so the NTP
client correctly refuses to reach the network without a nonce source, the same refusal
`without_entropy_the_client_refuses_rather_than_guessing` tests on purpose. `ntp_tests.rs` gained a
`machine_has_no_entropy` guard, the same convention `disk_tests`, `credential_tests` and
`entropy_tests` already use for that cause, so the four skip with a reason rather than panic; giving
`x86_64` a network entropy device is its own, unstarted piece of work.

Piece 1 is complete and independent of this.

## Follow-on

- **Decision.** `design/decisions/0121-port-io-capability.md` holds the question that decided piece
  2's shape, whether userspace may ever drive x86 legacy port I/O. Its recommendation keeps port I/O
  kernel-resident and is still PROPOSED; `design/decisions/0130-cmos-rtc-delegation.md` took the RTC
  question against it and is ratified, which is why `rtc_region()` stays `None` on x86_64 forever.
- **Recorded.** `crates/clock_protocol/src/lib.rs` (spelled clock_proto when this was written;
  milestone 265 renamed it) carries the new `rtc` kind, shipped as a provisional
  name like every name a lane mints. `CMOS` is unlike its siblings in kind: `PL031` and `GOLDFISH`
  name a register layout the clock service polls itself, while `CMOS` names a reading the kernel has
  already taken and passes to the service as data on its second `Spawn` argument.
- **Recorded.** `kernel/src/arch/x86_64/machine.rs`'s BUGS section had claimed for two milestones
  that the interrupt controller and the PCIe ECAM range were still `None` on x86_64, after
  milestones 161 and 165 had wired both. Corrected here, and it now names only the device windows
  that are genuinely absent.
- **Milestone 362.** `design/roadmap/0362-virtio-rng-on-the-x86-64-runner.md`, attach a virtio-rng
  function to the x86_64 test runner and wire it, so the NTP client has a nonce source there; four
  of `ntp_tests.rs`'s six tests skip on x86_64 without one. That block is SUPERSEDED, on its own
  instruction: the larger x86_64 fixture lane it asked to be folded into is numbered now and names
  the RNG among its devices, so the work is there rather than in two places.

## Index row

Milestone 161's own item 0, re-scoped fresh: two of the four windows its text named (the interrupt
controller, PCI) turned out to already be built, checked directly against the tree. Piece 1
built (2026-08-25): COM1's already-discovered IRQ (`Acpi::isa_irqs[4]`) wired into `memory::UART_IRQ`, and `machine.rs`'s stale `BUGS` comment corrected. Piece 2, a CMOS RTC,
built (2026-08-25) against [DECISIONS §130](../decisions/0130-cmos-rtc-delegation.md): the kernel
reads CMOS once (`arch::x86_64::rtc`) and hands the wall-clock seed to the clock service as a `Spawn` argument (`clock_proto::rtc::CMOS`), since every existing RTC consumer assumed the
userspace clock service reads its device itself and DECISIONS §121 forever forecloses that shape
for CMOS's kernel-resident ports. `date_tests.rs`, `time_tests.rs`, `clock_tests.rs` and `ntp_tests.rs` now run on `x86_64` instead of skipping.
