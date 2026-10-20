# Security audit, 2026-10-20: the window the supply-chain audit left unread

**Kind:** security. **Lens:** the delta the 2026-10-19 supply-chain audit named and did not read,
read as a confined process trying to widen: the device authority the window minted (radon's SD/MMC
and Ethernet servers, e1000e, the xHCI keyboard driver), the ABI names added since 2026-10-03
followed into their handlers, and the six host packages the glyph atlas brought, from the package
inward to the tables the target draws. **Findings:** fixed 4, minted 3, accepted 2.

The finding to carry off is the oldest one. `Virtio::READ_REGISTER` added the caller's offset to
the device's slot base and read through the kernel's direct map with no bound, so any holder of a
`Virtio` capability could read any physical address on the mmio transport: the kernel, and every
other process. It predates this window (the same code stands at `996bb792a`), and no audit's lens
crossed it because the method looked like "read a status register". It is fixed here. The other
two kernel findings are races in the shared capability table that milestone 812 (`std::thread::spawn` runs real threads in one address space) made possible, and
both are minted rather than fixed, because each sits on a hot path and wants a measurement.

## Why this lens

`script/audits` on `main` at `dd50a277b` reported security DUE on one trigger: external packages
190 to 196 (+6, any change fires). Issue #2448 is the scheduled workflow's repeat row. Its five
reds from 2026-09-21 to 2026-10-19 were the tripwire working: every one of them ran before the
2026-10-19 audits landed (#2413 and #2439, both merged after the 2026-10-19 run), and on today's
tree the documentation kind is not due. The check read both new rows correctly, so nothing here is
a defect in `script/audits`.

The six packages are one arrival, and small. The previous report's own "What wants a lane of its
own" is larger: twenty-one ABI constants and thirty-seven components that no security audit had
read, with `designware_mobile_storage`'s device authority first. That is the lens the last one
lacked, so this audit takes it, with the atlas's packages as its third part.

### The delta, counted the way `script/audits` counts

Distinct registry packages across every committed lockfile but `vendor/`, diffed between
`f1504f19f` (the commit that landed the 2026-10-19 documentation report) and `dd50a277b`. Six
added, none removed, all in `tools/glyph_atlas_generator/Cargo.lock`, arriving with milestone 142 (a text display good enough that people use it instead of a GUI)'s
atlas (#2428):

| Package | Version | Enters through | Ruled |
|---|---|---|---|
| `skrifa` | 0.44.0, pinned `=` | the generator's font parser | calef, 2026-10-20 (UTC), on #2428 |
| `read-fonts`, `font-types` | 0.41.0, 0.12.7 | `skrifa`'s graph | with `skrifa` |
| `bytemuck`, `bytemuck_derive` | 1.25.2, 1.12.1 | `font-types` | calef, 2026-10-20 (UTC), "Yes" to `bytemuck_derive` on #2428 |
| `ab_glyph_rasterizer` | 0.1.10, pinned `=` | the generator's rasterizer | §166 (the rasteriser dependency, and whether the glyph atlas ships one face or four), 2026-10-05 |

### The two uncountable triggers, answered

Has a new component taken device or network authority? **Yes, and it is this lens.** Since
2026-10-03: `designware_mobile_storage` and the DesignWare Ethernet transport on radon, the e1000e
transport and the xHCI keyboard driver on PCs. Since 2026-10-19 alone: none.

Has this booted on a new machine class? **No.** Milestone 890 (riscv64 and x86_64 join the shared boot path) moves x86_64 onto the shared boot
path and milestone 578 (an ACPI discovery path for aarch64) boots QEMU's GICv3 with an ITS; neither is a new class.

## What was read, and what came back clean

**The SD/MMC server.** It holds one register page, its transfer region and two endpoints, uses all
of them, and holds no interrupt. `Host::new` clears the internal DMA controller's enables, the
request count is clamped to the transfer window, `Window::translate` checks overflow and is
Kani-proved, and the block number is bounded against the card. Its header says plainly that the
page also holds `DBADDR` and `BMOD` and that a server which enabled the IDMAC could aim the
controller anywhere, since the JH7110 has no IOMMU.

**The two NIC transports.** Neither holds an interrupt, and each is denied its MAC and filter page.
`rx_verdict` bounds a device-written length to the room, `MAX_FRAME` and the buffer (Kani-proved);
transmit refuses an empty or oversized frame; ring offsets wrap modulo the ring; every region access
goes through a bounds-checked window. On a PC with no VT-d, e1000e runs confined only by its own
arithmetic; that is milestone 261 (the NVMe driver leaves the kernel)'s stated position and its `BUGS`, the boot line and
`notes/e1000e.md` all say so.

**The xHCI driver.** Every capability it holds is used, the MSI-X pages and scratchpads are
withheld, control-transfer lengths are capped at a page, descriptor parses run on the bytes
received, and the completed-report index and buffer offset are bounds-checked.

**The ABI names, at the kernel's check.** `SUSPEND`, `RESUME` and `SET_THREAD_POINTER` need
`WRITE`, refuse an embryo or a dead thread, and `SET_THREAD_POINTER` reaches only an embryo or the
caller. `Process::DESTROY`, `UNMAP` and retyping a `PROCESS` need `WRITE`; a retype from a
`SPEND_ONLY` budget mints no `GRANT`. `WAIT` and `WAKE` check their flags, `READ`, alignment, the
user half, and that the space is the caller's own. `REPLY_CAPABILITY` checks `GRANT` on the carried
capability under the hold that files it. The four renames dispatch as before. The `SIZE_*` names
are the futex flags word's widths, not register widths.

**The atlas's packages, from the package inward.** The generator is its own workspace and none of
the six reaches the root lockfile; `crates/glyph_atlas` has no dependencies, and what reaches the
target is four `include_bytes!` tables and `linear.rs`. `script/supply-chain` discovers the
generator's lockfile, and cargo-deny 0.20.2 passes it. The regenerate gate compares every table
byte for byte and runs in CI's host pass with `--locked`. On the target, `Face::coverage` returns
zero outside the cell and sends any code point outside the table to the missing-glyph box, colors
resolve totally over `u8`, and the per-client surface tables are sized by the kernel from a
compile-time scene with a build-time assert, so no client input sizes them. The vendored fonts'
hashes match `vendor/README.md`.

## Findings

### 1. FIXED: `Virtio::READ_REGISTER` read any physical address

`kernel/src/virtio.rs`'s `read_register` passed the caller's `off` to the mmio arm, which read
`phys_to_virt(mmio_phys + off)`. On aarch64 the direct map covers all of RAM, so a process holding
a `Virtio` capability (every virtio driver: block, net, input, GPU, entropy) could read the kernel
and every other process a word at a time. An unmapped offset faulted in the kernel instead. The PCI
arm was safe, since it matches known offsets and answers zero otherwise. The ABI doc said "Reads
are DMA-safe, so any register", which is true of registers and silent about offsets.

Fixed: `read_register` answers only an aligned word inside the device's own slot
(`register_in_slot`) and refuses anything else as `WrongObject`. Every driver's reads were checked
against the bound first; the highest is the block device's capacity at `0x104`, inside every
architecture's stride. A kernel test holds the bound, and the ABI and runtime docs now say it.

### 2. MINTED: two server threads can forge a reply to a later call

A plain `REPLY` copies the Reply capability, delivers, and only then deletes the slot, and
`REPLY_CAPABILITY` has the same order. `ipc_reply` checks that the caller is parked for a reply,
not for this one. Since milestone 812 the threads of a process share one table, so two threads can
take the same Reply; after the first answers, the client may call an unrelated server, and the
second thread's copy is delivered as that server's answer. A compromised server can do this on
purpose, and `notes/suspend-and-resume.md` names stale-Reply forgery as a class the kernel rules
out. Proposed as
[a reply is consumed and delivered under one hold](../roadmap/proposals/a-reply-is-consumed-and-delivered-under-one-hold.md).
It is not fixed here because the path is the IPC fast path, and a change there records its
footprint. It bears on fatal risk 7 (confinement); the integrator cites it at merge.

### 3. MINTED: `SYS_EXIT_THREAD`'s clear-word store has no lock against reclaim

The exit path walks the page tables for the clear-word and stores zero through the direct map with
no lock held across the two. A sibling thread holding `WRITE` on the region can destroy it in
between, and a reused frame then takes four zero bytes it does not belong to. `WAIT`'s load has the
same walk, and `notes/futex.md` recorded only the load, as a one-bit observation. Not shown
reachable: the audit did not confirm that a destroyed region's frames can be reused inside the
window. Proposed as
[a futex word is walked and touched under the revocation lock](../roadmap/proposals/a-futex-word-is-walked-and-touched-under-the-revocation-lock.md),
and `notes/futex.md`'s `BUGS` now names the store. It bears on fatal risk 7 if it is reachable.

### 4. MINTED: the xHCI driver is mapped the USB Legacy Support capability

The kernel performs the firmware handoff itself because that relationship is one "no process should
hold", but the driver's window withholds only the MSI-X table and pending-bit array. Where
`USBLEGSUP` and `USBLEGCTLSTS` share a page with nothing withheld, the driver can re-arm the SMI
enables or the BIOS-owned semaphore, and the IOMMU does not bound a register write. Proposed as
[the xHCI driver is not mapped the legacy support capability](../roadmap/proposals/the-xhci-driver-is-not-mapped-the-legacy-support-capability.md),
which needs a reading of where xenon's controller puts the capability before the refusal can land.
`notes/usb.md`'s `BUGS` now says so.

### 5. FIXED: `SECURITY.md` and the DMA map overclaimed radon

`SECURITY.md` said that on the VisionFive 2 "the confinement is the software validator and nothing
else", and that radon had no storage or network driver. Both servers exist, and each holds a
register page naming DMA addresses, so neither is behind the validator. `SECURITY.md` now says
virtio is confined by the validator and the board's own servers by their own code, started only
after a bench boot proves them. `notes/dma.md`'s map of address paths gains the row it lacked: a
register page that names DMA addresses, on a machine with no IOMMU, confined by nothing but the
driver.

### 6. FIXED: three documentation edges

- `notes/usb.md` and the driver's header said the controller reaches nothing outside its region.
  `iommu::confine` also maps any region the firmware reserved for the device (an RMRR), which VT-d
  requires; both now say so.
- `rights::BIND`'s doc said "add a thread to a process". A joined thread runs at its builder's
  entry inside the process's table and space, so `BIND` is full control of the process; the doc now
  says to delegate it like `GRANT`.
- `packages/drivers.package.toml` said the EL0 block server "will link" the SD/MMC crate; it does.

### 7. FIXED: a screendump header could wrap the pixmap reader's length

`board_console::screen::parse_pixmap`, the host reader the graphical swish-check leg and the OVMF
gate use to read the terminal off a QEMU screendump, multiplied the header's width and height
unchecked. In a release build a huge header wrapped to a small length, the truncation check passed,
and `decode_exact` indexed past the buffer. Host-only and a panic rather than undefined behavior,
but the fix was one line: the product is checked and an overflow is `NotAPixmap`, with a test that
fails without it.

### 8. ACCEPTED: nothing checks `vendor/README.md`'s font hashes against the files

`script/vendor-verify` reads `vendor/*.pin` only, and the generator reads the fonts by path, so a
replaced font with regenerated tables passes every gate and leaves the table stale. Accepted, and
this is partly about effort: a gate would be a few lines, but it would be a second verification
scheme beside `vendor-verify`'s, which is a design choice for whoever next touches that script
rather than an audit's drive-by. The worst case is a wrong picture that review of the diff sees.
Recorded in `vendor/README.md`'s `BUGS`, beside the existing entry that the hashes are not upstream's.

### 9. ACCEPTED: `bytemuck_derive` runs on the build host

It is a proc-macro, so its code executes on developer and CI machines during the generator's
build, where cargo-deny sees its advisories, license and source but not its behavior. Accepted:
this is the ordinary cost of any proc-macro in host tooling, nothing reaches the target, and calef
ruled on it by name.

## Is any confinement claim false as stated?

Yes, two, and both are fixed. Before finding 1, a `Virtio` capability was not confined to its
device. `SECURITY.md`'s radon sentence (finding 5) claimed a confinement the board's own servers do
not have. `Reboot` checks no
rights by §251 (restarting the machine is a kernel object)'s ruling, so a copy narrowed to nothing still reboots; that is the ruled design, not
a false claim.

## Fatal-risk implications

Findings 2 and 3 bear on risk 7 (confinement) and are minted, so the integrator cites both
proposals from that risk's entry at merge. Finding 1 bore on it and is fixed.

## What was deliberately not examined

- The other components the window added, beyond the four device servers: the resolver and dialer,
  the installer, the timetable keeper, the witnesses and fixtures.
- `net_stack` and the NIC transports beyond their ring and region code; the kernel's bring-up
  bodies for the DesignWare and e1000e controllers; the PCH sequence; `usb::descriptor`'s walk,
  taken on its proofs and call sites.
- The bodies of the six atlas packages. They parse only the vendored fonts, on the host.
- Region `DESTROY` racing a thread join, and whether a destroyed region's frames are reused at once,
  which finding 3 needs.
- The compiler, still: the nightly pin, `patches/std-nife` and the workflow that bumps the pin.

## Method, so the negatives can be judged

Three readers, one lens part each, 2026-10-20 (UTC), on `main` at `dd50a277b`, each finding checked
against the code by the author before it was dispositioned. The package delta is the lockfile diff
above. cargo-deny 0.20.2 was run on the generator's manifest. Finding 1's bound was checked against
every `read_register` caller in `components/` and `crates/` and tested in the kernel suite on
aarch64 and riscv64.

## What wants a lane of its own

The three proposals above, and the next security audit: the compiler, which three audits have now
named and none has taken.
