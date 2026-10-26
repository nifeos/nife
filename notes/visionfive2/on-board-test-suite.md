# The test suite on silicon, 2026-08-21

*Name: ratified 2026-10-18 (calef, "Approve all").*

An appendix to [notes/visionfive2.md](../visionfive2.md), beside its BUGS section. It records
boot 15 onward, when the `#[test_case]` suite first ran on the board.

## Boot 15 onward: six real hardware bugs in six boots

The on-board test-suite exit landed on 2026-08-21, and the `#[test_case]` suite ran on silicon for
the first time. It immediately found real hardware bugs no QEMU boot could have caught.

The UART pass/fail marker and SBI SRST shutdown worked exactly as designed. That was this
milestone's own remaining item, "the test suite where semihosting allows". Every failure below
printed `NIFE-TEST-EXIT: FAIL 1` and drove SBI SRST. OpenSBI accepted and attempted it. The board's
PMIC I2C read then failed completing the power-off, which is a firmware fact, not a kernel one.
There were six board boots, each fixing the failure the previous one found:

1. `PlicContexts::from_device_tree` read zero contexts against the board's real control DTB. The
   kernel's own `compatible = "sifive,plic-1.0.0"` match found nothing. The tree was captured at the
   bench via `fdt addr`/`save mmc` and read with `dtc`. The VisionFive 2's U-Boot-supplied tree
   names its PLIC `compatible = "riscv,plic0"` only. That is the older generic RISC-V PLIC binding,
   not the SiFive-specific string either JH7110 fixture in
   `crates/machine_discovery/tests/fixtures/` had assumed. It was fixed by trying both strings. A
   new host fixture holds the real board's tree, so this cannot regress silently:
   `crates/machine_discovery/tests/fixtures/visionfive2-uboot-control.dtb`, trimmed from the real
   42 KB capture. The S-context formula itself (hart h's context is 2h on this board) was already
   correct. Only the node-finding step was wrong.
2. Two `#[test_case]`s asserted `satp.ASID bits >= 8`, in `kernel/src/arch/riscv64/isa.rs` and
   `kernel/src/arch/riscv64/mmu.rs`. The U74 measures zero implemented bits
   (`satp.ASID 0 bits measured` in every boot summary since). RISC-V's WARL `satp.ASID` field
   permits that. The kernel's own `asid_tagging_is_trusted` mechanism (milestone 58 (RISC-V TLB shootdown, and the flush that makes ASIDs pointless)) already
   defends against it, by keeping the `sfence.vma` flush on a narrow machine. Both tests asserted
   the wrong invariant, a floor on the width. The right one is that the trust flag agrees with
   whatever the width actually is. They were fixed to check that, which holds on any width
   including zero.
3. A test asserted `Isa::described == Isa::harts`, named "QEMU virt describes every hart it has."
   `described` excludes disabled harts by its own doc comment, and the JH7110's S7 is exactly that
   hart: 5 `cpu@` nodes, `described == 4` by design. It was fixed to the invariant that actually
   holds on a heterogeneous machine: `described` is nonzero and no larger than `harts`.
4. Two MMU tests hardcoded `0x1_0000_0000` / `0x1_0100_0000` as "not RAM on QEMU virt." That was
   true when written. It is false on a board with 8 GiB of DRAM starting at `0x4000_0000`, where
   both addresses land inside the direct map. The test's own first `map_page` call failed with
   `AlreadyMapped` before either test's logic ran. It was fixed with a shared helper that computes
   an address past the top of every RAM region the device tree actually describes. So it is correct
   on any machine's memory map rather than a wider guess.
5. A test asserted RAM totaled exactly `256 * 1024 * 1024`, QEMU's runner-supplied `-m 256M`. The
   board's tree states 4 GiB (`reg = <0x0 0x40000000 0x1 0x0>`). That is distinct from the 8 GiB
   the U-Boot banner claims for the physical DRAM, a fact not yet chased further. It was fixed to a
   plausibility floor (16 MiB) instead of an exact QEMU literal.

## The fixture wall

Then the suite hit a different kind of wall, correctly. `nvme.rs`'s end-to-end test expects a
synthetic NVMe controller `xtask` always attaches under QEMU (`NIFE_NVME`). The board's manual
U-Boot boot attaches nothing. Its own comment already says why this is not a bug: "the test flow
always attaches a controller... absence is a lost QEMU flag, not a machine without a disk."

A `grep` across `kernel/src/` for the same shape (`.expect("no virtio-... device", ...)`) found at
least six more tests with the identical assumption. They span RNG (`credential_tests.rs`,
`disk_tests.rs`, `ntp_tests.rs`, `std_service.rs`), GPU (`display_tests.rs`), and the disk surveyor
programs. All are correct on QEMU and all are unreachable on bare silicon.
`helpers/qemu-runner-riscv64.sh` wires roughly forty `NIFE_*`-gated synthetic devices, and the
manual boot path has no equivalent for them.

This was the honest stopping point for "run the test suite on silicon" as then scoped. The
`#[test_case]` suite's design point is a QEMU machine loaded with synthetic fixtures, not a bare
board. Making it skip gracefully when a fixture is missing is a real mechanism to design, not a
bench fix. What does "this test needs hardware the current boot doesn't have" mean, and how does a
test declare it? It was recorded rather than chased further that night. Whoever picked it up next
was to scope that mechanism rather than patch the seventh `expect()`.

design/roadmap/0144-sandbox-screendump-gap.md holds a separate, still-open finding. The
*development sandbox*'s QEMU legs cannot reach the scanout/network referees at all. That is
unrelated to this board's fixture gap. It is about the host-side monitor connection, not about the
guest having no device.

## Milestone 145's skip mechanism, and two more board-only bugs

Milestone 145 (a test that needs hardware the boot doesn't have can say so) built the skip mechanism the section above called for. Re-running the suite under it
surfaced two more real, board-only bugs, in the same 2026-08-21 bench session. `skip!()` landed and
was applied to the fixture-dependent tests in `nvme.rs`, `pci.rs`, `disk_tests.rs`,
`display_tests.rs` and `entropy_tests.rs`. The run got 59 tests further before the next real
failure:

6. `kernel::sched::tests`'s two interrupt-delivery tests hardcoded `DELIVERY_IRQ = 10` /
   `PENDING_IRQ = 10`, QEMU virt's PLIC source for the console UART. The board's real source is 32.
   The boot banner's `uart irq : source 32 (device tree)` line has stated that since the UART-IRQ
   work. It was fixed by reading `user::uart_irq_and_source().0`, the DTB-driven value the rest of
   the kernel already uses, instead of a QEMU-only literal.
7. `Ns16550::enable_tx_interrupt` (test builds only) raced the transmitter on real hardware. The
   mechanism's own doc comment states its precondition correctly: "a 16550 asserts its line the
   moment `IER.ETBEI` is set while `LSR.THRE` is already set." QEMU's UART model has zero
   transmission latency. THRE reads back set the instant the previous write retires, so the
   precondition always holds there. Real 24 MHz serial hardware can have THRE genuinely clear for
   measurable time. The bench confirmed it: `LSR=0x0` immediately after this test's own diagnostic
   `println!` calls, which are still shifting out over the wire when the next instruction runs. And
   a real 16550's THRE interrupt is edge-triggered inside the chip. Setting `ETBEI` while THRE is 0
   only arms the interrupt for the *next* 0->1 transition. That may never come for a polling console
   with nothing queued to send. It was fixed by spinning on `LSR.THRE` before setting `ETBEI`. That
   is the same bounded pattern `init`'s busy-quirk drain already uses. The bench confirmed that
   `an_interrupt_becomes_a_message`, the test this function serves, now passes cleanly.

## A third finding, still open

`an_interrupt_that_arrives_before_the_wait_is_not_lost` still fails on the board after the THRE
fix. It is the second of the two interrupt-delivery tests, and shares IRQ 32 with the first by
design. `ROUTED_IRQS` never increments and `SPURIOUS_IRQS` stays at zero. So the interrupt never
reaches the trap handler at all, rather than arriving unrouted.

The bench confirmed the IER/LSR state right after `raise_test_irq` looks correct: `IER=0x2`, and
`LSR` showing THRE set. So the UART side of the mechanism is doing what it should. The next place to
look is the PLIC/hart affinity path, not the UART driver. That means `arch::irq::target_context`'s
"assign once, then reuse" cache, `s_context_of`, or which hart is actually executing the test when
the second call runs.

It was not chased further that session. Each round costs a full bench cycle: rebuild the board test
ELF, flash, reboot, transcribe. The two confirmed fixes above were worth landing on their own.
Whoever picks this up next should start by printing `crate::cpu::id()` and the assigned PLIC context
inside `target_context` itself, on the board, rather than guessing from IER/LSR alone. The earlier
data points at cross-hart delivery, not at the UART.
