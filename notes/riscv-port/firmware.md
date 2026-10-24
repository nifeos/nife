# riscv64 and its firmware: discovery, SBI, exit and reboot

*Name: provisional, minted 2026-10-23 (UTC) by milestone 929's riscv64 lane.*

An appendix to [notes/riscv-port.md](../riscv-port.md). It carries the design behind
`kernel/src/arch/riscv64/isa.rs`, `sbi.rs` and `semihosting.rs`: how the kernel asks the machine
and its firmware what they are, and how it asks firmware to stop or restart it.

## Discovery, once, at boot

The decoding lives in `crates/machine_discovery`'s `riscv64` module, which is host-tested. What
only a boot can do is in `isa.rs`, from three sources in the order they arrive:

1. The device tree, which is firmware's claim about the silicon.
2. SBI, which is firmware's claim about itself. `probe_extension` is the only way to know whether
   the `ecall`s the kernel makes unconditionally will do anything at all.
3. The `satp.ASID` probe, a measurement, which `mmu::init` records later because it needs the real
   page tables installed.

`kernel_main` runs `isa::init` after `memory::init`, which reads the device tree first, and before
`mmu::init`, which would build tables a refused machine cannot walk. The summary prints once paging
is on, before the test, shell or bench branches, so every boot mode reports the machine.

A machine missing something the kernel needs is refused with a panic that prints what is missing.
A kernel that assumed Sv39 on an Sv32 machine, or called SBI RFENCE on firmware without it, would
run degraded and report success. Silence is not a refusal, though. A tree that does not describe
its ISA, or firmware too old to answer, is reported as unknown, and the boot continues, because the
kernel is demonstrably executing by then.

The record is `None` until `init` runs, so an early read panics rather than returning a plausible
all-zero machine, the discipline `mmu::asid_bits` follows too.

## Asking SBI

Every SBI request goes through `sbi::call`, one `ecall` with the extension in `a7`, the function in
`a6` and six arguments in `a0` to `a5`. A function taking fewer ignores the rest (the
specification's "Binary Encoding"), so the unused ones are zero rather than whatever the registers
held. Firmware preserves every register except `a0` and `a1`, and M-mode runs on its own stack, so
`nostack` holds. One helper states the calling convention once and gives a proof one function to
stub.

The base extension is itself what might be absent: SBI v0.1 predates it. Its getters cannot fail,
and `probe_extension` reports absence as a zero value, so `isa.rs` discards their error word. A
firmware without the base extension returns an error and leaves `a1` alone, so a zero spec version
means "did not answer" and nothing after it is trusted. The spec version's minor number is the low
24 bits and its major the seven above, so QEMU's `0x0300_0000` is SBI 3.0. The SBI PMU calls can all
fail, so `pmu.rs` keeps the error ([notes/riscv-cycle-counters.md](../riscv-cycle-counters.md)).

## T-Head harts

The TH1520 build chooses `paging::Sv39Mae` at compile time ([memory.md](memory.md)), so
`probe_thead` checks the build and the hart agree. `mvendorid` comes from SBI (base function 4),
because S-mode cannot read it. On a T-Head hart, `th.sxstatus.MAEE` says whether firmware turned
`XTheadMae` on. A disagreement is fatal either way. With MAEE on and plain Sv39, every leaf names
memory type zero, which T-Head does not define. With MAEE off and `Sv39Mae`, bits 63:59 are
reserved, and a set one is a page fault on the first walk of the fine tables.

Linux keys the same errata on `mvendorid` with `marchid` and `mimpid` zero. This kernel reads the
CSR instead, because the CSR is the hart's own answer. `is_thead` also tells `fp::init` to close
`XTheadVector`'s own `VS` field, and is false before discovery, the safe default.

## The test harness's exit

The module keeps aarch64's name, `arch::semihosting::exit`, because that is the contract the
harness calls, but the mechanism is QEMU `virt`'s `sifive_test` finisher, an MMIO word at
`0x10_0000`. Writing `PASS` exits 0; writing `FAIL | (code << 16)` exits nonzero. It is reached
through the direct map, where `mmu::map_everything` maps it device-typed.

Under the `board` feature the finisher does not exist, and a store there is a bus error. The board
exit prints `NIFE-TEST-EXIT: PASS` or `NIFE-TEST-EXIT: FAIL <code>` for a harness on the serial
line, drains the console, then asks SBI SRST to shut down
([notes/visionfive2.md](../visionfive2.md), "The test suite where semihosting allows").

## Cold reboot

The SRST reboot lives in the same file. SRST is one function with a type argument, and shutdown
and cold reboot differ only by that argument; two files would repeat the extension id, function id
and calling convention for one `ecall`. SBI v0.3 defines type 0 as shutdown, 1 as cold reboot and 2
as warm reboot. An implementation may support any subset and refuses the rest with
`SBI_ERR_NOT_SUPPORTED` (-2), rather than resetting the wrong way. A successful reset never returns,
so a return is the firmware saying no.

`arch::reboot(marker)` is the same contract on all three architectures: print one line per route,
prefixed with `marker`, before trying it, and return only when every route was refused. Here there
is one route. The line comes first because once a reset begins the UART stops draining, and
`console::drain` runs before the `ecall`, because printing first is not sending first. On radon the
PMIC's power cut took the undrained tail of these lines. The refusal prints after, with the
firmware's own `sbiret.error`, so a firmware that says no is read off a console rather than
assumed. The board exit drains its marker for the same reason.

The callers are the rebooting soak (`soak::watch`, behind `--features reboot_soak_test`, reached
only after the escape in `console::is_byte_waiting` has been checked twice) and the reboot object's
method, `kernel::reboot::restart`, in every build. Under QEMU `virt` OpenSBI implements type 1
through the `sifive_test` device, and `script/soak-test --reboot --arch riscv64` proves it. radon's
OpenSBI accepts type 1, and then its reset fails in the PMIC write. That is recorded in
milestone 249 (the boot lottery is sampled by a person walking to the board),
`design/roadmap/0249-the-boot-lottery-is-sampled-by-a-person-walking-to-the-board.md`, and in
[notes/soak/rebooting-soak.md](../soak/rebooting-soak.md), "Verifying the reset before anything is
left unattended".

## BUGS

- Whether a C910 permits the S-mode read of `th.sxstatus` is the T-Head manual's claim and QEMU's
  model; no silicon here has shown it.
