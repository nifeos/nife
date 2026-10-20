---
status: PROPOSED
raised: 2026-10-20
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: xenon (the only PC whose controller's capability layout matters here)
needs_person: yes
---
# The xHCI driver is not mapped the USB Legacy Support capability

Raised by the 2026-10-20 security audit
([its report](../../audit-reports/2026-10-20-the-window-the-supply-chain-audit-left-unread.md),
finding 4). Name: provisional.

Reuse: `register_window`'s existing withheld ranges and its Kani harness; the kernel's
`find_extended` walk, which already locates the capability for the firmware handoff.

## The failure

The kernel does the USB Legacy Support handshake itself, because the firmware relationship is one
"no process should hold" (`kernel/src/extensible_host_controller_interface.rs`). But the driver's
register window withholds only the MSI-X table and pending-bit array (`kernel/src/pci.rs`,
`find_xhci_device`). `USBLEGSUP` and `USBLEGCTLSTS` are mapped writable whenever they share a page
with nothing else withheld. A compromised driver can set the SMI enables or the BIOS-owned semaphore
again. An SMI stops every core, and the firmware's handler then runs against a controller the driver
configures. The IOMMU does not bound this, since it is a register write, not DMA.

## The fix

Add the capability's eight bytes as a third withheld range, and let `register_window` refuse the
controller if that range shares a page the driver needs.

## Exit test

The Kani harness covers the third range. Before landing, a reading on xenon of where its
controller places the capability, since a refusal there would take the keyboard away: that is the
person this needs.
