---
status: NOT-STARTED
raised: 2026-10-05
promoted_from: argon-boots-the-aarch64-kernel
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none to build; aarch64 silicon to confirm
specific_machine: argon (the Jetson TX1, milestone 127's board)
needs_person: no
---
# 803. argon boots the aarch64 kernel: the board memory map nobody has built

*(Promoted from the proposal pile on 2026-10-06 (UTC); number provisional until the merge queue lands it.)*

Raised 2026-10-05 (UTC) by `lane/225-argon-soak`, the lane for milestone 225 (run the soak on radon,
argon and xenon), while making argon's soak a one-command run. Title, slug and every name here are
provisional.

*(Awaiting the board, 2026-10-06: the board delivered as argon is a Jetson TK1 shipped against a
TX1 order, going back while calef gets the TX1 from the seller; `notes/bench-runbook.md`, "argon is
not in hand". This proposal's tegra210 facts and its fork stand, and its bench confirmation waits
for the TX1. The finding that the aarch64 kernel only fits QEMU `virt` holds for any board. Draft PRs #1738
(`lane/argon-boots`, the tegra210 board option) and #1732 (`lane/argon-ethernet-proposal`, the argon
ethernet proposal) were closed 2026-10-06 to wait for the TX1; the branches are kept, so reopen both
when it is on the bench.)*

## The premise this corrects

Milestone 127 (the seL4 machine) names two pre-board prerequisites, the EL2 to EL1 drop and
the aarch64 half of milestone 74 (cycle counters), and both are built. `notes/bench-runbook.md` reads from that that
argon's first light is bench work. It is not. The board survey listed a third baseline item, "a
**board memory map** (DRAM base, MMIO windows)" (`notes/aarch64-board-survey.md`), and nothing
has built it. As the tree stands at `eb037d373`, the aarch64 kernel can only run on QEMU `virt`:

| what | where it is fixed | QEMU `virt` | tegra210 (argon) |
|---|---|---|---|
| physical load address | `kernel/link-aarch64.ld` `PHYS_START`, `image_header.s` `text_offset` 0x80000 | RAM at 0x4000_0000, so 0x4008_0000 | DRAM at 0x8000_0000 (127's step 2 expects it), so `booti` puts the image at 0x8008_0000 |
| boot block map | `boot.s`: L1[0] device, L1[1] normal RAM | right | the kernel's own RAM is unmapped, and the UART at 0x7000_6000 falls in L1[1] as *normal* memory |
| console UART | `console.rs` picks `Pl011` on aarch64; `mmu.rs` `UART_BASE` 0x0900_0000 | PL011 | 16550, `reg-shift=2`, at 0x7000_6000 (the survey) |
| virtio-mmio window | `mmu.rs` `VIRTIO_MMIO_BASE` 0x0a00_0000, mapped and probed unconditionally | 32 slots | unknown; riscv64 found QEMU's constants colliding with the JH7110's map on radon's first boot |

So a TX1 `booti` today jumps into an image linked 1 GiB below where it sits. The MMU-on jump
lands on unmapped memory before `console::initialize`, and the board prints nothing. That silence
would be read at the bench as a cabling or firmware problem, which is the expensive misreading this
proposal exists to prevent. What is already right, read rather than assumed: the GIC's addresses
come from the device tree (`mmu.rs`, step 6), as does the PSCI conduit, the PCIe windows and the
DRAM extent.

## What to build

Make one `cargo xtask` invocation produce an argon Image that reaches the banner, rehearsed as far
as QEMU can rehearse it:

1. The load address, the boot block map and `image_header.s` agree with tegra210's DRAM base.
2. The console is the 16550 at 0x7000_6000, through the existing `drivers/ns16550.rs`, which already
   takes `reg_shift` for the JH7110.
3. The virtio-mmio probe is skipped where no device tree node says the window exists, the way the
   PCIe windows already are.
4. `script/board-image` learns argon: an aarch64 Image, the archive built first (the order that
   matters for measured boot), and a U-Boot script for `booti`. Today it is riscv64 only.
5. A QEMU rehearsal of the same binary. QEMU has no tegra210 machine, so this proves the image
   builds, seals and boots where QEMU can put it. The board alone proves the map.

Reuse: the tree's own parts throughout: `drivers/ns16550.rs` (already shaped for `reg-shift`),
riscv64's `board` feature as the pattern, and `cargo xtask board-script` for the U-Boot script. The
kernel half is ours by §46 (thin primitives or whole subsystems; we write everything in between). Outside code considered: Linux arm64's `head.S`, for option C below.

Exit: the build exists and its QEMU rehearsal is green. Milestone 127 then owns the serial byte.

## The fork, which is the architect's

How one tree produces a kernel for two RAM bases.

- **A. A build-time board feature** (`board` plus an aarch64 board selection): relink at 0x8008_0000,
  map the matching block, pick the UART by `cfg`. Precedent: riscv64's `board` feature already
  builds radon a different binary (`semihosting.rs`, `mmu.rs`). Cost: small and mechanical, the
  four rows above. What it costs in kind: two aarch64 binaries, which is the shape 127's EL-drop
  section refused ("a switch means one binary for QEMU and a different one for argon").
- **B. riscv64's `text_offset` trick, one link address inside both RAMs.** Refused by arithmetic.
  `text_offset` is one number added to each machine's own RAM base, and the two bases differ by
  1 GiB, so the two landing addresses always differ by 1 GiB. riscv64 escaped this only because
  its link address happened to sit inside both machines' RAM.
- **C. Position-independent early boot.** `boot.s` reads its own load address with `adr`, builds
  the boot map there, and the kernel keeps a runtime image offset, Linux's `kimage_voffset`. One
  binary, devices from the device tree. Cost: more than A, and not only effort. `phys_to_virt`
  is `pa | KERNEL_VA_BASE` (322 call sites across the kernel use the direct-map helpers), and
  that is right for the direct map but not for the image's own symbols, so C adds a second
  translation with a runtime term. That is a moving part on a hot path, which should be measured
  before anyone calls it free.

Recommendation: A, as the first step. If both cost the same, would I still choose A? Yes,
narrowly: A has fewer moving parts at runtime, the riscv64 precedent is the same shape, and C's
benefit (one binary) is real but argues against a defect class nobody has yet seen here. That
answer leans on C's runtime cost, not on effort, so it should be checked against a measurement if
C is ever built. Reversible: nothing outside the build has acted on either, and A can be replaced
by C later without touching a wire format.

Prior art, recalled and not read: Linux arm64 does C, and has since KASLR. U-Boot `booti` honors
`text_offset` from the base of the first DRAM bank.

## Then milestone 225's argon run

With this built and 127's bench procedure passed (a byte, then four cores), the soak is radon's
procedure with argon's names. That means `script/board-image --soak` for argon, a `script/board-console`
profile for argon's prologue (deliberately absent until a capture exists), and
`script/board-console --exposure` to write the curve's row. 225's block has the steps.

## BUGS

- tegra210's addresses here come from the survey (UART) and 127's own expectation (DRAM), neither
  read off the board. 127's step 2 (`bdinfo`) is what confirms the DRAM base.
- Whether anything else on the boot path assumes QEMU was not swept exhaustively. The four rows
  are what a read of `boot.s`, `link-aarch64.ld`, `mmu.rs` and `console.rs` found.

## Index row

Milestone 127 (the seL4 machine)'s first light on argon is not bench work yet. The aarch64 kernel is linked, mapped and consoled for QEMU `virt` only. A TX1 `booti` today would land 1 GiB from where it was linked and print nothing, which the bench would misread as cabling. Building the board memory map (load address, boot block map, the 16550 console, a device-tree-gated virtio probe) and teaching `script/board-image` argon turns that silence into a banner, with one architect's fork: a build-time board feature or position-independent early boot.
