# Prior art

An appendix to [the milestone](../1105-pci-outside-review.md).

How the crate sits against what a reader already knows. Recent downloads are the last 90 days.

| source | shape | what it does that this crate does not | what this crate does that it does not |
|---|---|---|---|
| `pci_types` 0.10 (107k recent / 396k all) | `PciAddress { segment, bus, device, function }`, a `ConfigRegionAccess` trait, `Bar::Memory32 { address, size, prefetchable }` and `Memory64`, a `CapabilityIterator` yielding typed MSI and MSI-X capabilities, `EndpointHeader` and `PciPciBridgeHeader` as distinct types | spells every field out; separates endpoint and bridge headers so BAR sizing cannot run on a bridge; carries no device identities | walks bridges with a visited set; carries Kani proofs and falsification patches; parses `ranges` |
| `virtio-drivers` 0.13 (97k recent / 433k all) | `transport::pci` with `DeviceFunction { bus, device, function }`, `bar_info`, a virtio capability walk, `virtio_device_type` from the device id | keeps virtio identities beside the virtio transport, not in the PCI layer | the same closures-over-config-space idea, but host-testable without a `ConfigurationAccess` implementation |
| `pcics` 0.3 (10k / 50k) | a whole-header parser over a 256-byte or 4 KiB slice, every capability typed | types the capability list | bounded hostile-input walks |
| `pci-ids` 0.2 (165k / 1.06M) | the vendor and device name database | names | nothing in common beyond the ids |
| Linux `drivers/pci/probe.c` | `pci_scan_bridge_extend` marks a bridge whose primary bus disagrees with its parent as broken and configures it afresh (detail unverified); `__pci_read_base` sizes with `pci_size`, which takes the lowest set bit of the masked value and returns 0 for an unusable BAR; `pci_swizzle_interrupt_pin` composes INTx through each bridge | tolerates a non-contiguous size mask by taking the lowest set bit; swizzles behind bridges | proves the walk bounds |
| OpenBSD `sys/dev/pci/pci_map.c` | `pci_mapreg_probe` and `PCI_MAPREG_MEM_SIZE(mask)` (lowest set bit), a 64-bit BAR sized as one 64-bit mask | the same lowest-set-bit sizing; whether it refuses a 64-bit type in the last slot is unverified | hostile-input proofs |

Where this crate differs, and what a user coming from `pci_types` will miss: no header types (one
`read_bars` for every function), abbreviated field names (`dev`, `func`, `cfg_type`), and device
identities (`VIRTIO_*`, `CLASS_NVME`, `CLASS_XHCI`) in the PCI layer. What it has that none of them
has: eight machine-checked proofs, each with the patch that makes it fail. It has a breadth-first
bridge walk over a 256-bit visited set that any firmware bus numbering terminates. It has a
device-tree `ranges` parser the two `virt` boards hold against their own trees.
