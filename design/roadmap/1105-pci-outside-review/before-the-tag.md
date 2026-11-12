# P1: before the tag

An appendix to [the milestone](../1105-pci-outside-review.md).

**3. `read_bars` turns any writable-bits mask into a size, and the size is a power of two only when
the mask is contiguous.**
Where: `crates/pci/src/lib.rs` lines 401 to 408 and 411 to 419.
Why: the specification says the writable bits of a memory BAR are the high bits above the size,
and the crate computes `size = !mask + 1` on that assumption. A function that answers the probe
with a non-contiguous mask, or a 64-bit BAR whose upper dword reads back with no writable bit, gets
a `Bar` whose `size` is not a power of two and may be near `u64::MAX`. Every consumer then trusts
that size: `place_bars` aligns to it and checks the window against it, and the capability resolver
checks `offset + length <= bar.size` against it. The proof `read_bars_is_total_for_any_device`
proves no panic in the decoder, which is true, and says nothing about the shape of the answer.
Linux and OpenBSD both take the lowest set bit of the masked value as the size, which tolerates a
non-contiguous mask; the stricter answer is to refuse one.
Fix: compute the size from the lowest set bit of the mask (both BSDs' `PCI_MAPREG_MEM_SIZE` shape).
Return `None` for a 64-bit BAR whose upper mask is not all-ones above the size (the 64-bit type says
the upper dword is address, so a hard-wired zero there is a malformed function). Prefer refusing a
non-contiguous mask over tolerating it, since loosening later is compatible and tightening is not. Add a proof `a_decoded_bar_size_is_a_power_of_two_of_at_least_sixteen` with its falsification
patch, and unit tests for a non-contiguous mask and for an upper half that reads zero.
Effort: S. Door: two-way.

**4. `read_bars` probes six slots on any header, and a 64-bit type in the sixth slot probes a
seventh.**
Where: `crates/pci/src/lib.rs` lines 373 to 425 (the loop), `:394-399` (the upper half at `off + 4`).
Why: on a type-1 header, slots 2 through 5 are the bus numbers, the I/O base and limit and the
memory base and limit. The write-ones probe rewrites them and restores them, which is a transient
misrouting of everything behind the bridge. The doc comment says "a type-0 header" and every caller
in the tree happens to pass an endpoint found by vendor id or class, but nothing in the signature or
the code says so, and `Function.header_type` is available to say it. Separately, a 64-bit type bit
in slot 5 makes the probe read and write offset 0x28, the CardBus CIS pointer, one past the BAR
slots. The test `a_64_bit_bar_in_the_first_slot_keeps_its_high_half` asserts the probe never touches
it, but only for a fixture whose slot 5 is not 64-bit. `pci_types` avoids both by typing
the endpoint header separately.
Fix: take the header type (or the `Function`) and return `[None; 6]` for anything but a type-0
header, documented as the precondition it is. Treat a 64-bit type in slot 5 as malformed and return
`None` for it without probing 0x28. Extend the proof in item 3 with "no read or write beyond `BAR0
+ 20`" and a test that puts a 64-bit type in slot 5.
Effort: S. Door: two-way.

**5. Names the template's gates refuse at a ceiling of zero.**
Where and what, each a rename:
- `mem32_window` (`crates/pci/src/lib.rs` line 751): `mem` is a spelled-out word. Recommend
  `memory_window_32_bit`, or `non_prefetchable_memory_window`, since the function also refuses the
  prefetchable entry.
- `CLASS_NVME` (`:74`): `nvme` is a spelled-out word. Recommend
  `CLASS_NON_VOLATILE_MEMORY_EXPRESS`, matching the kernel module and the crate of that name.
- `Bar.size` (`:361`), `VirtioCapability.length` (`:454`), `MsixCapability.table_size` (`:528`): a
  size carries its unit and a count names the things. Recommend `size_bytes`, `length_bytes` and
  `table_entries`. `length` mirrors `struct virtio_pci_cap`'s field; keeping it needs the kept form
  (the source named in the first line of its doc comment, plus `#[doc(alias)]`), which it lacks.
- `MsiCapability::address_low` (`:583`), `MsiTarget.address` (`:660`), the locals `pci_addr` and
  `cpu_addr` (`:768-769`): an address is virtual or physical, or it carries a qualifier from the
  list. The MSI value is the specification's "Message Address"; recommend the kept form on the
  field and the method (`message_address_low`), and `pci_address`, `cpu_address` for the locals.
- The test in `src/tests.rs` line 624 that finds the 32-bit memory entry among its neighbors: the
  name spells that word the British way; use the American spelling. The test `an_empty_slot_costs_one_config_read` (`:390`): `configuration`.
  The file `tests/qemu_virt_dtb.rs` and its `dtb` bindings: `device_tree_blob`, moot after item 1.
Why: `check / clippy`'s naming step at the template's head runs these checks with no baseline, so
the first pull request in the new repository is red until they land, and a rename after the first
tag is a minor bump.
Fix: one rename lane across the crate, `kernel/src/pci.rs`, `kernel/src/memory.rs` and
`crates/driver_process_contract`, with the architect's ratification of each spelling.
Effort: M. Door: two-way until the first tag.

**6. Names outside the gates that the naming rules still rule against.**
Where and what, with the recommendation:
- `VIRTIO_BLK_MODERN`, `VIRTIO_BLK_TRANSITIONAL` (`:95, :98`): the acronyms appendix names `blk`
  as a word that fails the spoken test. `VIRTIO_BLOCK_MODERN`, `VIRTIO_BLOCK_TRANSITIONAL`, which
  also matches `VIRTIO_TYPE_BLOCK` four lines below.
- `VIRTIO_RNG_MODERN`, `VIRTIO_RNG_TRANSITIONAL` (`:125-126`) beside `VIRTIO_TYPE_ENTROPY`
  (`:134`): one device, two spellings. `VIRTIO_ENTROPY_MODERN`, `VIRTIO_ENTROPY_TRANSITIONAL`;
  the virtio specification's own name for device type 4 is the entropy device.
- `VIRTIO_NET_MODERN`, `VIRTIO_NET_TRANSITIONAL`, `VIRTIO_TYPE_NET` (`:103-104, :132`):
  `network`.
- `CMD_MEMORY_SPACE`, `CMD_BUS_MASTER` (`:83, :88`): `cmd` is an abbreviation with an expansion
  everyone says. `COMMAND_MEMORY_SPACE_ENABLE`, `COMMAND_BUS_MASTER_ENABLE`, the specification's
  bit names.
- `Bdf { bus, dev, func }` (`:22-26`): `dev` is in the abbreviations table (`device`), and
  `func` abbreviates `function`. `Bdf { bus, device, function }`, as `pci_types::PciAddress` and
  `virtio-drivers`' `DeviceFunction` both spell them. `Bdf` itself reads as a word aloud and stays,
  with `#[doc(alias = "bus device function")]`.
- `VirtioCapability.cfg_type` (`:451`): `cfg` abbreviates `configuration`. The field mirrors
  `struct virtio_pci_cap`; either `configuration_type` with the alias `cfg_type`, or the kept form.
  `notify_off_multiplier` (`:455`) is the same case and the same choice; `off` for `offset` has no
  owner but the specification.
- `CLASS_XHCI` (`:80`): the tree spells xHCI out everywhere else (`extensible_host_controller_interface`
  module and review package). `CLASS_EXTENSIBLE_HOST_CONTROLLER_INTERFACE`, or the kept form with
  the alias `xHCI`.
- `pba_bar`, `pba_offset` (`:534, :538`): "pending bit array" reads as words aloud. `pending_bit_array_bar`,
  `pending_bit_array_offset`.
- `VIRTIO_CAPABILITY_ISR` (`:437`): "interrupt status" reads as words aloud; `VIRTIO_CAPABILITY_INTERRUPT_STATUS`
  with the alias `ISR`.
- `Bar.is_64`, `MsiCapability.is_64` (`:363, :578`): `is_64_bit`; the bare number does not say what
  is 64.
- `VENDOR_ID`, `COMMAND`, `CLASS_REVISION`, `HEADER_TYPE`, `BAR0`, `CAPABILITIES_POINTER`,
  `INTERRUPT_PIN`, `BUS_NUMBERS`, `MESSAGE_CONTROL` (`:45-54, :68, :624`): register offsets end
  `_REGISTER` under the registers rule. `COMMAND` without the suffix is also the dword that holds
  Status, which the capability walks read it for.
- `Bar.base` (`:359`): it is the BAR's bus address, a physical address on both `virt` boards and
  under no IOMMU translation this crate knows of. `base_physical_address`, or the kept form naming
  the specification's "Base Address".
- Locals `orig`, `orig_hi` (`:382, :396`): `original`, `original_high`.
- `MsiCapability`, `MsixCapability`, `MSI_*`, `MSIX_*`, `msi_capability`, `msix_capability`,
  `MsiTarget`, `ecam_offset`, `intx_interrupt_number`: MSI, MSI-X, ECAM and INTx read as
  words aloud and stay, each with `#[doc(alias)]` for its expansion where the item lacks one.
Why: names are the architect's call before the first release, and the abbreviation report (83
components in `src/lib.rs`) lands in every pull request's job summary until the list shrinks.
Fix: the same rename lane as item 5, one ratification list.
Effort: M with item 5. Door: two-way until the first tag.

**7. The documentation assumes this tree, and 26 public items have none.**
Where: `crates/pci/src/lib.rs` line 18 (`#![allow(missing_docs)]` against a 24-item worklist), the
undocumented fields of `Bdf`, `BridgeBuses`, `Function` and `VirtioCapability`, and the constants at
`:45-54, :95, :104, :126, :133-136`. Doc comments at `:4, :38, :79, :131, :514, :566, :638,
:656, :711-713` cite `notes/pcie.md`, `kernel/src/iommu.rs`, `kernel/src/virtio.rs`,
`design/roadmap/0215-...`, `arch::x86_64::interrupt_controller::initialize_io_apic`,
`tests/qemu_virt_dtb.rs`, a bench machine, and milestones 16b, 29, 30, 56, 215, 242 and 320.
Why: a stranger cannot follow one of these references, and the template's `check / standalone`
counts them. The crate's two documentation tests are good; nothing else introduces the crate.
Fix: document every public item (what it is, which register and bit, which specification section).
Replace each tree reference with the specification or with a sentence that stands on its own. Drop
the `allow`. Write the README from item 2 so its EXAMPLES section is the
`enumerate` doctest and its "What is proven" section is the eight harness names in plain words.
Effort: M. Door: two-way.

**8. The MSI capability walk has no proof, and nothing proves the ceiling.**
Where: `crates/pci/src/lib.rs` lines 598 to 619 (`msi_capability`), `:256-303` (`walk`).
Why: `msix_capability` and `virtio_capabilities` each prove termination on any device;
`msi_capability` is the same loop written a third time, and neither harness covers it. A future
edit that drops its hop bound goes unnoticed. One fixture test holds the ceiling property
(`walk` never reads a bus at or above `ceiling`), and the doc comment argues it. A proof would need
the device loop unrolled 32 times per bus and may not finish (unverified), which is the kind of
limit the proofs module should state.
Fix: add `the_msi_walk_terminates_on_any_device` with a falsification patch; factor the three
capability walks over one private `capabilities(bdf, read32, &mut dyn FnMut(offset, head) -> bool)`
so one proof covers all three; try a ceiling proof at `ceiling <= 2` and record the result either
way in the module's doc comment.
Effort: S. Door: two-way.
