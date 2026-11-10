---
status: DECIDED
raised: 2026-11-10
decided: 2026-11-10
ratified_by: calef
---

# 320. The JH7110 clock and reset wire: device-level verbs, and the badge is the device

*(Section number provisional until the merge queue lands it. Every name below is provisional under
[design/naming.md](../naming.md); the wire is not.)*

## Background

The JH7110 powers most of its blocks off at reset. Before a driver can touch the TRNG, the GMAC, an
SD/MMC controller or the I2C bus to the power chip, something sets an enable bit in that block's
clock words and clears its bit in a reset word. Those words sit in shared 64 KiB windows, one per
domain: STG at `0x1023_0000`, SYS at `0x1302_0000` and AON at `0x1700_0000`. Reset bits pack 32 to
a word, so releasing one block's reset is a read-modify-write of a word that holds 31 others.

Today the kernel does it. `kernel/src/drivers/jh7110_clock_and_reset.rs` (193 lines, 120 of code)
walks plans that `crates/jh7110_clock_and_reset` supplies, for five callers: the entropy starter,
the GMAC bring-up, the SD/MMC starter and its card clock, the reboot path's I2C5, and the soak's
watchdog. Nothing in the tree turns a clock off or asserts a reset. §304 (who may turn a device off)
keeps it so until the power cost is measured.

Milestone 1062 (the device control planes leave the kernel) moves this to a userspace server whose
verbs live in a contract crate both sides use (§311 (a contract is code, not prose)). Its steps 3
and 5 depend on that server. The wire must fit three fixed facts. A `CALL` carries two words and its
reply two (§12 (call and reply IPC)). The server learns the caller by a badge the kernel stamps
once (§230 (badged endpoint capabilities)). And a server scopes each client by that badge, never by
an id the client wrote (§256 (a server scopes each client by the caller's badge)).

## The decision

**Device-level verbs, and the badge is the device.** The starter that grants a driver its device's
register pages also mints a badged copy of the server's endpoint. The badge is that device's
register base physical address, as the device tree's `reg` gives it: `0x1603_0000` for the GMAC's
first port, `0x1600_c000` for the TRNG. The server maps the badge to that device's clocks and
resets and to nothing else. No request word names a device, so §256 holds by construction.

The wire is interface id 24, version 1, with no magic. A magic marks offset 0 of a shared page, and
this contract has no page; `entropy_contract` and `socket_contract` carry none either.

The opcode sits in bits 63:56 of request word 0, where `entropy_contract::OPERATION_SHIFT` puts it.

| opcode | verb | arguments | reply word 1 |
|---|---|---|---|
| 0 | version query, reserved | none | the served versions, as milestone 1000 (message contracts carry bind-time version checks) defines them |
| 1 | bring up | none | the bring-up summary |
| 2 | read clock | word 0 bits 7:0, the device's own clock index | that clock's raw word |
| 3 | read reset | word 0 bits 7:0, the device's own reset index | that reset's raw status word |
| 4 | set rate | word 1, the most hertz the device may run at | the hertz it now runs at |

Opcodes 5 to 255 are free. No verb turns a device off.

Bring up walks the device's clocks, each parent choice before its clock's enable, then its
resets, in the crate's plan order. It is idempotent, since firmware may have started the block, so
a client whose call failed simply calls again. Set rate serves the one rated clock a caller sets
today, the SD/MMC card clock; a device with none is refused.

Reply word 0 is a status, a small non-negative number. A kernel failure of the `CALL` itself is a
small negative, enormous as a `u64`, so the two never collide (`entropy_contract::delivered`'s
reasoning).

| status | meaning |
|---|---|
| 0 | done |
| 1 | the badge names no device this server serves, badge 0 included |
| 2 | an opcode this server does not serve |
| 3 | an enable bit did not read back set |
| 4 | a reset did not read released within the poll bound |
| 5 | a read named a clock or reset beyond the device's own |
| 6 | set rate on a device with no rated clock |
| 7 | no divider gives a rate at or below the one asked |

For statuses 3 and 4 the server still sends the summary, so the failure is diagnosable.

The bring-up summary, from bit 0: bits 7:0 count the clocks the plan enables, 15:8 those reading
back as running, 23:16 the resets in the plan, 31:24 those released. Bit 32 says the device was
already up before the call. Bits 47:40 count steps the server refused. The rest is zero.

Why A. It has the fewest moving parts. The order the silicon needs stays in one place, the server,
where host tests already pin it. It needs no refcount, since nothing is released twice or turned
off, and it obeys §304 by having no off verb. And it is the restrictive choice. nife means its
drivers to be portable across many chips, so line-level verbs may come later. They arrive as new
opcodes with no version bump, while taking one back would be a break.

## The options refused

- **B, line-level verbs by the device tree's `clock-names` and `reset-names`.** Every client would
  carry the order the silicon needs, and any later off verb would need a per-line refcount. Refused
  for today; it can be added as new opcodes.
- **C, a plan the client uploads.** A list of steps does not fit two words, so it needs a shared
  page and a magic, and the server would check the order it was handed anyway.

## Prior art

- **OpenBSD, read** (`sys/dev/ofw/ofw_clock.c`, revision 1.11). The kernel's FDT drivers call
  `clock_enable(node, name)`, `clock_set_frequency`, `reset_deassert(node, name)` and
  `clock_set_assigned(node)`, resolving each name through the node's `clock-names` or
  `reset-names`. Whole-device forms exist too (`clock_enable_all`, `reset_deassert_all`). There is
  no refcount. Each driver carries its own order: `dwqe_fdt_attach` enables two clocks, deasserts
  both resets, then enables two more. That is B's shape, run in the kernel and trusted.
- **FreeBSD extres, recalled.** `clk_get_by_ofw_name`, `clk_enable` and `hwreset_get_by_ofw_name`
  name a line by the node and its name, with enables counted up the parent tree. Also B's shape,
  in the kernel.
- **NetBSD fdtbus, recalled.** `fdtbus_clock_get` and `fdtbus_reset_get` by name, then
  `clk_enable` and `fdtbus_reset_deassert`. B's shape again.
- **Genode's platform driver, recalled.** One userspace process owns the devices. A client acquires
  a device by name as its policy allows, and the platform driver switches on that device's power,
  resets and clocks as its description lists them. The client calls no clock verb. This is A's
  shape and the closest precedent.

Where nife differs: the verbs cross a process boundary; the device is named by the kernel-stamped
badge, never by a client-written id; the order is the server's, where the BSDs leave it to each
driver; and nothing turns off, so nothing is counted.

A concrete example. The progenitor starts the server with the STG, SYS and AON windows. To start
the GMAC's control process it badges the server's endpoint with `0x1603_0000` and calls opcode 1.
The server enables SYS clocks 108, 111 and 109 and AON clocks 3 and 2, chooses parent 1 for AON
clock 5 while it is gated, enables clock 5, and releases AON resets 0 and 1. It replies status 0
with a summary of six clocks enabled and running and two resets released. A read of clock 9 on that
capability answers status 5, and nothing it can send reaches the TRNG's clocks in STG.

## What is one-way

Once a peer outside the tree builds against the wire, these hold for good:

- interface id 24, and what version 1 means;
- the opcode numbers, opcode 0's reservation for the version query, and where each argument sits;
- the status codes, and the layout of reply word 1 for each opcode, the summary included;
- that the badge is the device's register base physical address, the agreement between every
  starter and the server; a second scheme would give one badge two live meanings.

That no verb turns a device off is not one-way: an off verb is a new opcode, and a version 1
client never relied on its absence.

Two-way: every name, the crate's and the program's included; whether the starter or the driver
holds the capability; whether the server takes a device's plan from the crate or the tree; the
readiness word, the poll bound and the restart policy; and new opcodes.
