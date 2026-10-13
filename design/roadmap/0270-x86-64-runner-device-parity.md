---
status: NOT-STARTED
raised: 2026-09-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 270. Wire `virtio-gpu-pci` and `virtio-input` into the x86_64 test runner

Not a design fork. `helpers/qemu-runner-aarch64.sh` and
`helpers/qemu-runner-riscv64.sh` already wire these devices. This brings the third runner
script into line with the other two, the way `notes/architecture-list-sweep.md`'s eleven gaps
already are.

## What is skipping, and why this is the cheapest fix in the inventory

Four `#[test_case]`s skip on every x86_64 test run, all for the same reason.
`helpers/qemu-runner-x86_64.sh` wires no `virtio-gpu-pci` and no `virtio-input` function onto the
PCI bus it enumerates.

- `kernel/src/user/display_tests.rs`, twice (GPU driver start; keyboard-to-terminal byte)
- `kernel/src/user/compositor_tests.rs` (GPU driver start)

Each site's own comment already states the cause correctly and calls it "an honest, expected gap
rather than a bug" (milestone 164 (x86_64 userspace can't build `aes`)'s shape: a scope gap named
where the reader meets the feature). It is not a capability gap. virtio-gpu and virtio-input are
PCIe, their BARs are memory, and neither driver maps device registers directly; both hold a
kernel-mediated `Virtio` capability (`components/src/gpu_driver.rs`,
`components/src/keyboard_driver.rs`). DECISIONS §121 (what a device capability is when the device
has no page) does not touch them.

So this is a missing command-line flag in a shell script, not a design question, and it is the
single change that closes four of the seven skips found in the 2026-09-10 inventory.

## What this needs

1. Add the QEMU device arguments `helpers/qemu-runner-aarch64.sh` / `-riscv64.sh` already pass for
   `virtio-gpu-pci` and `virtio-input` (keyboard) to `helpers/qemu-runner-x86_64.sh`, gated the same
   way the other two gate them (`NIFE_GPU`, `NIFE_KEYBOARD`).
2. Confirm the four sites above stop skipping and pass under q35's PCI enumeration (milestone 165),
   not just under `virt`'s.
3. Update each site's comment: the "honest, expected gap" framing is no longer true once the device
   exists, and a comment that keeps saying so after the fix lands is the §76 defect in miniature.

## BUGS

- **Not yet verified that q35 accepts the same `-device virtio-gpu-pci` invocation `virt` does.**
  QEMU's PCI topology differs by machine type; this may need a different bus/slot address rather than
  a literal copy of the other runners' flags.

## Follow-on

- **None.** Once the four sites pass, this does not touch milestone 182 or DECISIONS §149; it is
  display and input, not the console.

## Index row

calef, 2026-09-10. Four of seven live x86_64 skips, one missing runner flag.
