# The ELF parser: what `crates/elf` accepts, and why

[`notes/elf.md`](../elf.md) carries the format and how the kernel loads a program. This page
carries the design of the parser crate itself, `crates/elf`, which reads untrusted images: what it
refuses, how it decides which machine a file may be for, and how it finds a note.

## The scope it parses

The crate is pure logic. It compiles for the host and its tests run in milliseconds with no emulator
(§7 (testing: QEMU harness + host-testable crates, from commit one)). Nothing in it knows what a
page table is. It answers one question: what does this file want put where, and with what
permissions?

It parses static, little-endian, `ET_EXEC` binaries for this build's own machine and nothing else.
There is no dynamic linking, no relocation, no interpreter and no PIE. Each of those is a real
feature, and each is also a way for a file to ask the loader to do something surprising. Saying no
costs eleven lines; saying maybe would cost a thousand.

Forging a hostile binary is a few lines of a test builder on the host. Producing one from a real
toolchain, getting it into an initial ramdisk and booting QEMU to watch it be refused would be a
day's work and a slower test. That is why the parser is a host crate.

## Validation happens once, up front

`Elf::parse` checks the header, the program-header table's bounds, and every `PT_LOAD` segment's
bounds, permissions and address arithmetic before it returns. A loader that validated as it
mapped would have mapped half a bad program by the time it found out. Every later accessor trusts
that pass.

The per-segment arithmetic a hostile header can weaponize, `p_offset + p_filesz` within the file and
`p_vaddr + p_memsz` without overflow, is factored into `check_segment_bounds`. It is pure and
loopless, so Kani proves it panic-free and its result in bounds for every field combination. A proof
that the whole of `parse` is total did not finish:
[`notes/verification/elf-parser-wall.md`](../verification/elf-parser-wall.md) says why. In short,
the overlap check is a loop over up to `MAX_PHNUM` (64) headers inside a loop over the same, and
Kani unrolls it by the linear 64 bound it can see. On top of that, `phoff` and `p_offset` place
reads at symbolic offsets in a symbolic array. The solver did not return in minutes even with the
header count pinned to one segment. Totality wants a loop-invariant tool such as Verus, or more of
the leaf arithmetic factored out. The tests cover the specific hostile files by example.

A near-`u64::MAX` `p_memsz` would wrap `vaddr + memsz`. Under the dev profile's overflow checks
that is a panic, which is a crafted binary halting the kernel. `Error::AddressOverflow` refuses it
before the entry check and `Segment::page_range` use the sum. `page_range` also saturates on its
own, because `Segment` is public and must be panic-free without `parse`'s guarantee.

`MAX_PHNUM` bounds the header count before the overlap check, which is quadratic. A real static
executable has a handful of headers. A count of 65,535 exists only to make validation stall. The
note reader walks the same bound.

## What it refuses that a general loader would not

- **Writable and executable together.** A page that is both is how a buffer overflow becomes code
  execution, and an ELF can simply ask for one. `paging::Flags` has no
  `writable_and_executable()` constructor for the same reason; this check stops a file talking the
  loader into building one.
- **Two segments on one page.** A general loader handles this. It is legal, and common when
  `.text` and `.rodata` share a page. This one refuses, because the tree's own linker script
  page-aligns every segment, so an overlap means something unexpected happened. The script that
  keeps it true, with the two empty-section cases that broke it, is
  `crates/user_mode_runtime/link.ld`.
- **A segment neither readable nor executable.** Nothing could ever touch it. An execute-only
  segment is accepted.
- **A foreign machine.** `Error::WrongMachine` catches a riscv binary handed to the `aarch64`
  kernel, or an `aarch64` one handed to the x86 kernel, at the parse rather than as an
  illegal-instruction fault when the program starts.

## Which machine a file may be for

The machine a build accepts, `EXPECTED_MACHINE`, is a compile-time fact. A kernel only loads
binaries for its own architecture, and the host build the tests run in accepts the host's. That
keeps the foreign-binary check without threading an expected-machine argument through the kernel
and userspace init, both of which would only pass their own architecture anyway. The `cfg` arms
name each architecture. A default arm that names one architecture becomes a trap the moment a third
exists: `not(riscv64)` also catches `x86_64`.

`KNOWN_MACHINES` lists every `e_machine` nife runs, and the other host-dependent constants are
derived from it. Adding an architecture is one edit there and one arm of `EXPECTED_MACHINE`.

- `NATIVE_MACHINE` exports the expected machine. A test that forges a header has to write some
  machine number, and the native one lets the forgery past the machine check to the property under
  test. One definition replaces a `cfg` at every such test.
- `FOREIGN_MACHINES` is `KNOWN_MACHINES` minus the native machine, derived rather than listed. A
  test that expects a forgery refused iterates it, which proves the check is symmetric: it refuses
  every nife architecture that is not this build's, on any host. A hand-written pair would be a
  statement about one laptop.
- Its length is `KNOWN_MACHINES.len() - 1`, and the asserts in `foreign_machines` make that a
  fact. An architecture that reaches `EXPECTED_MACHINE` without reaching `KNOWN_MACHINES` fails the
  build, because nothing is subtracted and the array overflows. The mirror case compiles on
  purpose. A machine in `KNOWN_MACHINES` with no `EXPECTED_MACHINE` arm builds as `aarch64` and is
  right to list the newcomer as foreign; it is wrong only once someone runs on it, and that is the
  arm they will be writing.
- `machine_no_nife_build_accepts` hands back a machine number after proving it is in none of
  `KNOWN_MACHINES`, so a `const` that names a foreign machine fails to build rather than going on
  testing nothing. It checks all three rather than only the native machine. A check against
  `EXPECTED_MACHINE` alone would fail only on the one host that has it; checking every known
  machine fails on every host, including the one where the mistake is made. A gate that fires only
  where nobody is standing is not a gate.
- `KNOWN_MACHINES` is exported for callers with a per-machine obligation of their own.
  `crates/elf/tests/fuzz_seed.rs` is one: the fuzz corpus needs a seed for every machine some build
  accepts.

## The fuzz seeds

`fuzz/fuzz_targets/elf_parse` returns at once on anything `Elf::parse` rejects. If this build's
seed stopped parsing, the fuzzer would start from an empty corpus and have to rediscover five
constants and a program-header table before reaching an interesting branch, while still reporting
"no crashes". `crates/elf/tests/fuzz_seed.rs` makes that loud for a millisecond. The other fuzz
targets seed from fixtures that already have tests of their own
(`crates/device_tree_blob/tests/qemu_aarch64_virt.rs`,
`crates/globally_unique_identifier_partition_table/tests/real_disks.rs`).

There is one seed per machine nife runs, in `fuzz/seeds/elf_parse/`. They differ at exactly one
byte, offset 18, `e_machine`. A build's fuzzer spends one rejected input on each seed for another
machine. The test picks this build's seed by the machine number in its header, not by `cfg`, so the
choice cannot disagree with the bytes and no default arm naming one architecture comes back. It
checks the seeds against `KNOWN_MACHINES`, so a missing seed fails on every host rather than only
on the one it is missing for. The seed is read-and-execute, not writable, so it passes the W^X
refusal and reaches the paths after validation. The generator is in
[`fuzz/seeds/README.md`](../../fuzz/seeds/README.md).

## Finding a note

`Elf::note`, `NoteSegments` and `NoteSearch` find one ELF note by owner and type. They exist for
milestone 597 (a program carries its manifest in an ELF note), §197 (a package is one archive
file)'s option M2, ruled by calef on 2026-09-26 (UTC).

The reader goes through the program headers, not the section headers. A linker gathers allocated
`SHT_NOTE` sections into a `PT_NOTE` segment, and the loader already walks the program-header
table to find `PT_LOAD`. So a note is one more `p_type` in a table the crate already bounds, and
the crate keeps its "program headers only" property. Section headers cost more than a second table:
`strip --strip-section-headers` and `sstrip` delete them from a working binary, so a reader that
needed them would lose the manifest to a tool that leaves the program runnable.

Nothing in the note reader runs inside `Elf::parse`, and the kernel never calls it. Only a program
that reads manifests does (the shell, the progenitor), and only on the bytes it was handed to
spawn.

### Two ways in, one rule

The progenitor holds a whole image and asks `Elf::note`. The shell streams a file into frames a
page at a time and never holds all of it. It reads the first page, finds the `PT_NOTE` headers there
with `NoteSegments::from_head`, and reads each segment's bytes on its own. Both paths walk the same
headers with the same iterator and judge every segment with the same `NoteSearch`. So "the first
copy or the last?" and "is this note malformed?" have one answer however the bytes arrived. The
streaming path adds only a limit on how much it will hold, which is the caller's and fails closed.
`NoteSearch` must be fed every segment in table order for a duplicate to be caught.

### The format, where the specification and practice disagree

Each note is three little-endian `u32`s, `namesz`, `descsz` and `type`. Then come `namesz` bytes of
owner name, NUL included, padded, then `descsz` bytes of descriptor, padded. The gABI says an
ELF64 note's words and padding are 8 bytes. No toolchain ships that. Every one writes 4-byte words
on ELF64 and 4-byte padding, except `.note.gnu.property`, which is 8-byte padded and sits in its own
`PT_NOTE` with `p_align` 8. So the padding is read from the segment's `p_align`: 0, 1 and 4 mean 4,
8 means 8, and anything else is refused. That is LLVM's `Elf_Note_Iterator` rule, which
`llvm-readelf` uses.

### What a hostile file can try

- A `namesz` or `descsz` near `u32::MAX`. The extent arithmetic is checked, and
  `verification::note_extent_never_panics` proves it total.
- A note whose padded end runs past its segment. It is refused (`NoteError::Truncated`), as LLVM
  refuses "ELF note overflows container". A lenient reader would be a second opinion about where
  the last note ends.
- A `PT_NOTE` that points outside the file, refused (`NoteError::SegmentOutOfBounds`).
- The same owner and type twice, refused (`NoteError::Duplicate`). This one is not about memory
  safety. The shell and the progenitor both read the manifest. If one took the first copy and the
  other the last, the person at the prompt would approve one manifest and the progenitor would
  endow another. Refusing the ambiguity makes "which one?" unaskable.
- An owner name with no terminating NUL where it must have one. The note does not match, since the
  comparison includes the NUL.
- Up to 64 `PT_NOTE` headers (`MAX_PHNUM`), each walked once. Each note advances the cursor by at
  least 12 bytes, which is proved, so the walk ends.
- A header table outside the bytes the reader was given, refused (`NoteError::NotReadable`).
  `verification::note_segments_from_a_head_never_panic` proves the check panic-free.

*Name: provisional, minted 2026-10-24 (UTC) by lane comments/929-crates-12, after the crate it
describes. calef names files.*
