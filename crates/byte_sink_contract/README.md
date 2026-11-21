# byte_sink_contract

[![check](https://github.com/nifeos/byte_sink_contract/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/byte_sink_contract/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/byte_sink_contract/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/byte_sink_contract/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/byte_sink_contract#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/byte_sink_contract/blob/main/Cargo.toml)

One framing for "write these bytes there".

Up to sixteen bytes ride in three 64-bit message registers, with an opcode and a count in the first
word, so a terminal, a file and a pipe can sit behind one capability slot.

A writer packs its bytes with `pack`, sends the three words, and ends the stream with one
end-of-stream message. A sink decodes each message with `unpack`, which refuses anything the framing
does not define. The crate is `no_std`, has no dependencies and is dual licensed `MIT OR
Apache-2.0`. Kani proves it: every property it claims is a harness in the tree, and every harness
carries a falsification patch that shows what turns it red.

## EXAMPLES

Pack a line, decode each message as a sink would, and end the stream.

```rust
use byte_sink_contract::{MAXIMUM_INLINE_BYTES, Message, MessageWords, end_of_stream, pack, unpack};

let line = b"the quick brown fox\n"; // 20 bytes: two messages and an end of stream
let mut received = [0u8; 32];
let mut got = 0;

let mut rest = &line[..];
while !rest.is_empty() {
    let (words, taken) = pack(rest);
    rest = &rest[taken..]; // the count says how much `pack` took, so no tail is lost silently

    let mut buffer = [0u8; MAXIMUM_INLINE_BYTES];
    match unpack(words, &mut buffer) {
        Message::Bytes(count) => {
            received[got..got + count].copy_from_slice(&buffer[..count]);
            got += count;
        }
        Message::EndOfStream | Message::Malformed => unreachable!(),
    }
}

let end = MessageWords::new(end_of_stream(), 0, 0);
assert_eq!(unpack(end, &mut [0u8; MAXIMUM_INLINE_BYTES]), Message::EndOfStream);
assert_eq!(&received[..got], line);
```

Tell a delivered message from a missing sink and from a destroyed one by the return of the send.

```rust
use byte_sink_contract::{GONE, Sent, classify};

assert_eq!(classify(0), Sent::Ok);
assert_eq!(classify(GONE), Sent::Gone); // the sink was destroyed: the writer must end
```

## What is proven

Six Kani harnesses state the framing for every input:

- `any_prefix_survives_pack_then_unpack`: any prefix of up to sixteen bytes comes back unchanged,
  with a count equal to its length.
- `unpack_never_answers_more_than_a_message_holds`: for any three words, the decoder is total and
  never reports more bytes than a message carries.
- `severity_reads_back_clamped_and_leaves_the_rest`: a severity written to a first word reads back
  clamped, and the count, the opcode and the kind keep their values.
- `classify_separates_the_three_outcomes`: the destroyed-sink return is `Gone` and only that one,
  and a non-negative return is `Ok`.
- `an_end_of_stream_decodes_exactly_from_its_one_message`: `unpack` answers end of stream if and
  only if the words are the end-of-stream message, with both reserved words zero.
- `end_of_stream_is_no_bytes_message`: the end-of-stream first word equals no bytes message of any
  length.

Each harness has a falsification record at `falsifications/<module>.<harness>.patch`. Applying it
with `git apply -p1` from the crate root makes that harness fail at its own assertion, which shows
the proof checks something. The patches need a git checkout, and you revert each one with
`git checkout -- src` before the next.

Not proven: that a particular sink acts on end of stream, or that a writer sends zero in the
reserved words. The decoder refuses a non-zero reserved word; the writer's side is a convention.

## Build, test and prove

You need a Rust toolchain at or above the minimum version at the foot of this page (`rustup` is the
usual way to get one) and a git checkout. From the crate root:

- `cargo build` and `cargo test` run on the host and need nothing else. The tests are unit tests
  and doctests.
- The crate also builds for bare metal. Add the three targets with `rustup target add
  riscv64imac-unknown-none-elf aarch64-unknown-none-softfloat x86_64-unknown-none`. Then run
  `cargo build --lib --target <target>` for each.
- The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
  kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
  checker, so it needs network access. After that, `cargo kani` runs all six harnesses in about
  twenty seconds and about 600 megabytes of memory on one development machine, most of it
  compiling. A passing run prints a long list of checks, some marked `UNREACHABLE` or `SUCCESS` for
  standard library code. It also prints `internal error: entered unreachable code` descriptions,
  which are checks proved unreachable. Only the final `VERIFICATION:- SUCCESSFUL` and the line
  `Complete - 6 successfully verified harnesses` matter. To run one harness, use `cargo kani
  --harness verification::classify_separates_the_three_outcomes --exact`.

### Running one falsification patch

```sh
patch -p1 --no-backup-if-mismatch < falsifications/verification.classify_separates_the_three_outcomes.patch
cargo kani --harness verification::classify_separates_the_three_outcomes --exact   # ends VERIFICATION:- FAILED
git checkout -- src
```

The failing run names the assertion that a return of zero is `Ok`.

## BUGS

- The contract has no operation that asks a sink for its version. A sink is one-way, so a program
  declares `INTERFACE_ID` and `VERSION` where its parent starts it, and whoever wires it compares them.
- `MessageWords` keeps a provisional name until the release name review.
- The severity field's width and its plus-one encoding are provisional.

## Minimum Rust version

Rust 1.88. Check it with `rustup toolchain install 1.88 --profile minimal`, then `cargo +1.88
test`, and the bare-metal build with the same `+1.88`.

## License

MIT OR Apache-2.0, at your option: see `LICENSE-MIT` and `LICENSE-APACHE`.
