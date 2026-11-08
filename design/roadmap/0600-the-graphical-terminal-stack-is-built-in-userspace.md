---
status: BUILT
raised: 2026-09-26
built: 2026-09-26
promoted_from: build-the-graphical-terminal-stack-in-userspace
---
# 600. The graphical terminal stack is built in userspace

*(Number and title provisional. The lane minted the next free number after 596 to
599, which open pull requests hold, and the integrator confirms it at merge.)* Promoted from the
proposal `build-the-graphical-terminal-stack-in-userspace`. The lane for milestone 23 (a
capability-routed component OS with live replacement) filed it on 2026-09-26, on its own branch
(#1342). Built 2026-09-26 by lane `milestone/600-userspace-graphical-stack` on aarch64 and riscv64.
x86_64 has no virtio-gpu, and its screen terminal is a separate case; both are recorded below.

## The question, answered

The proposal asked whether the gpu's DMA pages are one frame run, and whether its `virtio`
registration accepts one. Yes to both, since milestone 142 (a text display good enough that people
use it instead of a GUI). `display_service::wire_device` allocates the region with one
`alloc_contiguous_zeroed(DMA_PAGE_FRAMES)`. It registers it with `crate::virtio::register` as one
region, and grants it as one `PageFrame` capability (DECISIONS §102 (a Frame names a run of pages)).
`gpu_driver` maps it in one `MAP`.

So the premise behind milestone 177 (wire the graphical terminal stack into the real interactive
boot) had expired a month earlier. It built the stack in the kernel because "a virtio-gpu device
alone needs eleven capability-table slots". The gpu is four capabilities.

## What changed

The kernel builds no graphical process any more. It wires the devices and grants their raw materials
to the progenitor, which builds `gpu_driver`, `display_terminal` and `keyboard_driver` itself. That
is how it already built `entropy` and `net_stack`.

| slot | grant | was |
|---|---|---|
| 12 | the gpu's surface run (`READ \| WRITE \| GRANT`) | the kernel-created keystroke endpoint `kbd_ep` |
| 17, 18, 19 | the gpu's transport, interrupt and whole DMA run | nothing |
| 20, 21, 22 | a virtio keyboard's transport, interrupt and DMA page | nothing |
| 10, 11 | the firmware-screen terminal's endpoint and page | also the gpu stack's terminal, until now |

- `build_graphical_stack`, in `crates/system_initializer`, gives the driver and the terminal exactly
  the slots `display_service::start_terminal` gives them. It takes the three reports the boot must
  take: `UP`, `TERM_UP`, then the one `FLUSHED` whose absence hung milestone 177. It returns the
  terminal's endpoint and output page.
- `build_keyboard_driver` builds the keyboard in `MODE_DIRECT` on the terminal endpoint the
  progenitor now creates itself. With no virtio keyboard, the UART's `input` driver is built as on a
  plain boot. That is milestone 192 (a keyboard on real silicon)'s option A, decided by one `if` in
  userspace instead of a `match` in the kernel.
- The drivers read their DMA base out of their DMA region. A process that holds a run as a
  capability knows no physical address, which is the point. So the kernel writes each region's
  physical base into its first page at `abi::virtio::DMA_PHYS_OFFSET`, and `gpu_driver` and
  `keyboard_driver` read it there instead of from `x1`. The rng and the NIC have used that
  convention since DECISIONS §120 (a QEMU-only virtio-rng stopgap for the interactive boot). The
  kernel's test wiring writes it the same way, so the harness and the boot give each driver one
  world. The offset was a number in two places; with four readers it is a constant in `abi` (AGENTS.md
  rule 7). Name provisional.
- The three programs are measured now. `kernel::user::program` reads the archive and checks nothing,
  so nothing vouched for them while the kernel built them. The progenitor measures them like
  everything else it loads. One it cannot vouch for costs the graphical terminal, not the boot.
- Retired: `kernel::user::boot_graphical_terminal`, `GraphicalTerminal`, `KeystrokeSource`,
  `keyboard_service::start_direct`, and `kernel/src/user/input_service.rs`. The last existed only to
  spawn `input` in the kernel on a graphical boot.

## The builder moved again: from the boot to the launch (2026-09-30)

calef's 2026-09-30 ruling (milestone
[632](0632-graphics-on-demand-screen-launched-from-the-prompt.md)) moved where this stack is built.
No boot builds it any more. `build_graphical_stack`, rewritten as `build_graphical_terminal_session`, runs in
the spawn service when a `graphical_terminal` session is launched from the swish prompt. Everything this
milestone decided survives. The kernel grants the devices rather than building them, and the slots
are the same. So are the DMA-base-in-the-first-page convention and the measured programs. The
one-time reports (`UP`, `TERM_UP`, `FLUSHED`, `KEYBOARD_UP`) are still taken in the order the drivers
send them, which is this milestone's three-week lesson carried forward. The BUGS entries below
describe the boot-time builder and are historical. The silent trap became a refused launch, and the
UART is never freed early, because no boot has a keyboard to free it for.

## What proves it

`script/swish-check --graphical` and `--graphical-serial` boot the real progenitor on aarch64 and
riscv64 with a virtio-gpu. They are the `swish-check-graphical` row of `script/ci-build`. Each finds
`swish`'s prompt on the screen and types a key that must echo back through the terminal: through
`keyboard_driver` in one leg, and through `input` over the UART in the other. A slot collision in the
new builds fails that boot in silence, so a prompt on the screen disproves one.

The kernel's display and keyboard tests (`display_tests.rs`, `compositor_tests.rs`) now run both
drivers against the page-held DMA base, which is the other half of the change.

## What it unblocks

Milestone 23's second live swap. `display_terminal` is now a child of the progenitor, built from
capabilities the progenitor held, so it could build a replacement from the same ones. The swap
itself is milestone 23's, and one cost of it is below.

## BUGS

- A graphical stack that does not come up now traps the progenitor with no sentence. The kernel
  used to `assert_eq!` each report with a message naming the program. The progenitor has no console
  at that point, and with a virtio keyboard the UART is already released. So it traps (`must_ok`)
  and the kernel reports only a fault address. A driver that exits without reporting parks the boot
  in `recv`, as the kernel's `ipc_recv` did before.
- A boot with all four QEMU devices is counted, not measured. With a gpu, a keyboard, a virtio-rng
  and a NIC, the progenitor starts with twenty kernel grants, four more than before. It frees the
  UART's two at once, since a virtio keyboard leaves them no user. Entropy's build then peaks at
  twenty-two of twenty-four by count. No gate boots all four devices; `swish-check --graphical`
  attaches the gpu, the keyboard and the rng.
- Keeping `display_terminal`'s endowment for a swap costs slots this table does not have. The
  progenitor deletes the terminal's endpoint, output page, surface and display endpoint once
  `line_editor` holds its copies. A supervisor that rebuilds the terminal needs at least those four
  for the life of the boot, and `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` is 23 of 24. That is
  milestone 23's to solve when it builds the swap, by the two buy-backs that constant names or by a
  raise. It is recorded here because this milestone makes the question live.

## Architectural parity

- aarch64 and riscv64: built, and proven by both graphical legs on each.
- x86_64: no virtio-gpu, so nothing to move. QEMU's x86 runner attaches none and no PC in the fleet
  has one, and `display_service::wire_device` answers `None` when the bus has no gpu. x86_64's
  terminal on a screen is the firmware-screen path. That is still built by the kernel, for a
  different reason: the kernel must hand the screen over from its own console under the console lock
  (`console::yield_screen`). The proposal `build-the-firmware-screen-terminal-in-userspace` covers it.

## Follow-on

- **Milestone 664.** Milestone 664 (build the firmware-screen terminal in userspace). `design/roadmap/0664-build-the-firmware-screen-terminal-in-userspace.md`: the
  same move for `boot_screen_terminal`, which is the only way to swap `display_terminal` on x86_64.
  And `design/roadmap/0677-the-progenitor-stack-has-no-measured-headroom.md`: this lane's
  first gate overflowed the progenitor's stack, which nothing measures.
- **Recorded.** The silent trap, the four-device count and the swap's slot cost are in `BUGS` above.
  The swap's slot cost is also in this lane's report to milestone 23.

## Index row

The progenitor builds `gpu_driver`, `display_terminal` and `keyboard_driver` from device grants
where the kernel used to build all three, now that a gpu's DMA region is one capability. Proven by
both graphical `swish-check` legs on aarch64 and riscv64; x86_64 has no virtio-gpu.
*2026-09-30, milestone 632 (graphics on demand): no boot builds anything graphical now. The same
builder runs at a `graphical_terminal` launch, from the same grants, and the legs prove it there.*
