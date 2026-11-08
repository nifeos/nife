# Acronyms and abbreviations

*An appendix to [`design/naming.md`](../naming.md#acronyms-and-abbreviations), which states the
rules. It holds each short form's spelling and the cases that decide it, the names the test does not
reach, and why. [§154 (the acronym test is whether the phrase is spoken, applied recursively)](../decisions/0154-the-acronym-test-is-whether-the-phrase-is-spoken.md)
holds the argument for its acronym rows.*

## The test

An acronym or abbreviation stays whole only where nobody says its expansion as a phrase. Ask the
question again of any acronym inside the expansion: `pcie` becomes `pci_express`, and `pci` stays.

The test asks about a reader. Would a competent stranger who has never read this tree recognize the
name? `capsh`, `uheap`, `vt`, `blk` and `kbd` fail, because a reader meets them only inside a Unix
source tree. Shorthand that saves its author keystrokes costs every later reader a lookup, and the
third principle ("a newcomer must be able to succeed without asking anyone") refuses it. The
expansion costs an expert nothing, since a reader who knows the acronym recognizes its expansion at
once.

## Short forms the tree spells out

Every name this tree authors spells these out, function parameters included.

| Short form | Spelled | What decides it |
|---|---|---|
| `cap`, `caps` | `capability`, `capabilities` | every name, PCI capability structures included; comments and prose are outside the rule |
| `op`, `ops` | `operation`, `operations` | `OPERATION_READLINE`; not `opcode`, which names the number and not the request |
| `recv` | `receive` | a syscall this tree defines is its own name, whatever POSIX calls it |
| `rx`, `tx` | `receive`, `transmit` | where the word names a side and not a direction, the name says what it holds: `read_pipe`, `write_pipe` |
| `phy` | `physical_layer_device` | IEEE 802.3's phrase for the chip; not `physical_layer`, which blurs the chip with the layer smoltcp's `phy` module abstracts; prose gives "PHY" on first use |
| `init` | `initialize` | where it means initialize: `initialize_hardware` |
| `irq` | `interrupt`, `interrupt_number` or `interrupt_line`, by what the name holds | the ABI object is `Interrupt`; the module is `arch::<arch>::interrupt_controller`; not `interrupt_request`, so that `irq` reads like `ipi` and `nmi` |
| `fifo` | `queue` | `receive_queue_bytes`; a name that states the order says `first_in_first_out` |
| `min`, `max` | `minimum`, `maximum` | a Unix limit this tree defines is `PATH_MAXIMUM`, `LINE_MAXIMUM`, `NAME_MAXIMUM`; prose that cites Unix's own limit writes `PATH_MAX` |
| `os` | `operating_system` | `OPERATING_SYSTEM_TYPE` in the partition table crate |
| `config`, `spec`, `arg`, `mem`, `dir` | `configuration`, `specification`, `argument`, `memory`, `directory` | §154's table |
| `prog`, `pos`, `opt`, `mtime` | `program`, `position`, `option`, `modification_time` | the same test |
| `va`, `pa`, `phys` | `virtual_address`, `physical_address`, `physical` | every location, with no split by where the name sits: `data_physical_address` |
| `ipi`, `nmi`, `ist`, `msr` | `inter_processor_interrupt`, `non_maskable_interrupt`, `interrupt_stack_table`, `model_specific_register` | §154's table |
| `dtb`, `ipc`, `asid`, `gpt`, `nvme`, `pcie`, `dma`, `pr` | `device_tree_blob`, `inter_process_communication`, `address_space_identifier`, `globally_unique_identifier_partition_table`, `non_volatile_memory_express`, `pci_express`, `direct_memory_access`, `pull_request` | §154's table |
| `initrd` | `initial_ramdisk` | inside the tree; the boundary keeps the owner's spelling (below) |
| `proto` | `protocol` | a truncation, equally short for `prototype` |
| `rgmii`, `mdio` | `reduced_gigabit_media_independent_interface`, `management_data_input_output` | the same test |

## Short forms that stay

- Earned, because nobody says the expansion: `pci`, `elf`, `usb`, `syscall`, `cpu`, `xor`, `io`,
  `bios`, `efi`, QEMU, and the hardware acronyms `MAC`, `GMAC` and `SD`. `glob` and `paging` are words.
- `crc` stays, because nobody says cyclic redundancy check: `has_crc`, `CRC_BYTES`, `crc32`. Every
  name spells it `crc`, and prose expands it at first use, as "cyclic redundancy check (CRC)".
- `id` and `Id` stay, as std writes `ThreadId`, `TypeId` and `process::id`: `device_id`,
  `DEVICE_IDS`. Spell `identifier` out only where ID is one letter of a compound specification
  acronym: `address_space_identifier`, `CardIdentification`, `version_identification`.
- `SDIO` is not an accepted acronym. It is the vendor's label for one controller instance, so a name
  says which instance it means.
- `IPC` in `notes/stack-high-water/per-ipc-depth.md` is a term of art, and the document keeps it.
  `helpers/name_checks.py` carries the exception beside its check.
- `os_primitives_benchmarker` keeps its name until the variable-width step of
  [§307 (how long a name may be)](../decisions/0307-how-long-a-name-may-be.md) renames it.
- `OPERATION_SENDTO` keeps POSIX's `SENDTO` in its suffix until a ruling settles `SEND_TO`.
- `cap` that means a ceiling (`heap_cap`, `PRINT_CAP`) is the English word and abbreviates nothing.
  `helpers/vendored_name_gates.py` lists each such identifier in `CAP_KEPT`.

## A name someone else owns

A name that arrives across another project's interface keeps its owner's spelling, and only the
identifier the owner wrote keeps it. Kept: `errno`, `std`, `argv`, `core::ops`, rustc's
`unsafe_op_in_unsafe_fn`, `std::sync::mpsc`'s `recv_timeout`, smoltcp's `phy` module and its
`RxToken`, seL4's `ReplyRecv`, PCI's `CONFIG_ADDRESS`, the `rdmsr` and `wrmsr` mnemonics, Intel's
`IA32_*` registers, RISC-V SBI's `sbi_send_ipi`, `st_mtime` and `touch -m`. A concept nife names in
its own code takes nife's spelling even when POSIX has a field with the same meaning: a type that
implements smoltcp's `RxToken` is `NicReceiveToken`.

The foreign spelling stops at the boundary. QEMU's `-initrd` flag and the devicetree's
`linux,initrd-start` keep theirs, and every name past the point that reads them says
`initial_ramdisk`. The file at that boundary opens with the expansion, so the reader meets both
words together.

A kept name carries its expansion where a reader meets it. Its doc comment opens with the full name
and the source, as in `**System Control Register** (SCTLR_EL1, Arm DDI 0487): ...`, and the item
carries `#[doc(alias = "<plain-English name>")]`.

A specification's term of art inside a name this tree authors follows the same split. A term a
reader searches for keeps its spelling and takes the expansion and the alias: FAT's `Fat12` and
`Fat32`, `efi_system_partition`, `fs_info_sector`, the JH7110's
`COMPATIBLE_*CRG`. Any other term expands: NVMe's `NVM_READ` is `NON_VOLATILE_MEMORY_READ`.

Hardware register, field and bit names follow
[design/naming.md](../naming.md#registers-and-constants).

## Aliases

A spelled-out name carries `#[doc(alias)]` for the term a reader searches for: the specification's
or the source's spelling (`rgmii`, `E1000_*`, `OSType`, `irq`, `fifo`, a vendor or OpenBSD symbol).
Before a crate's first release it carries none for this tree's own earlier spelling, because nobody
outside the tree has searched for that. After first publication a rename breaks callers, and an
old-name alias can help them.

## Where the existing names stand

Not every existing identifier follows the table yet. `python3 helpers/name_checks.py` lists the
unratified names that break a row the check knows, and each family lands as its own pull request. A
ratified name keeps its spelling until its own name review. Closed records (`BUILT` roadmap blocks,
`design/decisions/`, dated audit reports) keep the names they used, so a grep for the old spelling
still finds them.

The command line abbreviates where the code behind it does not:
[command-line-names.md](command-line-names.md).

## BUGS

- The word list holds no `phys`, so the gate passes a new name that uses it. A reviewer catches it,
  or nobody does.
- The test needs a reader. No script can judge an abbreviation nobody has ruled on.
