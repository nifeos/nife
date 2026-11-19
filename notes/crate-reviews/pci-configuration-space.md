# Review package: `pci_configuration_space`

*Name: provisional. The `pci_configuration_space` package of [crate-reviews.md](../crate-reviews.md),
in its own file because the packages together outgrow the page.*

The architect reads this to clear the crate for its move to its own repository,
`nifeos/pci_configuration_space`. Clearing it is what approves creating that repository
([milestone 1105 (`pci`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1105-pci-outside-review.md),
option A of its open decision, taken provisionally by the maintainer). It describes the crate as `main`
plus this branch leaves it.

## In one paragraph

A PCI function's configuration space decoded over closures. The caller supplies `read32(Bdf, offset)`
and the crate does the rest. It does ECAM addressing and the requester identifier, a breadth-first
walk of the buses behind every bridge, base address register sizing and the capability list. It also
decodes the virtio vendor capabilities, MSI and MSI-X, the legacy interrupt swizzle, and the first
32-bit window of a host bridge's `ranges`. Every volatile access stays in the kernel. A function that answers every read with
garbage leaves the caller with a bounded, well-formed answer or `None`. The crate is `no_std`, has no
dependency of any kind and forbids `unsafe`. Ten Kani harnesses prove the hostile-input properties and
each has a falsification patch. Nothing in it points back into nife. Every renamed public name is
provisional, so the move waits on the architect's ratification of the table under "Names".

## Identity

| item | value |
|---|---|
| crate | `pci_configuration_space`, ratified: the short name `pci` stays ratified, and the architect ruled the package name because another author holds `pci` on crates.io |
| identity values | none: the crate is not a wire contract. No magic, no interface number, no layout a peer reads |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 (`as_chunks` is stable since 1.88) |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest |
| `publish` | `false`; no tag and no publish until the architect says so |
| dependencies | none, at run time or in development |
| size | `src/lib.rs` of 1,279 lines (library and Kani harnesses) and `src/tests.rs` of 1,185 lines |

## Who in nife depends on it

- `kernel`: `kernel/src/pci.rs` (enumeration, BAR placement, capability reads, interrupt routing),
  `kernel/src/memory.rs` (the host bridge window), `kernel/src/drivers/jh7110_pci_express.rs`,
  `kernel/src/arch/x86_64/interrupt_controller.rs` and `kernel/src/arch/x86_64/iommu.rs` (the
  requester identifier).
- `crates/driver_process_contract`: `virtual_io` uses the virtio constants and `VirtioCapability`.
- `crates/jh7110_pci_express`: the root port and its simulation decode through `Bdf` and the register
  offsets.
- `crates/machine_discovery`: `src/jh7110/pci_express.rs`, and `tests/pci_host_bridge_windows.rs`, which
  now holds the five device-tree witness tests that this crate no longer carries.
- Later, the bus server of
  [milestone 1064 (PCI enumeration leaves the kernel)](../../design/roadmap/1064-pci-enumeration-leaves-the-kernel.md)
  will link it from its own repository, the same way. The crate has no dependency on that milestone or
  on the ones it waits for.

`packages/kernel.package.toml` lists the crate among the kernel's interfaces.

What references back into nife from the crate: nothing. No path dependency, no `include_bytes!`, no
milestone number in a doc comment, no tree path.

## What ran

All on this branch, on the committed tip, with the toolchain pinned in the tree unless the row says
otherwise.

| check | result |
|---|---|
| `cargo test -p pci_configuration_space` | 41 unit tests and 2 doctests pass |
| `cargo clippy -p pci_configuration_space --all-targets -- -D warnings` | no warning |
| `cargo doc` with warnings denied | no warning; every public item has a doc comment and `allow(missing_docs)` is gone |
| bare-metal builds | `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none` build |
| MSRV 1.88, on a copy outside the tree | builds and tests pass |
| Miri | the test suite passes |
| `cargo kani`, each of the ten harnesses | all ten `VERIFICATION:- SUCCESSFUL`; the slowest are `read_bars_is_total_for_any_device` (211 s), `the_bus_queue_stays_inside_its_array_on_any_firmware` (57 s) and the two walk harnesses (26 s and 28 s) |
| `script/crate-standalone pci_configuration_space` | one failure. `names`: 66 names are not ratified, which is the architect's table below. The build passes on 1.88 for the host and the three bare-metal targets, the proofs report 10 verified harnesses and 0 failures, and each of the ten falsification patches turns its harness red |
| `script/crate-move --dry-run` | stages the tree and builds it; three findings for the move lane, which the move lane lists again when it runs the move for real, and the list of files in nife that name the crate, which is the path-dependency conversion below |
| `script/lint` | run before the push; the pull request's checks are the record |

Not run, and so not claimed:

- A Kani harness of `walk` on a two-bus machine. It ran over twelve CPU-minutes without a result, so
  this branch stopped it and keeps no harness of the bus scan itself. Literals bound the scan loops,
  and the bus queue, the only part a device controls, has its proof. The module documentation and
  the README say so. This was the ceiling attempt of milestone 1105's item P1-8; the walk proof stays
  in its decomposed form.
- Mutation. The first run happens in the new repository, which exempts the 11 equivalent survivors the
  baseline row already counts.
- The stranger test, the fuzz targets (an opt-in that waits for the repository) and
  `cargo-semver-checks` (a first release has no baseline).
- The template's workflows, which run in the repository.

## Public API

| kind | items |
|---|---|
| address | `Bdf { bus, device, function }`, `Bdf::ecam_offset`, `Bdf::requester_id` |
| header registers | `VENDOR_ID_REGISTER`, `COMMAND_REGISTER`, `CLASS_REVISION_REGISTER`, `HEADER_TYPE_REGISTER`, `BAR0_REGISTER`, `CAPABILITIES_POINTER_REGISTER`, `INTERRUPT_PIN_REGISTER`, `BUS_NUMBERS_REGISTER`, `HEADER_TYPE_BRIDGE` |
| classes and command bits | `CLASS_NON_VOLATILE_MEMORY_EXPRESS`, `CLASS_EXTENSIBLE_HOST_CONTROLLER_INTERFACE`, `COMMAND_MEMORY_SPACE_ENABLE`, `COMMAND_BUS_MASTER_ENABLE` |
| virtio identifiers | `VIRTIO_VENDOR`, `VIRTIO_BLOCK_{MODERN,TRANSITIONAL}`, `VIRTIO_NETWORK_{MODERN,TRANSITIONAL}`, `VIRTIO_ENTROPY_{MODERN,TRANSITIONAL}`, `VIRTIO_GPU_MODERN`, `VIRTIO_INPUT_MODERN`, `VIRTIO_TYPE_{BLOCK,NETWORK,ENTROPY,GPU,INPUT}` |
| the bus walk | `walk`, `enumerate`, `Function`, `BridgeBuses`, `bridge_buses` |
| base address registers | `read_bars`, `Bar { base_physical_address, size_bytes, is_64_bit }`, `BarRefusal` |
| virtio capabilities | `virtio_capabilities`, `VirtioCapability { configuration_type, bar, offset, length_bytes, notify_offset_multiplier }`, `VIRTIO_CAPABILITY_{COMMON,NOTIFY,INTERRUPT_STATUS,DEVICE}` |
| interrupts | `msi_capability`, `MsiCapability` (`message_address_low`, `data`), `msix_capability`, `MsixCapability`, `MsiTarget`, `CAPABILITY_ID_{MSI,MSIX}`, `MESSAGE_CONTROL_REGISTER`, `MSI_ENABLE`, `MSI_64_BIT`, `MSIX_ENABLE`, `MSIX_FUNCTION_MASK`, `MSIX_ENTRY_BYTES`, `MSIX_ENTRY_MASKED`, `VIRTIO_MSIX_NO_VECTOR`, `intx_interrupt_number` |
| host bridge | `memory_window_32_bit` |

No nife type, capability or handoff appears in a public signature. The three capability walks
(`virtio_capabilities`, `msi_capability`, `msix_capability`) share one private walk with one hop bound
of 64, so one bound and one proof cover all three.

What a stranger may find surprising:

- Registers and classes are bare integer constants, not enums, because they are words a device wrote
  and a reader must keep a value a newer device added.
- `read_bars` returns `[None; 6]` for a header that is not type 0, `None` for a non-contiguous size
  mask, and `None` for a 64-bit BAR in the last slot or whose upper half does not probe cleanly.
- `intx_interrupt_number` is the swizzle for a function on bus 0. The routing through a bridge is a
  recorded limitation (BUGS in the milestone), not a function of this crate yet.

## Proofs

Ten Kani harnesses, each with a falsification patch under `falsifications/`.

- The ECAM offset stays inside the 256-bus window.
- `intx_interrupt_number` is total and bounded.
- `read_bars` is total, and every `Bar` it returns has a power-of-two size of at least 16 and an end
  that does not overflow.
- A BAR size is exactly the boundary between the bits a device ignores and the bits it decodes.
- The capability walk, the MSI walk and the MSI-X walk terminate on any device, and an MSI-X table
  size is in `1..=2048`.
- The bus queue stays inside its array for every sequence of bridges firmware could write, and a
  fresh queue holds only the root.
- The ranges parse is total.

The proofs and their timings are under "What ran".

## Names

Every name below is provisional and this package ratifies none. The architect rules the table as
one decision. "Was" is the spelling before this branch. Where the specification spells a name
differently, a `#[doc(alias)]` carries its spelling, and no alias carries a nife spelling. This
package leaves the two ratified names, the crate and `intx_interrupt_number`, as they are.

| item | kind | was | status | proposal |
|---|---|---|---|---|
| `Bdf` | struct | unchanged | provisional | keep; alias "bus device function" |
| `Bdf::device`, `Bdf::function` | fields | `dev`, `func` | renamed, provisional | keep |
| `VENDOR_ID_REGISTER`, `COMMAND_REGISTER`, `CLASS_REVISION_REGISTER`, `HEADER_TYPE_REGISTER`, `BAR0_REGISTER`, `INTERRUPT_PIN_REGISTER`, `BUS_NUMBERS_REGISTER`, `MESSAGE_CONTROL_REGISTER` | constants | the same without `_REGISTER` | renamed, provisional | keep; a register offset ends in `_REGISTER` |
| `CAPABILITIES_POINTER_REGISTER` | constant | `CAP_PTR` | renamed, provisional | keep |
| `CLASS_NON_VOLATILE_MEMORY_EXPRESS`, `CLASS_EXTENSIBLE_HOST_CONTROLLER_INTERFACE` | constants | `CLASS_NVME`, `CLASS_XHCI` | renamed, provisional | keep; both crates in the tree are already spelled this way |
| `COMMAND_MEMORY_SPACE_ENABLE`, `COMMAND_BUS_MASTER_ENABLE` | constants | `CMD_MEMORY_SPACE`, `CMD_BUS_MASTER` | renamed, provisional | keep; the specification's bit names |
| `VIRTIO_BLOCK_*`, `VIRTIO_NETWORK_*`, `VIRTIO_ENTROPY_*`, `VIRTIO_TYPE_NETWORK` | constants | `VIRTIO_BLK_*`, `VIRTIO_NET_*`, `VIRTIO_RNG_*`, `VIRTIO_TYPE_NET` | renamed, provisional | keep; the virtio specification calls the device `entropy` |
| `VIRTIO_GPU_MODERN`, `VIRTIO_INPUT_MODERN`, `VIRTIO_TYPE_{BLOCK,ENTROPY,GPU,INPUT}`, `VIRTIO_VENDOR` | constants | unchanged | provisional | keep |
| `VIRTIO_CAPABILITY_{COMMON,NOTIFY,INTERRUPT_STATUS,DEVICE}` | constants | `VIRTIO_CAP_*`, `ISR` | renamed, provisional | keep; alias `ISR` |
| `VirtioCapability` and its fields `configuration_type`, `length_bytes`, `notify_offset_multiplier` | struct | `VirtioCap`, `cfg_type`, `length`, `notify_off_multiplier` | renamed, provisional | keep; the specification's names as aliases; the unit is on the length |
| `virtio_capabilities` | function | `virtio_caps` | renamed, provisional | keep |
| `MsixCapability` and its fields `capability_offset`, `table_entries`, `pending_bit_array_bar`, `pending_bit_array_offset` | struct | `MsixCap`, `cap_offset`, `table_size`, `pba_bar`, `pba_offset` | renamed, provisional | keep; a count names the things counted |
| `msix_capability`, `msi_capability` | functions | `msix_cap`, `msi_cap` | renamed, provisional | keep |
| `MsiCapability`, `capability_offset`, `is_64_bit`, `message_address_low` | struct, fields, method | `MsiCap`, `cap_offset`, `is_64`, `address_low` | renamed, provisional | keep |
| `MsiTarget { message_address, data }` | struct | `address` | field renamed, provisional | keep; the specification's name is "Message Address" |
| `CAPABILITY_ID_MSI`, `CAPABILITY_ID_MSIX` | constants | `CAP_ID_MSI`, `CAP_ID_MSIX` | renamed, provisional | keep |
| `Bar { base_physical_address, size_bytes, is_64_bit }` | struct | `base`, `size`, `is_64` | fields renamed, provisional | keep; the unit is on the size |
| `memory_window_32_bit` | function | `mem32_window` | renamed, provisional | keep |
| `walk`, `enumerate`, `Function`, `BridgeBuses`, `bridge_buses`, `read_bars`, `BarRefusal` | | unchanged | provisional | keep |
| `HEADER_TYPE_BRIDGE`, `MSI_ENABLE`, `MSI_64_BIT`, `MSIX_ENABLE`, `MSIX_FUNCTION_MASK`, `MSIX_ENTRY_BYTES`, `MSIX_ENTRY_MASKED`, `VIRTIO_MSIX_NO_VECTOR` | constants | unchanged | provisional | keep |
| `intx_interrupt_number` | function | unchanged | ratified 2026-10-27 | untouched |

Names the architect might prefer otherwise: `Bdf` keeps an abbreviation because the specification's
phrase is spoken as the letters (the acronym test), which the template's report counts but does not
refuse. `VIRTIO_CAPABILITY_INTERRUPT_STATUS` is long where `ISR` is what the specification and every
driver say; the proposal is the long form with `ISR` as the alias.

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair. The
register layouts, capability formats and identifiers are specification facts. They come from the PCI Local Bus
Specification 3.0, PCI Express Base for ECAM and the requester identifier, the virtio 1.2
specification section 4.1.4 for the vendor capabilities, and the device tree specification for
`ranges`. The crate copies no driver text.

## What still blocks the move

| item | owner |
|---|---|
| Ratify the names in the table above. `script/crate-standalone` step 0 refuses a crate with provisional names, and the run on this branch fails `names` for exactly that: 66 names in the crate are not ratified. `python3 helpers/name_release.py --draft pci_configuration_space` prints the table for the pull request that ratifies them. | the architect, then a ratification lane |
| Rewrite the `Name:` blocks of the ratified names to ratified. | the ratification lane |
| Run `script/crate-move` for real: it stages the tree at the newest template, rewrites the `Name:` blocks, patch paths and record positions, and makes the repository's first commit. | move lane |
| Convert the four path dependencies in nife (`kernel`, `driver_process_contract`, `jh7110_pci_express`, `machine_discovery`) to `git` and `rev` with `version = "0.1.0"` once the repository exists. | move lane |
| Move the rows of `helpers/template_report_baseline.txt` and the verification notes with the crate; add the first weekly mutation run's exemptions for the 11 equivalent survivors. | move lane |
| Land the milestone's remaining P2 items that belong in the repository: fuzz targets for `memory_window_32_bit` and the walks (opt-in), and the `intx_interrupt_number` bridge precondition with its `BUGS` entry in `kernel/src/pci.rs`. | move lane |
| Run the stranger test and the template's workflows in the new repository. | move lane |

## Open questions

1. Whether `Bdf` should keep its short spelling. Recommendation: keep, with the alias; the phrase is
   spoken as the letters. The maintainer can decide it, a two-way door until the first tag.
2. Whether `intx_interrupt_number` should take the bridge route now or after the bus server exists.
   Recommendation: after; it is a recorded limitation and no consumer needs it.

## Recommendation

Ready for review.
