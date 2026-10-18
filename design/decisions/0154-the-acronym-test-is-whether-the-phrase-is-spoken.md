---
status: AMENDED
raised: 2026-09-18
decided: 2026-09-18
ratified_by: calef
---

# 154. The acronym test is whether the phrase is spoken, applied recursively

calef, 2026-09-18: an acronym expands where its expansion is a phrase people
actually say, and stays whole where nobody says it; ask the same question of any acronym left
inside the expansion. This replaces the 2026-09-05 test in AGENTS.md and the boundary milestone
265 drew on 2026-09-13, both of which are superseded rather than refined.

What that means concretely, so no lane has to infer it:

| name | is the expansion a phrase people say? | outcome |
|---|---|---|
| `pci` | no. Nobody says "peripheral component interconnect" | **stays** |
| `elf` | no | **stays** |
| `dtb` | yes, "device tree blob" | `device_tree_blob` |
| `ipc` | yes, "inter-process communication" | `inter_process_communication` |
| `asid` | yes, "address space identifier" | `address_space_identifier` |
| `gpt` | yes, "GUID partition table", and "globally unique identifier" is said too | `globally_unique_identifier_partition_table` |
| `nvme` | yes, "non-volatile memory"; "Express" is an ordinary word | `non_volatile_memory_express` |
| `pcie` | yes, "PCI Express", and `pci` stays by the row above | `pci_express` |
| `dma` | yes, "direct memory access" | `direct_memory_access` (already ruled 2026-09-05) |
| `xor` | no. It is the everyday word for exclusive or | **stays**, an earned abbreviation in `kernel-write-xor-execute-check` (calef, 2026-10-18 UTC) |
| `usb` | no. People say "USB", and the expansion adds nothing a newcomer can use | **stays** (calef, 2026-10-17, ruling `usb_keyboard_driver`) |
| `syscall` | no. It is the everyday word for "system call", and the tree uses it everywhere | **stays**, kept tree-wide as an abbreviation (calef, 2026-10-18: "Yes") |
| `cpu` | no. It is the scheduling unit's word | **stays**, by the rule already in design/naming.md (2026-10-03); not a new ruling |
| `errno` | not ours. It is C's own identifier | **stays**, recorded as a foreign identifier (calef, 2026-10-18) |
| `config` | yes, "configuration" | **expands** tree-wide, a foreign name keeps its owner's spelling (PCI's `CONFIG_ADDRESS` and `CONFIG_DATA`). calef, 2026-10-18 (UTC): "Yes: config is not earned. Spell it out tree-wide; foreign names like PCI's CONFIG_ADDRESS keep their spelling." |
| `spec`, `arg`, `mem`, `dir` | yes: "specification", "argument", "memory", "directory" | **expand** tree-wide. calef, 2026-10-18 (UTC): "Yes: spec, arg, mem and dir are not earned. Have the drafter propose the whole family's names together, and bring it back as one table." The `grant_plan` manifest family (`ArgSpec`, `FileSpec`, `MemSpec`, `DirSpec`) waits for that table |
| `va`, `pa` | yes, "virtual address", "physical address" | **expand** to `_VIRTUAL_ADDRESS` and `_PHYSICAL_ADDRESS` everywhere, with no split by location. calef, 2026-10-18 (UTC): "VA and PA are not earned, but use _VIRTUAL_ADDRESS / _PHYSICAL_ADDRESS everywhere, no split." |
| `ipi`, `nmi`, `ist`, `msr` | yes: "inter-processor interrupt", "non-maskable interrupt", "interrupt stack table", "model-specific register" | **expand** to `inter_processor_interrupt`, `non_maskable_interrupt`, `interrupt_stack_table` and `model_specific_register` (calef, 2026-10-18 UTC: "Yes"). Owner spellings stay: the `rdmsr` and `wrmsr` mnemonics, Intel's `IA32_*` register names, and RISC-V SBI names such as `sbi_send_ipi` |
| `pr` | yes, it is said as "pull request" | `pull_request` (calef, 2026-10-18, "Approve all") |

**`pci_express` is the row that shows the rule is one rule and not two.** It falls out without a
special case, and it is exactly what people write.

## What was wrong with the two tests this replaces

The record contradicted itself, in three layers, and nobody had noticed. `elf`, `pci`, `dtb`
and `gpt` were all ratified on 2026-08-01 under a tenet calling standard terms untouchable, and
all four blocks still carry that sentence. AGENTS.md's 2026-09-05 acronym test then said `dtb`,
`gpt`, `ipc` and `asid` "sit the same way" as the deratified `dma_validator`, which reads as: they
expand. Milestone 265 on 2026-09-13 then listed `elf`, `pci`, `dtb`, `gpt` as the ones that
stay whole. The middle layer and the last layer cannot both be right about `dtb` and `gpt`.

**The 2026-09-05 test's stated reason was an asymmetry that does not hold.** It said *a reader who
knows the term recognizes its expansion instantly, so spelling it out costs the expert nothing and
saves the newcomer a bounce.* That is true of "device tree blob" and false of "peripheral component
interconnect": an expert meeting the second has to translate it back to `pci` to know what they
are looking at, so it costs them. The new test keeps the asymmetry and fixes the thing that measures
it. Recognition is not about whether an expansion exists; it is about whether anybody uses it.

265's format-versus-protocol boundary was inapplicable in principle, and its own block said so:
*"nothing mechanical can tell a format from a protocol. The next name that tests it comes to
calef."* It was drawn to separate `ntp` (expand) from `gpt` (stay), and six weeks later `nvme`
arrived looking like hardware and ruling like a protocol. A rule whose author records that it
cannot be applied without him is rung four of AGENTS.md's ladder wearing the clothes of a rule.

## Why "is it spoken" is better than "does it teach"

Both are judgments, and this decision does not pretend otherwise. The difference is who can
answer them and whether they can be checked.

- *Does the expansion teach the reader something?* is a question about an imagined newcomer, and
  two people will answer it differently for the same name. It produced three incompatible layers in
  six weeks.
- *Do people say this phrase?* is a question about the world. It can be checked against the
  specification, the vendor's own documentation, and how the term appears in prose, and two people
  checking will usually agree. It is not gateable, but it is arguable from evidence rather than
  from taste, which is the property AGENTS.md asks of a rule a newcomer must apply without asking.

The flat rule was considered and refused. "Every acronym expands" is the only fully mechanical
option and it is the one a gate could enforce, which is a real argument given the ladder. It loses
because `peripheral_component_interconnect` (33 bytes, over `nifefs`'s `NAME_LEN`) and
`executable_and_linkable_format` are worse names than the acronyms they replace, by the rule's own
purpose: they do not help the newcomer and they do cost the expert. A rule that produces names
nobody would choose is not saved by being mechanical.

## The cost, recorded rather than hidden

**The sweep is large and most of it is not identifiers.** `elf` appears 1,139 times across 225
files, `pci` 919 across 157, `ipc` 1,100, `dtb` 718, `gpt` 527, `asid` 408. Four senses of each
word, and only the first moves: an identifier; a citation of a standard (`NVMe 1.4 §3.1`, an ELF64
header); a bench transcript, which is evidence and is never edited; and a `BUILT` roadmap block,
which is an account of what was built under the name it had. AGENTS.md's own scar is the blind
`sed` that rewrote the row recording a name's *refusal*.

Searchability is the standing cost. Datasheets, error messages and this kernel's own boot output
say `gpt` and `nvme`, so a reader grepping the word the machine printed will not find the identifier.
That was weighed for `nvme` and applies to every row here.

And this decision will itself be tested. `acpi` ("advanced configuration and power interface",
42 bytes) and `uart` ("universal asynchronous receiver-transmitter", 43) are the next two, and both
look like `pci` to me: the expansion exists and nobody says it. A lane meeting one applies the test
and records its answer rather than asking, which is the point of replacing a rule that had to be
asked.

## What this does not touch

Programs typed at a prompt keep their latitude. `mdr`, `ps`, `wc` and `pgrep` are short because
a typed command is a different thing from a crate a newcomer greps, and 265 recorded that exception
already. Nothing here narrows it.

**Part numbers are not acronyms.** `pl011` and `ns16550` name specific silicon and have no
expansion.

## Sequencing, because two lanes are live in these files

Milestones 320 and 321 are open against `fix/host-bridge-test-on-real-chipsets`, and 320 is
rewriting `kernel/src/pci.rs` and `crates/pci` in particular. Every rename this decision
authorizes waits until those land. The name blocks say `provisional` with the ruling recorded
beside them, the shape `board_console` uses for its ruled `serial_console`, so
`script/names --unratified` keeps carrying the work rather than dropping it.
