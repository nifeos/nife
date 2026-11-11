# Prior art

An appendix to [the milestone](../1088-elf-outside-review.md).

How the crate sits against what a reader already knows. Downloads are from the crates.io API during
this review (all-time / last 90 days). Rows marked "784" take their measurements from [milestone
784's appendix](0784-reuse-what-the-tree-wrote/elf-against-three-readers.md), which built, fuzzed
and model-checked each one. The review re-read the appendix and did not rerun it.

| reader | downloads | allocator | `unsafe` | what it enforces | what a user coming from it will miss here |
|---|---|---|---|---|---|
| cole14 `elf` 0.8 (784) | 14.1M / 3.98M | none needed | none, `forbid` | nothing: it reads, the caller judges | ELF32, big-endian, section headers, symbols, dynamic table, any machine; and the name |
| `object` 0.41 read side (784) | 710M / 142M | needs `alloc` with `read_core` | 10 blocks | bounds of each read; note walk strict | every format, any machine, sections, symbols, relocations |
| `goblin` 0.10 (784) | 87.8M / 22.8M | needs `alloc` past the header | 16 blocks | little; panics on a misaligned header without `alloc` | sections, symbols, dynamic, Mach-O, PE |
| `elf_loader` 0.19 | 34k / 8.5k | `no_std`-friendly, per its description (read from crates.io only) | not read | loads and relocates, with a runtime linker | relocation, `ET_DYN`, dependencies between objects |
| `xmas-elf` 0.10 | 3.40M / 674k | none | not read | header and program-header reads | sections; it carries RUSTSEC-2025-0018 per 784 |
| Linux `fs/binfmt_elf.c` (recalled) | | | | `elf_check_arch` per architecture; `PT_GNU_STACK` decides an executable stack; no W^X refusal; a `p_memsz < p_filesz` segment is refused | a parser the caller can hand any machine, and `PT_INTERP`, `PT_TLS`, `PT_GNU_STACK` handled |
| OpenBSD `kern/exec_elf.c` (recalled, not re-read) | | | | refuses a writable and executable `PT_LOAD` unless the file carries `PT_OPENBSD_WXNEEDED` | the opt-out note |

Where this crate differs, it is the only one of the Rust readers that validates everything before it
hands back a segment. It is also the only one with machine-checked proofs (eight, each with a
falsification patch), and it has no `unsafe`, no allocator and a 48-byte parsed type. 784 measured
it at 147 ns per parse plus manifest lookup, within 20 ns of `object` and half of `goblin`, and
found it refusing every crafted-note case the others let through. What it costs: it reads only
static little-endian ELF64 `ET_EXEC` files for one machine fixed at build time. It reads no section
header. Its refusals bake in one loader's policy. The W^X refusal has a kernel precedent in OpenBSD.
The one-segment-per-page refusal has none the review knows of, and is this tree's linker script made
into a rule.

A concrete example of the gap: a tool on an x86_64 build host that wants to list the manifest notes
of an aarch64 package cannot call `Elf::parse` on its members, since the parser refuses them as
`WrongMachine`. `jig` avoids the problem only because it reads packages for `std::env::consts::ARCH`
alone (`jig/src/main.rs`). The note readers (`NoteSegments::from_head`, `NoteSearch`) skip the
machine check for this reason, so the tree already holds two answers to one question.
