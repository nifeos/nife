# boot_ladder

The fixed heads of the lines a kernel and its shell print while booting, and the list of self-test
check names. A program that reads a serial log, a CI transcript or a photograph of a screen then
keys on the same bytes the kernel wrote. `no_std`, no dependencies, no `unsafe`.

Each head is a system's name followed by a fixed shape, such as `<system> on ` for the first line
and `<system> self-test: ` for the verdict. The crate names no system: the caller passes its own
name to `Heads::new`, and the program that prints a line and every program that reads it take the
head from the same `Heads`, so both sides hold the same bytes. A head prints through `Display` and
matches a line in place, so neither side allocates.

## The matching rule

A head begins a line, but a serial link may put corrupted bytes in front of it. A reader therefore
finds the head anywhere in a line (`Head::find_in`) and takes the tail from there. Every head but
the shell banner ends in a space or punctuation, so a match is on a boundary. The shell banner
(`Heads::shell_banner`) is matched whole: the shell prints a period after it, and the period is not
part of the head. The architecture after the first line is free text; a reader takes it from the
tail of the machine line (`Heads::machine`).

The tail of a head may change between releases, because it carries numbers. A head never changes.

## The verdict

The self-test prints one verdict line. Its grammar is code, not prose: `verdict::Verdict` prints it
and `verdict::ParsedVerdict` reads it, so the printer and every reader agree by construction.

```text
nife self-test: 12 of 15 passed, 3 absent: block_device clock true_random_number_generator
nife self-test: 13 of 15 passed, 2 FAILED: userspace entropy
```

A verdict is green unless it carries a `FAILED:` list. An `absent:` list names checks the machine
has no device for; whether an absence is acceptable is the reader's board profile to say. The
total is `SELF_TEST_CHECKS.len()` for a kernel built from the same version, and one of
`EARLIER_SELF_TEST_CHECK_COUNTS` for an older one.

## Version policy

The Cargo version is the contract's version, and the crate carries no separate identity constant:
a console line cannot hold a checkable version without a new line to freeze. A change to a head or
to a check name bumps the minor version and adds a compatibility entry: the earlier-counts list
for a changed total, and a list of renamed checks if a name is ever renamed. A reader built
against one version reads a log from an older kernel by those entries and no other.

## EXAMPLES

Finding the verdict in a transcript, and deciding whether it is red:

```rust
use boot_ladder::{Heads, verdict::ParsedVerdict};

let heads = Heads::new("example");
let line = "\u{fffd} example self-test: 12 of 15 passed, 3 absent: clock, 1 FAILED: entropy";

// The head is found after the noise, and the tail is read through the shared grammar.
assert!(heads.self_test().is_in(line));
let verdict = ParsedVerdict::in_line(heads, line).unwrap();
assert_eq!((verdict.passed, verdict.total), (12, 15));
assert!(verdict.is_red());
assert_eq!(verdict.failed_names().collect::<Vec<_>>(), ["entropy"]);
```

Printing a head:

```rust
use boot_ladder::Heads;

assert_eq!(Heads::new("example").banner().to_string(), "example on ");
```

## What is proven

Two Kani harnesses show that a count never overflows and that a name list holds only plain names,
whatever ASCII text arrives up to their bounds. A replayable patch in `falsifications/` turns each
red. The tests cover the round trip of
the verdict, the invariants of the check list, and the shape of every head.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

## BUGS

- Nothing gates that a program uses `Heads` rather than a head spelled as a literal; review does.
  After the crate leaves its first consumer's repository, only that consumer's own lint can.
- `Heads::shell_banner` is the shell's banner, not its `$ ` prompt, which is too weak to key on in a
  log. A reader that needs the prompt waits for the banner and then looks for `$ `.
- Nothing checks that every program of one system builds its `Heads` from the same name; each
  passes its own, and a reader of that system's logs finds a mismatch as a missing rung.
- The kernel pads a check's name to ten characters in its per-check lines, and the longest names no
  longer line up in that column.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
