# Naming crates

*An appendix to [`design/naming.md`](../naming.md). It holds the crate rules: the one spelling,
the suffixes, the crate that shares a program's name, and what `crates/` holds. The directory
`design/naming/` and this file's stem are provisional names; naming is an architect's.*

## One rule, with no second tier

A crate takes `snake_case`: one word where one word will do, and an underscore where the name is a
qualifier applied to a thing (`filesystem_contract` is the contract for the filesystem,
`measured_boot` the measurement of boot). A name never runs two words together (`lineedit`,
`bitfont`): that shape produced every abbreviation a reader had to decode. A standard term keeps its
standard shape: `nifefs` follows `procfs`, which nobody writes `proc_fs`.

Programs follow the same rule, with no short tier for programs a person types. Whether a program is
typed is not a stable property (`wc` went from internal plumbing to a prompt-typed pipeline stage in
a day), and a convention keyed to it produces renames. A short name for a typed command is a choice
its author makes, not a convention to apply.

A crate is the most reader-facing name in the tree. A newcomer greps `crates/` before opening
`components/src/`, and the name appears in every `Cargo.toml` that depends on it and every `use`.

## Standard terms stay

`elf`, `pci`, `paging`, `glob` and `socket_contract` are names a reader knows from outside this
project, so they cost nothing to learn. This rule is a naming authority, not a renaming mandate:
renaming `elf` would destroy the recognition the rule exists to buy. Which acronyms count as
standard terms is the acronym test's call:
[acronyms-and-abbreviations.md](acronyms-and-abbreviations.md).

## Suffixes

- A nife wire-contract crate ends in `_contract`: `filesystem_contract`, `socket_contract`,
  `supervision_contract`. An external standard keeps its own word (`network_time_protocol`), which
  is why nife's suffix is not `_protocol`. `abi` is the syscall boundary and takes no suffix.
  `script/lint` check 3 holds the spelling and names `network_time_protocol` as its one exception.
- `compositor` and `line_editor` hold a protocol module and are logic crates, not contracts. A
  `_contract` name would promise a wire definition and deliver an algorithm.
- A network card transport is `<device family>_transport`: `virtio_net_transport`,
  `e1000e_transport`. Qualify the family only when its name covers more than one kind of device
  (`designware_ethernet_transport`, since DesignWare also names USB, MMC, I2C and PCIe blocks). No
  vendor prefix, because vendors rebrand while family names stay; the vendor goes in the crate and
  module headers.

## A crate that is a component's engine

A crate that holds a component's logic takes the component's name. The crate is the logic, lifted
out to be host-tested, and the program keeps the IO: `line_editor`, `compositor`, `coremark`. A
document tells them apart as "the `line_editor` crate" and "the `line_editor` binary". A second name
for the engine is how `termd` and `linedisc` happened.

Where the two are different things they take different names. The crate `video_terminal` names the
protocol it implements (bytes in, a character grid out). The program `display_terminal` names its
role, the terminal on the display beside `gpu_driver`.

## Page contract crates

A crate that exports a read-only mapped page (`counter_frequency_contract`, `current_cpu_contract`,
`environment_contract`, `machine_statistics_contract`, `program_argument_contract`, and any later
one) spells its constants the same way.

| Constant | Name |
|---|---|
| The length of the page's content, which is less than a page | `PAGE_CONTENT_BYTES` |
| A real page size | `PAGE_BYTES` (`program_argument_contract`, 4096) |
| The address the page maps at | `PAGE_VIRTUAL_ADDRESS`, and `virtual_address` for a parameter that takes it |

`build_page`, `PageBuilder::build` and `build_header` are outside the rule. A crate takes the family
names in its own name review before it leaves, and `helpers/name_release.py --draft` marks the rows
the rule settles.

## Tests and proofs in `file_allocation_table`

In `crates/file_allocation_table`, every module keeps its host tests in `<module>/tests.rs` as the
module `tests`, and its Kani proofs in `<module>/verification.rs` as the module `verification`.
Each such module carries `Name: ratified 2026-11-01 (UTC)` in its own block. A module that breaks
the layout falls outside the rule and keeps its own status, and the rule says nothing of any other
name in the crate.

## What `crates/` holds

`crates/` holds four audiences under one directory, and no name tells them apart. That is a known
gap, not a design.

- Kernel logic, host-tested and Kani-reachable: `capability`, `paging`, `page_frames`,
  `memory_regions`, `generational_table`, `address_space_identifier`, `intrusive_queue`,
  `inter_process_communication`, `direct_memory_access_validator`, `measured_boot`,
  `user_mode_heap`.
- Wire contracts: the `_contract` crates, `network_time_protocol` and `abi`.
- Format and hardware parsers: `elf`, `device_tree_blob`, `pci`,
  `globally_unique_identifier_partition_table`, `nifefs`.
- Userspace libraries: `user_mode_runtime`, `grant_plan`, `virtio`, `video_terminal`,
  `line_editor`, `bitmap_font`, `glob`, `calendar`, `credentialer`, `compositor`, `coremark`,
  `c_seam`.

What two binaries share is a crate, never a `#[path]` module (`AGENTS.md` rule 7), and
`script/lint` check 5 fails at two consumers. What remains beside the programs is single-consumer
submodules, which are ordinary Rust.

## Length

`nifefs` caps a program's archive name at `NAME_LEN = 32` bytes, which clears the longest settled
name by seven. Crates live outside the archive and have no cap. Do not let the limit pick a name,
and do not spend a format change on bytes nothing needs; [nifefs.md](../../notes/nifefs.md) has
the cost per block.
