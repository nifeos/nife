# Security audit, 2026-11-17: radon's device authority

**Kind:** security. **Lens:** the device authority the window gave radon's peripherals, read as a
holder that has turned hostile: the clock and reset server (`jh7110_clock_and_reset`), the GMAC's
control process (`designware_ethernet_control`) and the PMIC route
(`power_management_chip_route`). For each, what the kernel maps it and grants it, what that page
reaches beyond the device it serves, who can send to it, and what it trusts from its inputs.
**Findings:** fixed 3, minted 3, accepted 2.

The finding to carry off is the size of the clock and reset server's page. The MMU grants a page at
the smallest, and each JH7110 clock and reset domain keeps every clock word and every reset word in
its first page. So the server, which serves five devices, holds the resets of every CPU core, the
DRAM controller, the watchdog, the real-time clock and the power management unit, and it can gate
any clock in three domains. Only its code narrows a request to the badged device's bits. No record
said so. It is accepted, not minted, because the hardware offers no smaller grant and the server's
input is small: the kernel is its only client and stamps every badge. The record now says it in
notes/jh7110-clock-and-reset.md and SECURITY.md, and finding 2 mints the check that would show the
narrowing holds for every request rather than for bring up alone.

No confinement claim read was false. One claim in SECURITY.md was stale in its reason, and is
fixed (finding 8).

## Why this lens

The 2026-11-17 security audit of the verifier answered the first uncountable trigger yes: three
components arrived in its window holding device authority on radon, and it left them unread as the
next lens. The README's candidate list names them first. The 2026-10-20 audit read the SD/MMC and
Ethernet data planes; the control planes that milestone 1062 (the device control planes leave the
kernel) moved out of the kernel are what no audit has read.

### The two uncountable triggers, answered

Has a new component taken device or network authority? **Yes**, and these three are it; this report
reads them.

Has this booted on a new machine class? **No.** radon and xenon both booted before this window.

## What was read, and what came back clean

Every component was read at this lane's base, `9e8c189c0`. `designware_ethernet` is pinned in
`components/Cargo.toml` at `ba75ceba7bb7e27799bc28d2e46ad5ea7cb4b096`, and its bring up was read at
that commit from cargo's checkout. `designware_inter_integrated_circuit` and both
`jh7110_clock_and_reset` crates are in the tree.

**Who can reach the clock and reset server.** `kernel/src/user/jh7110_clock_and_reset_service.rs`
gives the server `READ` on its request endpoint and keeps the send side in a kernel static. A grep
of `components/src`, `crates/system_initializer` and `crates/grant_contract` finds no program that
names the endpoint. The kernel calls it through `sched::ipc_call_badged`, from the entropy, GMAC,
both SD/MMC and PMIC starters and the storage bench, every one a boot-thread path. The badge is the
device's register base, written by the kernel.

**What the server does with a request.** `answer` refuses an unknown badge, then an unknown
operation, then a malformed request, before it reads or stores anything
(`a_reply_refuses_in_the_wires_order_before_anything_is_touched`). Bring up only enables clocks and
releases resets; nothing in its code gates or asserts, as §304 (who may turn a device off) rules.
Reset stores are read-modify-writes that keep the word's other 31 lines
(`a_reset_store_keeps_the_other_31_lines_of_its_word`). `SET_RATE` writes the badged device's rated
clock word alone. Asked for 0 Hz it is refused unless the parent rate is below the largest
divider's reach, and `sd_card_divider_for` answers the largest divider for 0 rather than dividing
by it. The SYS syscon is mapped read-only (`Flags::user_device_read_only`), and the server's `write`
asserts it never stores there.

**What the domains' pages hold.** The identifiers were checked against the Linux 7.0 headers
`dt-bindings/clock/starfive,jh7110-crg.h` and `dt-bindings/reset/starfive,jh7110-crg.h`, and the
offsets against the crate's `Domain` constants: SYS has 190 clocks and 126 resets, its reset status
words end at `0x318`, inside `USED_WINDOW_BYTES` (`0x1000`), and a compile-time assertion holds every
domain to it. In SYS, clocks 0 and 1 are the CPU root and core, 43 the DDR bus, 122 and 123 the
watchdog's; resets 5 to 9 are the CPU cores, 29 and 38 to 40 the DRAM path, 109 and 110 the
watchdog's. In AON, clocks 10 to 13 are the real-time clock's and resets 3 and 4 the power
management unit's. This is finding 1.

**The GMAC's control process.** It holds the GMAC's first two pages, the AON syscon page, the DMA
region and a report endpoint, and it exits after one report. The kernel checks the report's
indices, `ControlReport::from_words` and that the link agrees with the account, before it starts
`net_stack`. In the pinned crate the inputs a link partner can influence are the PHY's
autonegotiation words, and `link_from_registers` decodes them into a three-value `Speed`; nothing
from the wire indexes memory. The PHY is refused unless its identifier is the YT8531. The two
authorities a hostile control process would use, DMA aimed anywhere (the board has no IOMMU) and the
other blocks' words on the syscon page, are already in `components/src/designware_ethernet_control.rs`'s
`BUGS`. `PROVEN_ON_SILICON` is false in `kernel/src/user/designware_ethernet_service.rs`, so
`device_starters` touches nothing on the booted system. Nothing new here.

**The PMIC route's inputs.** Its target (chip, register, bit) comes from the kernel's spawn word,
and `Target::unpack` refuses a chip past 7 bits, a bit past 7 and any high bit. Its wire answers
`RESET` and nothing else. `crates/system_initializer`'s spawn loop endows `WRITE` on the route at
`grant_contract::POWER_MANAGEMENT_CHIP_ROUTE_SLOT` to a program that declares `reboot`, beside the
reboot object, and to nothing else. Such a program can already reset the board.

**The two writers of the SYS reset word.** The kernel's own walk in
`kernel/src/drivers/jh7110_clock_and_reset.rs` still runs, for the soak's watchdog alone
(`cfg(all(soak_watchdog, riscv64))`). The soak runs from `run_measurement_boot` on the boot thread,
after the tour, and every caller of the server runs on that thread too, so the two never store at
once. The comment's argument was out of date, not its conclusion (finding 7).

## Findings

### 1. ACCEPTED: the clock and reset server holds the whole of three domains

What the page reaches is listed above: every CPU core's reset, the DRAM path, the watchdog, the
real-time clock and the power management unit, in a process that serves the TRNG, one GMAC port,
two SD/MMC controllers and I2C5. A server that turned hostile could stop the machine, hold DRAM in
reset or hold the watchdog that milestone 593 (a wedged kernel resets itself) relies on. A bug in a
device's plan could do the same by releasing or enabling the wrong identifier.

The hardware offers no smaller grant, and §256 (a server that keeps windows for many clients scopes
each by the caller's badge) chose this shape so that no driver holds the window. The server's
input is the narrowest in the tree: two words from the kernel with a kernel-stamped badge, and no
program can send to it. Accepted, and recorded in notes/jh7110-clock-and-reset.md's `BUGS` and in
SECURITY.md. The `BUGS` entry also says that a milestone handing a badge to a userspace driver
widens the input to that driver. Severity medium for what it reaches, with no reach from any
program today.

### 2. MINTED: the narrowing is checked for bring up alone

`one_device_never_touches_another_devices_bits` drives `Device::bring_up` for each device from one
starting state of the fake windows and checks every store. It does not drive `answer`, `SET_RATE`,
refused requests or other starting states. Since the server's code is all that confines finding 1,
that code is the thing to check for every request. Severity low. Proposed as
[the clock and reset server is checked to change only its device's bits](../roadmap/proposals/the-clock-and-reset-server-is-checked-to-change-only-its-devices-bits.md).

### 3. ACCEPTED: the PMIC route holds the whole I2C controller

The route maps the first `0x100` bytes of I2C5's page, the whole controller. With it a hostile route
could address any chip on the bus and any register of the power controller, not only its reset bit.
From memory, and not checked here: the AXP15060's other registers set its regulators' output
voltages, so that reach includes the SoC's supplies. The route's input is one word from one program,
which can only ask for the reset that program could already perform. Accepted, and recorded in
notes/board-reboot.md's `BUGS` and in SECURITY.md.

### 4. MINTED: the PMIC route ignores reserved request bits

`power_management_chip_route::operation` reads word 0's top eight bits; the other 56 bits and word 1
are ignored, so a `RESET` with garbage in them resets the board. `components/src/reboot.rs` sends
none, so nothing misbehaves. The clock and reset wire and the entropy wire refuse reserved bits
before acting, and a route built before an argument-carrying operation would misread one. Severity
low. Proposed as
[the PMIC route refuses reserved request bits](../roadmap/proposals/the-power-management-chip-route-refuses-reserved-request-bits.md).

### 5. MINTED: §320's example names a client that does not exist

§320 (the JH7110 clock and reset wire)'s concrete example has the progenitor start the server and
badge its endpoint. The kernel does both, and no program holds the endpoint. A reader judging the
server's exposure from the decision would place a userspace client on it. The section is an
architect's, so this lane proposes the edit. Severity low (records). Proposed as
[§320's example names its client](../roadmap/proposals/the-clock-and-reset-wires-example-names-its-client.md).

### 6. FIXED: the PMIC route's endpoint slot

`kernel/src/user/power_management_chip_route_service.rs` and notes/board-reboot.md said the
progenitor holds the route's send side at slot 35. `progenitor_boot` inserts it at 41
(`WRITE | GRANT`), and 35 is the self-test page. Both now say 41.

### 7. FIXED: the kernel walk's single-writer argument

The `SAFETY` comment on the kernel walk's reset read-modify-write said nothing else writes the SYS
reset word. The server writes it. The comment now names both writers and why they cannot interleave.

### 8. FIXED: why the booted system starts no SD/MMC server

SECURITY.md said the booted system starts neither the SD/MMC nor the Ethernet server until a bench
boot proves it. The SD/MMC server's `PROVEN_ON_SILICON` is true, since a bench boot proved it, and
`components/src/designware_mobile_storage.rs`'s `BUGS` still said it had never run on silicon,
while notes/designware-mobile-storage.md said the flag keeps the booted system from starting it.
What keeps it off is that no booted path calls its `start` (§260 (radon's block server serves a USB
drive)). The conclusion held; the three reasons now say why.

## Fatal-risk implications

Risk 7 (the confinement claim): on the JH7110 the clock and reset server and the PMIC route are
trusted for the whole machine, as the SD/MMC and Ethernet servers are trusted for DMA. Neither is a
confinement hole a program can reach today. Both mean that on this board a hostile driver process is
contained by its code, not by the kernel, and the record now says so where a reader meets it.

## What was deliberately not examined

- The `designware_inter_integrated_circuit` crate's transfer sequence and its timing arithmetic,
  beyond the route's use of it.
- The rest of the pinned `designware_ethernet` crate: its data plane, descriptor handling and the
  coherence probe's DMA. The 2026-10-20 audit read the data plane.
- What lives on the AON syscon page beyond the interface-select word. The control process's `BUGS`
  already records that it holds the other blocks' words; no source for the page's full map was read.
- The kernel's device-tree discovery of each window (`memory::initialize`), beyond the constants the
  spawners use.
- Nothing ran on radon. Every finding was checked by reading code, a grep or the Linux headers.

## What wants a lane of its own

The three proposals of findings 2, 4 and 5, finding 2 first. Then the graphical drivers' parsers, the
README's remaining candidate.
