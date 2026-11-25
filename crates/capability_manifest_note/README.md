# capability_manifest_note

[![check](https://github.com/partsfoundry/capability_manifest_note/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/partsfoundry/capability_manifest_note/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/partsfoundry/capability_manifest_note/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/partsfoundry/capability_manifest_note/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/partsfoundry/capability_manifest_note#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/partsfoundry/capability_manifest_note/blob/main/Cargo.toml)

The ELF (Executable and Linkable Format) note a program carries to ask for authority, the
compile-time encoder that writes it and the strict decoder that reads it. `no_std`, no allocation,
no `unsafe` code of its own.

In a capability system a program starts with nothing, and something has to decide what to hand
it: a memory grant, a file, a directory, the clock, the network. A manifest note is the program's
own statement of what it needs, kept inside the executable behind a `PT_NOTE` program header. It
sits in the bytes a loader hashes, so the request cannot change without the digest changing. The
program states its needs as a constant `grant_contract::Manifest`. The `carry!` macro has the compiler
encode that constant into the note, and a loader or a shell decodes the note before it grants
anything. Nothing in a note grants authority: it is a request, and the reader decides.

The crate makes one claim, in both directions: **one manifest has exactly one encoding.** What the
compiler accepts into a program's bytes, the reader accepts, and what the reader refuses, the
compiler refuses. So two readers cannot disagree about what a note meant, and a program cannot
ship a note its own readers will refuse.

## What it does

A manifest note is an ordinary ELF note. Its header is the three 4-byte words `namesz`, `descsz` and
`type`, then the owner string `OWNER` and its terminating null (NUL) byte, padded to 8 bytes, then
the descriptor. The owner string is the name of the operating system that defines these notes, as
`FreeBSD`, `GNU` and `FDO` are the owners of theirs. Note types are numbered per owner, so the three
types below collide with no one else's. The `interfaces_note` crate's note, which lists the binary
interfaces a program links, has the same owner and header, and this crate's tests hold the two to
the same bytes.

| type | constant | carried by | descriptor |
|---|---|---|---|
| 1 | `MANIFEST` | every program that asks for authority | a manifest, in one of three versions |
| 2 | `SUBTREE_GRANTS` | a file server that enforces subtree grants itself | a version word, then the scope crate it enforces them through |
| 3 | `VERBS` | a program whose words after its name each get their own grants | a sorted table of verbs and the grants each one is given |

Every descriptor opens with a little-endian version word. A reader refuses a version it does not
know (`Error::UnknownVersion`) and never skips past one, because a later version may mean a
request the reader would grant wrongly.

### The manifest descriptor, three versions

Version 1 (`BARE_VERSION`) is exactly `DESCRIPTOR_BYTES` (56) bytes: the version word, then one
byte or one 8-byte word per field of `grant_contract::Manifest`, at fixed offsets. The table in the
crate documentation (`cargo doc --open`) gives every offset and value.

Version 2 (`ZONED_VERSION`) is version 1's 56 bytes with `2` in the version word, then the
resolver zones the program asks for: a count from 1 to `MAXIMUM_ZONES` (8), then each zone as a
length byte and that many bytes of dotted lowercase ASCII, at most `MAXIMUM_ZONE_BYTES` (253). A
length of 0 is the root zone. The zones are sorted and each appears once, so one request has one
spelling, and a manifest that asks for no zone is version 1, never a version 2 with a count of 0.

Version 3 (`SERVICED_VERSION`) is version 2's layout with the count allowed to be 0 and one more
byte after the zones: `1` when the program asks to run as a service, else `0`. It exists beside
version 2, not in place of it, so that the notes of programs already built keep their bytes: a
manifest that asks for zones and no service is still version 2, and its digest does not change.
The service ask is the one thing only version 3 can say.

Some authorities have no byte at all. A grant that only the owner of the machine may hand out
(rebooting it, installing software, the package catalog) cannot be asked for in a note, so `encode`
fails the build on a manifest that asks for one. A program that wants one gets it from the owner
directly, and a note can never be the way it got it.

### The verb table

A `VERBS` descriptor is a version word, a count word and `count` records of `VERB_RECORD_BYTES`
bytes each. A record holds the verb, zero-padded, then the bits of the grants that verb is given,
then two zero bytes. The records are sorted by the verb's bytes and each verb appears once, which is
what makes one table one encoding and what lets `Verbs::grants_for` search it. A table holds at
most `MAXIMUM_VERBS` (256) verbs: room for a program the size of git, which has 175 subcommands in
2.56.0, and a bound on how much a malformed count can make a reader walk.

## EXAMPLES

Encode a manifest at compile time and read it back:

```rust
use capability_manifest_note::{
    DESCRIPTOR_BYTES, Error, asks_service, decode, encode, encode_zoned, zoned_descriptor_bytes,
    zones,
};
use grant_contract::{
    DirectoryGrantDeclaration, FileGrantDeclaration, IndexStoreDeclaration, InputDeclaration,
    Manifest, MemoryGrantDeclaration, NO_SHORT_OPTIONS, OutputDeclaration,
    ProgramArgumentDeclaration, StartupContract,
};

// What the program asks for: bytes out, between 4 and 64 pages of memory, and the clock. Every
// field is stated, and the ones it does not ask for are forbidden or false.
const ASKS: Manifest = Manifest {
    program_argument: ProgramArgumentDeclaration::Forbidden,
    memory_grant: MemoryGrantDeclaration::Required { minimum: 4, maximum: 64 },
    file: FileGrantDeclaration::Forbidden,
    directory: DirectoryGrantDeclaration::Forbidden,
    short_options: NO_SHORT_OPTIONS,
    output: OutputDeclaration::Bytes,
    input: InputDeclaration::Forbidden,
    reports: true,
    interruptible: false,
    clock: true,
    domain: false,
    configuration: false,
    entropy: false,
    network: false,
    machine: false,
    share: false,
    reboot: false,
    sync: false,
    installer: false,
    catalog: false,
    indexes: IndexStoreDeclaration::Forbidden,
    activation_directory: false,
    console_input: false,
    startup: StartupContract::Native,
};

// The compiler builds the descriptor. A manifest with no spelling fails the build here.
const DESCRIPTOR: [u8; DESCRIPTOR_BYTES] = encode(&ASKS);
assert_eq!(decode(&DESCRIPTOR), Ok(ASKS));

// A later version is refused, not skipped.
let mut later = DESCRIPTOR;
later[0] = 4;
assert_eq!(decode(&later), Err(Error::UnknownVersion));

// The same ask with two resolver zones: the root and one domain, sorted.
const ZONES: &[&str] = &["", "example.net"];
const ZONED: [u8; zoned_descriptor_bytes(ZONES)] = encode_zoned(&ASKS, ZONES);
assert_eq!(decode(&ZONED), Ok(ASKS));
let asked: Vec<&[u8]> = zones(&ZONED).unwrap().iter().collect();
assert_eq!(asked, [b"".as_slice(), b"example.net"]);
assert_eq!(asks_service(&ZONED), Ok(false));
```

Carry the note in a program, with one line at module scope:

```rust,ignore
capability_manifest_note::carry!(ASKS);
capability_manifest_note::carry!(ASKS, zones = &["", "example.net"]);
capability_manifest_note::carry!(ASKS, zones = &["example.net"], service = true);
```

Use one of the three forms, once: a second use is a duplicate symbol. The macro places a `Note`,
`ZonedNote` or `ServicedNote` in a named section of the program's data. Rust emits it as an
ordinary data section, so the program's linker script must put that section in a `PT_NOTE` segment;
`carry!`'s source names the section. A build that cannot use the macro can write `Note::of(&ASKS)`
into an object file of its own and link that.

To read a note, use any ELF reader to find the `PT_NOTE` entry whose owner is `OWNER` and whose
type is `MANIFEST`, then pass its descriptor to `decode`, `zones` and `asks_service`. A note of type
`VERBS` goes to `decode_verbs`, and one of type `SUBTREE_GRANTS` to `decode_subtree_grants`.

## What is proven

Kani, a model checker for Rust, proves these over every input in its bound:

- Whatever `decode` accepts as a version 1 descriptor, `encode` spells back byte for byte.
- Whatever `encode` builds from any manifest the type allows, `decode` reads back as the same
  manifest.
- `encode` refuses a memory grant whose minimum is above its maximum.
- `decode` never panics on any input up to one byte past version 1's length, nor `decode`, `zones`
  and `asks_service` through a version 2 or 3 descriptor of six zone layouts, at every length up to
  62 bytes.
- `grants_for` finds every verb `decode_verbs` accepts in a table of one or two verbs of one or two
  bytes, with the grants the table gives it.
- `decode_subtree_grants` never panics on any input up to one byte past its 8 bytes.

Every harness carries a falsification patch in `falsifications/`, a small defect that turns the
harness red. Unit tests and doctests cover the rest: each way to respell a zoned descriptor is
refused, a cut descriptor of every version is `Error::WrongLength`, and the encoders fail the
build on a zone the resolver would not spell and on an inverted memory grant.

## Building and testing

```sh
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none
cargo build --lib --target aarch64-unknown-none-softfloat
cargo build --lib --target riscv64imac-unknown-none-elf
```

The crate has no features, so a plain `cargo test` runs every unit test and doctest.

### Running the proofs

Install Kani first: run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. Setup downloads a model checker and the Rust toolchain Kani is pinned to, so it
needs network access. Then run every harness:

```sh
cargo kani
```

To run one harness, give its full path, because `--exact` refuses a bare name:

```sh
cargo kani --harness verification::decode_never_panics --exact
```

A passing run prints warnings that mean nothing, among them `Found the following unsupported
constructs` and many `aborting path on assume(false)` lines, which are paths Kani discards on
purpose. The answer is the last line, `Complete - N successfully verified harnesses, 0 failures, N
total.`, and `cargo kani` exits 0.

### Running one falsification patch

Each file in `falsifications/` applies a small defect that turns its harness red. Apply one,
confirm the proof fails, then undo it with the same patch reversed:

```sh
patch -p1 --no-backup-if-mismatch < falsifications/verification.decode_never_panics.patch
cargo kani --harness verification::decode_never_panics --exact
patch -R -p1 --no-backup-if-mismatch < falsifications/verification.decode_never_panics.patch
```

The failing run reports the failed check and ends with `Complete - 0 successfully verified
harnesses, 1 failures, 1 total.`, and `cargo kani` exits 1. Each patch's opening lines name the
defect and the check it expects to fail.

## BUGS

- `llvm-objcopy --add-section` makes no program header, so a note added to a finished binary that
  way is not in a `PT_NOTE` and no reader finds it. A build that cannot use `carry!` must link an
  object that holds the note.
- Changing a manifest means relinking the program, and the program's digest changes with it. That
  is the cost of keeping the request inside the hashed bytes.
- Kani's bound stops short of the longest zoned descriptor. The zone walk past one zone is covered
  by unit tests and fuzzing, not by a proof.

## Minimum Rust version

Rust 1.88. To check it, install the toolchain and its bare-metal targets, then run the tests and
all three bare-metal builds with `+1.88`:

```sh
rustup toolchain install 1.88 --profile minimal
rustup target add --toolchain 1.88 x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo +1.88 test
cargo +1.88 build --lib --target x86_64-unknown-none
cargo +1.88 build --lib --target aarch64-unknown-none-softfloat
cargo +1.88 build --lib --target riscv64imac-unknown-none-elf
```

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
