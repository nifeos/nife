# byte_glob

An `fnmatch(3)`-style matcher for one file name. A pattern and a name go in, a `bool` comes out. It
is a total function on two byte strings: `no_std`, no allocation, no `unsafe`, no dependency, a cost
bound a caller can compute before matching, and six Kani proofs of its corners.

Three things a caller does not guess:

- Bytes, not characters. The match is `fnmatch(3)` in the C locale, so `?` matches one byte,
  which is half of a two-byte UTF-8 character.
- One name, not a path. `/` is an ordinary byte, so `matches(b"*.txt", b"dir/notes.txt")` is
  `true`. A caller that matches a path splits it first. Recursive descent (`**`) belongs to the
  layer that walks directories, because descending is an authority, not a match.
- A leading `.` needs a literal `.` by default (`LeadingDot::Special`), the reading that grants less.
  `*` does not match `.config`; `[.]config` does not either, because a bracket expression is a
  wildcard however few members it has. `.` and `..` are ordinary names to the matcher, so `.*`
  matches both. The layer that enumerates a directory decides whether to offer them.

A malformed pattern means itself: an unterminated `[` is a literal `[`, and a trailing `\` is a
literal `\`. There is no error case, because a pattern comes from outside and an error path for
every typo is a cost. There are no `**`, no zsh-style qualifiers (they need type, time and size per
candidate, which is more authority than listing a directory) and no POSIX (Portable Operating System Interface) classes.

## The contract

- `matches(pattern, name)` and `matches_with(pattern, name, dot)`: does the pattern match the name.
  `LeadingDot::Special` is the default; `LeadingDot::Ordinary` treats a leading `.` as any other byte.
- `has_magic(pattern)` and `literal(pattern, out)`: is the word a pattern at all, and if not, what
  name is it with the escapes stripped. `literal` returns `None` rather than truncate.
- `match_steps(pattern, name, dot)` and `step_bound(pattern_bytes, name_bytes)`: the cost of a
  match, and the most it can be, computed from the two lengths alone. `match_steps` is at most
  `step_bound`; its exact count is not promised across versions.

Syntax: `*` any run of bytes, `?` one byte, `[abc]` a set, `[a-z]` a range, `[!abc]` or `[^abc]` the
complement, `\x` the byte `x`, anything else itself.

## EXAMPLES

```rust
use byte_glob::{matches, matches_with, has_magic, literal, LeadingDot};

assert!(matches(b"*.txt", b"notes.txt"));
assert!(!matches(b"*", b".hidden"));
assert!(matches_with(b"*", b".hidden", LeadingDot::Ordinary));
assert!(matches(b"*.txt", b"dir/notes.txt")); // one name: `/` is a byte

assert!(has_magic(b"*.txt"));
let mut buf = [0u8; 16];
assert_eq!(literal(br"a\*b", &mut buf), Some(&b"a*b"[..]));
```

## What is proven

Six Kani harnesses (`src/verification.rs`), each with a falsification patch that turns it red.
They quantify over every byte value of patterns and names up to three bytes (2^48 pairs per
harness) but not over length: four bytes each did not finish. Matching is total under
both dot rules. The step count never exceeds `step_bound`. A pattern with no magic matches exactly
itself. `*` and `?` have their base meanings. The dot rule touches only names that start with a
dot. A negated class is the exact complement of its class. The length-independent claims are
host tests: an exhaustive cross-check against a naive matcher over every pattern of up to five
bytes and every name of up to three, and a 100,000-byte adversarial run held under the bound.

## How it avoids blowup

The matcher keeps exactly one backtrack point, the most recent `*`, which is the algorithm
`fnmatch` implementations use to avoid the exponential split a recursive matcher tries. A retry
position advances monotonically, so a match makes at most `name_bytes + 1` backtracks, and between
two of them the name position plus the pattern position strictly increases. The work is polynomial
in the two lengths with no term in the number of `*`s. `has_magic` and `literal` are not counted by
`match_steps`: each decodes a `[` by scanning to its closing `]`, so `n` unterminated brackets cost
about `n * n / 2` byte reads.

## Corners

Every row is a place a real implementation has been wrong, and a test pins it.

| input | answer | why |
|---|---|---|
| `[foo` | literal `[foo` | no closing `]`, so it was never a bracket expression |
| `[]]` | matches `]` | `]` right after `[` or `[!` is a member |
| `[a-]` | matches `a` and `-` | a `-` with nothing after it is a member, not a range |
| `[-a]` | matches `-` and `a` | the same rule at the other end |
| `[z-a]` | matches nothing | a reversed range is empty, the answer that cannot grant more |
| `[\]]` | matches `]` | POSIX has no escapes in brackets; glibc and bash honor one |
| `a\` | matches `a\` | a trailing backslash has nothing to escape |
| empty pattern | matches only the empty name | |
| `[[:alpha:]]` | matches `a]`, `:]`, ... | no POSIX classes: the class `[[:alpha:]` then a literal `]` |

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
cargo kani
```

## BUGS

- The harnesses do not quantify over length.
- `match_steps` returns a tuple whose second value has no named type.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
