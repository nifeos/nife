# Precedents set by example

*An appendix to [`design/naming.md`](../naming.md), which is the rule. calef's rulings in the tier 1
naming review of 2026-10-18 (UTC) settled more than the names in front of him. Each entry below is
the rule a ruling implies, with the names it came from. Unit-specific details are kept off-tree by
the maintainers. The file and its stem are provisional names; naming is an architect's.*

calef, 2026-10-18 UTC: "Write down tonight's precedents and rerun the drafter." The rulings below
are dated 2026-10-18 UTC unless an entry says otherwise. File-name precedents are in
[documents-numbers-and-gates.md](documents-numbers-and-gates.md).

## Abbreviations

### The acronym test also decides abbreviations

§154 (the acronym test is whether the phrase is spoken, applied recursively) was written for
acronyms. The same question settles every abbreviation: does a person say the expansion as a phrase?

- Not earned, so they expand: `config`, `spec`, `arg`, `mem`, `dir`, `prog`, `pos`, `opt`, `mtime`
  (modification time) and the acronyms `va`, `pa`, `ipi`, `nmi`, `ist` and `msr`. The acronym
  outcomes are rows in §154's table.
- Earned, so they stay: `usb`, `syscall`, `cpu`, `xor` and QEMU.
- `ipc` was already ruled by §154. The sweep that applies it is owed.
- Ruled 2026-10-26 (UTC): `ms`, `us`, `id`, `io` and `bios` earn their place and stay; `min`
  expands to `minimum` and `os` to `operating_system`.

### The rules cover function parameters

Ruled 2026-10-26 (UTC): the naming rules apply to a function's parameters as well as to its public
items. `data_phys` becomes `data_physical_address`, and a `blocks_per` names what it counts
(`blocks_per_<what>`).

### A foreign identifier keeps its owner's spelling, and only that identifier

The exemption covers the identifier the owner actually wrote. It does not cover a concept nife names
in its own code. Kept: `errno`, `std`, `argv`, PCI's `CONFIG_ADDRESS` and `CONFIG_DATA`, the
`rdmsr` and `wrmsr` mnemonics, Intel's `IA32_*` register names, RISC-V SBI names such as
`sbi_send_ipi`, `st_mtime` and `touch -m`. Not kept: nife's own opcodes for setting a modification
time expand, because the concept is nife's even when POSIX has a field of the same meaning.

This applies the foreign-spelling rule in [vocabulary-rulings.md](vocabulary-rulings.md) at the
level of one identifier.

### A kept foreign name carries its expansion

Ruled 2026-10-26 (UTC), tree-wide: a name kept under the foreign-identifier exemption carries its
expansion in its documentation. The doc comment opens with the full name and the source's heading,
as in `**System Control Register** (SCTLR_EL1, DDI 0487 <section>): ...`, and the item carries
`#[doc(alias = "<plain-English name>")]`. The first names under the rule are the aarch64 register
accessors in `kernel/src/arch/aarch64/instructions.rs`; the riscv64 and x86_64 accessors follow it
too.

Ruled the same day: the rule extends to a specification's terms of art inside a name this tree
authors. A term the reader searches for keeps its spelling and takes the expansion and the alias:
FAT's `Fat12`, `Fat16`, `Fat32` and `NotFat32`, `efi_system_partition`, `EFI_CLUSTER`,
`fs_info_sector`, NVMe's `prp_pair`, and the JH7110's `COMPATIBLE_*CRG`, `VENDOR_CLKGEN` and
`VENDOR_RSTGEN`. Other terms expand: NVMe's `NVM_READ`, `NVM_WRITE` and `NVM_FLUSH` become
`NON_VOLATILE_MEMORY_READ`, `NON_VOLATILE_MEMORY_WRITE` and `NON_VOLATILE_MEMORY_FLUSH`.

### `VIRTUAL_ADDRESS` and `PHYSICAL_ADDRESS` spell out everywhere

The spelled-out forms `_VIRTUAL_ADDRESS` and `_PHYSICAL_ADDRESS` are used in every location. There
is no split by where the name sits, even where only one kind of address exists in that file.

### The command line abbreviates and the code behind it does not

"We abbreviate on the command line" (2026-10-06, in [vocabulary-rulings.md](vocabulary-rulings.md))
reaches the option and stops there. `--mem` is ratified; the identifiers behind it say
`memory_grant`. Wherever both appear, the difference is recorded as deliberate, so a reader does not
take it for drift.

## Words

### Avoid a word whose kernel meaning differs

Some English words already mean something exact to a kernel reader. Do not use them for another
thing: `schedule`, `swap`, `map`, `console`, `argument`, `tick`, `probe`, `frame` and `flag`.

- A flag is a bit. A letter on a command line is an option, as POSIX says (`options`, never
  `flags`).
- "Argument" is always qualified: a program argument, never a bare `arg`.

### A word the tree uses for two things gets a qualifier

When one word names two things, each gets a qualifier. `handoff` is `state_handoff` in
`capability_plan`, and a note about the framebuffer is `framebuffer-handoff-...`. A `frame` is a stack
frame or a page frame.

### Prefer the standard word

Choose the word the field already uses.

| Instead of | Use | Because |
|---|---|---|
| `due` | `pending` (`PendingReaps`) | the standard word for work not yet done |
| `args` | `operands` (`TouchOperands`) | POSIX calls the non-option words operands |
| `SecondDir` | `mount` (`MountedTree`) | a mount is the act and the word |

### No suffix may imitate a family it is not part of

`SETMTIME_AT` suggested POSIX's `*at()` calls, which this was not. A suffix that a reader knows from
another family makes a claim, so it needs a member of that family behind it.

### Local jargon stays when it is defined where a reader meets it

"Lift" in `notes/interleaving` stays. A term of art coined here is fine when the note that uses it
defines it on the spot, and a rename would only trade it for a vaguer word.

## Shapes

### Name the property, not the count

`SecondDir` became `MountedTree`. An ordinal in a name is false the day the design allows a third.
The name states what the thing is.

### Name the hazard until the fix exists

`IMAGE_ROW` became `IMAGE_PLACEHOLDER_ROW`. While a value is a stand-in, the name says so; the name
changes when the real value lands.

### A manifest says Declaration for what a program asks and Grant for what it gets

`FileGrantDeclaration` sits beside `FileGrant`. "Grant" is used only where authority is conferred:
files, directories and memory. Not every `Spec` is a declaration, and a name must say which it is:
`RunSpec` became `ParsedInvocation`.

### A field that is an array of booleans takes the predicate form

`is_directory[i]`, not `directory[i]`. Each element answers a question, which is the shape the
yes-or-no rule on the main page gives a function.

### Names that follow a renamed thing move with it, across crates

`SecondDirGrant` follows `SecondDir`. A worklist follows its appendix, and the `Flags` family takes
`subtree_flag` along. A rename that stops at the crate boundary leaves a stale name in the next
crate.

### "Contract" can name the type that selects an agreement

calef, 2026-10-18 UTC ("Yes"). In `grant_plan`, `Runtime` became `StartupContract`, its field became
`startup`, and the variants (Native or Std) are unchanged. The word means here what the `_contract`
suffix means on a crate: an agreement between two programs. The type selects which agreement applies.
