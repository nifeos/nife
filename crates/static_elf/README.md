# static_elf

The front half of a program loader: it parses a static, little-endian ELF64 executable (`ET_EXEC`)
for one machine, checks everything a loader needs before it maps a page, and hands back the
segments to map. It reads program headers only, never section headers. It also finds one note in
a `PT_NOTE` segment, over a whole image or one segment at a time. It is `no_std`, needs no
allocator and has no dependencies.

It does not relocate, link or run anything. A position-independent executable, a shared object or
a file that names an interpreter is refused, not loaded. A reader that must accept every
well-formed ELF file wants a general reader such as `object` or `goblin`.

## EXAMPLES

`examples/parse_static_executable.rs` is the whole loop. `cargo run --example
parse_static_executable -- <file>` prints what a loader would map, or the reason for refusing.
The core of it:

```rust,ignore
use static_elf::{Elf, PageBytes};

let elf = Elf::parse(&bytes)?; // every check has run
for segment in elf.segments() {
    let (first, end) = segment.page_range(PageBytes::FOUR_KIBIBYTES);
    // Map [first, end) with the segment's permissions, copy `segment.data` to
    // `segment.virtual_address` and zero the rest of `segment.memory_bytes`.
}
// Start the program at `elf.entry()`.
```

`Elf::parse` accepts this build's own machine (`NATIVE_MACHINE`). `Elf::parse_for_machine` names
another one, and `Elf::parse_with` also names the page size the overlap check rounds to.

### Finding a note

`Elf::note(owner, kind)` returns the descriptor of the one note with that owner and type, or
`None`. Two copies of the same note are refused, since a reader that took the first and another
that took the last would disagree about the file:

```rust,ignore
let build_id = elf.note(b"GNU", 3)?; // NT_GNU_BUILD_ID
```

A reader that holds one segment at a time feeds each `PT_NOTE` segment, in table order, to
`NoteSearch::segment`. `NoteSegments::from_head` lists those segments from the first bytes of a
file.

## What it refuses, and why

The parse refuses a malformed file: one that is too short or has the wrong magic, class, byte
order, version or machine, or whose headers or segments run past the end of the file. It also
refuses five things that are well-formed ELF. They are this loader's policy, with no setting to
turn one off:

| refusal | why |
|---|---|
| `WritableAndExecutable` | A page that is writable and executable is how a buffer overflow becomes code execution. |
| `SegmentsOverlap` | A loader that gives each segment its own permissions cannot give one page two. A linker script for this loader page-aligns every segment. |
| `EntryNotExecutable` | An entry outside the file bytes of every executable segment cannot start, or starts on zeros. |
| `TooManyProgramHeaders` | More than 64 headers. The overlap check is quadratic in their number, and 64 bounds it. |
| `NeedsRelocation`, `NotExecutable` | Anything but a static `ET_EXEC` expects a dynamic linker or is no program. |

`Error` and `NoteError` print as a sentence and implement `core::error::Error`.

## What it does not check

None of these fields is read. For a loader that copies file bytes into fresh pages, none matters.

- `EI_OSABI` and `EI_ABIVERSION`: the operating system the file was built for.
- `e_flags`, which on RISC-V carries the float ABI and whether compressed instructions are used.
  A loader for one ABI checks it itself.
- `e_ehsize`. The parse reads the fixed ELF64 header size.
- `p_align` of a `PT_LOAD`, and the congruence `p_vaddr mod p_align == p_offset mod p_align`. A
  loader that maps file pages directly, rather than copying them, must check the congruence
  itself.
- `PT_TLS`, `PT_GNU_STACK` and every other segment type but `PT_LOAD` and `PT_NOTE`.
- Section headers, of any kind.

## Building and testing

`cargo build` and `cargo test` need no setup beyond a Rust toolchain at the minimum version below.
`cargo test` runs the unit tests, which forge hostile files by hand, the documentation examples and
a test that the seeds in `tests/fixtures/` still parse. The crate is `no_std`, so it builds for the
bare-metal targets `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
`x86_64-unknown-none` once `rustup target add` has installed each one.

### What is proven

Nine Kani harnesses. Five in `src/verification.rs` prove the per-segment bounds check panic-free,
in bounds and free of address overflow, a program-header offset a whole header inside the table,
and `Segment::page_range` panic-free and ordered for every power-of-two page size. Four in
`src/note.rs` prove the note reader panic-free and in bounds. Totality of `Elf::parse` over every
byte string is past bounded model checking. The tests cover the specific hostile files by example.

The proofs need Kani, which `rustup` does not install. Run `cargo install --locked kani-verifier`,
then `cargo kani setup`, which downloads a model checker. `cargo kani` runs them all, and `cargo
kani --harness <name>` runs one.

Each harness has a falsification record at `falsifications/<module path>.<harness>.patch`.
Applying it with `patch -p1 < <file>` from the crate root makes that harness fail at its own
assertion, which shows the proof checks something. `patch -R -p1 < <file>` takes it out again.

## BUGS

- `Elf::parse` checks overlap over `p_vaddr`, not `p_paddr`. A boot loader that places segments
  by physical address must check that their physical ranges are disjoint itself.
- The note reader reads a `p_align` of 0 or 1 as 4-byte padding for any segment. LLVM's reader
  accepts 0 only for an empty segment and refuses 1. No common producer writes either value.
- The machine list is three machines: `aarch64`, `riscv64` and `x86_64`.
- The API may still change before the first release.

## Minimum Rust version

Rust 1.88.

## License

You may use this crate under the MIT license (LICENSE-MIT) or the Apache License, version 2.0
(LICENSE-APACHE), at your option.
