# Fuzz seeds

Inputs a fuzz target starts from, committed on purpose. See [notes/fuzzing.md](../../notes/fuzzing.md)
for the whole discipline; this file is the part a reader meets when they open the directory and find
almost nothing in it.

**Almost nothing is the point.** A fuzzer starting from `[]` spends its first minutes rediscovering
that a device tree begins `d0 0d fe ed`, which with a sixty-second CI budget means it never gets past
the magic check. So seeds matter. But the seeds this project needs mostly **already exist in the
tree**, committed for their own reasons and tested by their own tests:

| Target | Seeds from | What they are |
|---|---|---|
| `device_tree_blob_walk` | `crates/device_tree_blob/tests/fixtures/` | three real device trees, dumped from the boards we boot |
| `globally_unique_identifier_partition_table` | `crates/globally_unique_identifier_partition_table/tests/fixtures/` | two real disks, formatted by `sgdisk` and by Apple's Disk Utility |
| `file_allocation_table_boot_sector` | `crates/file_allocation_table/tests/fixtures/` | six boot sectors dosfstools' `mkfs.fat` formatted: FAT12, FAT16 and FAT32, at 512 and 4096 bytes |
| `elf_parse` | here | the three files below, because nothing else in the tree is a small ELF |
| `domain_name_system_reply` | `crates/domain_name_system/tests/fixtures/` | three real replies captured from 1.1.1.1: two A records, a CNAME chain with compression, and NXDOMAIN |
| `http_response_feed` | here | two responses, a `200` with a body and a bare `404`; the first byte of each is the read size the target splits at |
| `nifefs_roundtrip` | nothing | the input is a *structure*, not bytes; the fuzzer builds file sets from scratch and reaches the interesting shapes immediately |

`script/fuzz` passes those fixture directories to libFuzzer as read-only corpora. Copying them here
would make a second copy of bytes that already have one, and a second copy drifts.

## `elf_parse/minimal_rx_<machine>.elf`

120 bytes each: a 64-byte ELF64 header and one 56-byte `PT_LOAD` program header, read-execute, with
the entry point inside the segment. It is the smallest thing `elf::Elf::parse` accepts, which is
exactly what a seed should be: past every constant check and every validation refusal, so the
fuzzer's mutations land on the arithmetic instead of on the magic number.

**There is one per machine nife runs**, differing in the `e_machine` field at offset 18 and nowhere
else. All three numbers happen to be under 256, so in practice the files differ at exactly **one**
byte, which `cmp` will confirm:

| File | `e_machine` |
|---|---|
| `minimal_rx_aarch64.elf` | `EM_AARCH64` (183) |
| `minimal_rx_riscv64.elf` | `EM_RISCV` (243) |
| `minimal_rx_x86_64.elf` | `EM_X86_64` (62) |

**Why three and not one.** `crates/elf` picks the machine it accepts at compile time, so any single
seed is right for one host and refused by the other two. There was one seed, the aarch64 one, under
a note saying a riscv64 build would reject it; the note predated `x86_64` becoming a target and
never grew the third case, so on an x86_64 host the `elf_parse` corpus was silently empty and
`crates/elf/tests/fuzz_seed.rs` went red with no available fix (milestone 288).

`crates/elf/tests/fuzz_seed.rs` now asserts that the seed for *this* build parses, and that this
directory holds a seed for every machine in `elf::KNOWN_MACHINES`, so adding a fourth architecture
fails on whatever host adds it.

## BUGS

**Two of the three seeds are dead weight in any one fuzz run.** The target returns immediately on
anything `Elf::parse` rejects, so on an aarch64 host the riscv64 and x86_64 seeds each cost one
rejected input and contribute no coverage. That is the price of committing the corpus rather than
generating it per host, and at 120 bytes and one execution apiece it is not worth removing. It does
mean a corpus count of three overstates what any one run starts from by two.

**Nothing checks that the committed bytes match the generator below.** The script is a record a
reader can re-run, not a gate: an edit to the seeds that the script would not produce passes every
check in this repository, so long as the result still parses and carries a machine
`elf::KNOWN_MACHINES` names. Re-running it was verified byte-identical for the aarch64 seed when the
other two were written (milestone 288), which is evidence about one day rather than a mechanism.

Regenerate all three with:

```sh
python3 - <<'PY'
import struct
EHDR, PHDR = 64, 56
total = EHDR + PHDR
vaddr, memsz, entry = 0x4000_0000, 0x1000, 0x4000_0000

# Every machine `elf::KNOWN_MACHINES` names. A fourth architecture adds a row here and a row in the
# table above, and `crates/elf/tests/fuzz_seed.rs` fails until it does.
for arch, machine in [('aarch64', 183), ('riscv64', 243), ('x86_64', 62)]:
    e = bytearray(EHDR)
    e[0:4] = b'\x7fELF'
    e[4], e[5], e[6] = 2, 1, 1             # ELFCLASS64, ELFDATA2LSB, EV_CURRENT
    struct.pack_into('<H', e, 16, 2)       # e_type = ET_EXEC
    struct.pack_into('<H', e, 18, machine) # e_machine
    struct.pack_into('<Q', e, 24, entry)
    struct.pack_into('<Q', e, 32, EHDR)    # e_phoff
    struct.pack_into('<H', e, 52, EHDR)    # e_ehsize
    struct.pack_into('<H', e, 54, PHDR)    # e_phentsize
    struct.pack_into('<H', e, 56, 1)       # e_phnum

    p = bytearray(PHDR)
    struct.pack_into('<I', p, 0, 1)        # p_type = PT_LOAD
    struct.pack_into('<I', p, 4, 5)        # p_flags = PF_R | PF_X
    struct.pack_into('<Q', p, 8, 0)        # p_offset
    struct.pack_into('<Q', p, 16, vaddr)
    struct.pack_into('<Q', p, 24, vaddr)   # p_paddr
    struct.pack_into('<Q', p, 32, total)   # p_filesz
    struct.pack_into('<Q', p, 40, memsz)
    struct.pack_into('<Q', p, 48, 0x1000)  # p_align

    path = f'fuzz/seeds/elf_parse/minimal_rx_{arch}.elf'
    open(path, 'wb').write(bytes(e + p))
PY
```

## What does not go here

**The working corpus.** libFuzzer writes every input that reaches a new edge into
`fuzz/corpus/<target>/`, which grows without limit and is machine-specific. Gitignored.

**Crash artifacts.** When a target finds a crash, the input becomes a host test in the crate that
owns the bug, where it runs in milliseconds forever and where a reader meets it next to the code.
`crates/device_tree_blob/tests/hostile.rs` is the worked example. A hand-built blob with a docstring saying what
it attacks is worth more than a 7,642-byte file named after its SHA-1.
