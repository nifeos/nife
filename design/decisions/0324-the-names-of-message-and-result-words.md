---
status: DECIDED
raised: 2026-11-22
decided: 2026-11-22
ratified_by: calef
---

# 324. The names of a message's words and a system call's result words

*(The section number and title are provisional until the merge lands them. Ruled 2026-11-22 (UTC).)*

## Background

A nife message is a handful of 64-bit words carried in registers. `SEND` and `CALL` take up to
three, a plain `RECEIVE` hands back five (the fifth written only by the kernel), and a dead child's
label rides in a sixth. A system call answers in one or two result words, and a `CALL`'s reply is
two result words. Each contract decides what its words mean: in `filesystem_contract`, the first
word of a `READ` packs the operation, the handle and the byte count, and the second word is the file
offset. [notes/abi.md](../../notes/abi.md) lays the words out by position.

The tree names those positions `w0` to `w5` (the words of a message) and `r0` to `r2` (the result
words), in doc prose and as parameter, local and field names. design/naming.md requires every name
the tree authors to spell out its abbreviations, parameters included, so these names are refused.
Spelled numbers (`word_zero`) and ordinals (`first_word`) are refused in identifiers too, and the
tree must not permit them.

## The rule

How nife names the words of a message and the result words of a system call, in identifiers and in
prose, across nife, its `std` overlay and the kernel:

- The message words a call sends or a receive delivers are an array: `send(slot, words: [u64; 3])`,
  `call(slot, words: [u64; 2])`, `Message { words: [u64; 3], badge, label, delivered }`.
- Result words are never an array. The first result word carries a sign, a negative error or a
  value, so results are a tuple whose first element is `i64`, as SBI's `sbiret` is a struct:
  `call(..) -> (i64, u64)`, `reply(to, results: (i64, u64))`, and `survey` keeps `(i64, u64, u64)`.
- No numbered word name exists. A value the code has decoded takes the name of what it is, as
  design/naming.md requires: `let (handle, length) = call(slot, request);`, or
  `let [operation, offset, _] = message.words;`.
- A function that decodes one word takes a parameter named by meaning, or plain `word`.
- In prose and in notes/abi.md, `words[i]` and `results[i]` are notation for position `i` of the
  message table and the result table. They name a position, not a variable.
- The thin wrappers `send`, `call` and `receive` carry `#[inline]`, so the generated code matches
  what a digit-named form would produce (the measurements are below).
- Counting starts at zero: notes/abi.md numbers the words from 0, the registers that carry them are
  `x0` and `a0` onward, and a Rust index starts at 0.
- `w0`..`w5` and `r0`..`r2` join the `[spelled_out]` list.

The reasons: an array leaves no numbered identifier to rule on, so the tree needs no convention for
one. The compiler checks the position: `words[3]` on a `[u64; 3]` fails to build, where a name
`word_3` is only a name. And the tree already works this way. The trap boundary is
`raw_system_call(number, words: [u64; 6]) -> [u64; 6]` and `trap6(number, a: [u64; 6])`, and the
kernel's saved registers are `x: [u64; 31]`. A parameter `words: [u64; N]` appears 16 times in 12
files, three of them in `const fn` contract decoders.

This costs more than the refused digit form, since it changes signatures and call sites where a
digit renames. The rule is chosen on the merits, not on effort.

### Where an array fits badly, and what the rule does there

- **`Message` is not all words.** `x0` to `x2` are the sender's, `x3` the badge, `x4` the kernel's
  account of what arrived and `x5` a label. Only the first three form the array; the rest keep
  names by meaning, so the array index matches the register only up to 2.
- **A signed first word.** Four local `reply` helpers take `r0: i64` (`fs_file_caretaker`,
  `fs_nameset_caretaker`, `fs_subtree_caretaker`, `redoxfs_server`); other servers cast
  `r0 as u64` on 21 lines in 16 files. The clock's `page.read().state` and the resolver's
  `outcome.word()` are non-negative and pass as an `i64` unchanged, and `Outcome::from_word` takes
  an `i64` and refuses a negative one.
- **The kernel writes `x4`.** It is never a sender's word, so it stays outside the array as
  `delivered`.
- **A capability moves the words.** `receive_capability` returns the slot (in `x1`) by name beside
  a two-word array holding the sender's words. A reply carrying a capability uses another order:
  the kernel writes `[msg[0], msg[1], delivered, 0, 0]` (kernel/src/sched/rendezvous.rs), so the
  slot is in `x2`. An index would not survive either trip, so each function returns the slot by
  name beside the words.
- `const fn` decoders. Constant indexing builds in `const fn`, and three decoders take an array
  already.

### Code generation: why the wrappers carry `#[inline]`

Measured by compiling `send`, `call` and `receive` both ways, with each architecture's `trap6`
copied in and `call` returning `(i64, u64)`. Callers' instruction counts, digits against arrays:

| Build | aarch64 | riscv64 | x86_64 |
|---|---:|---:|---:|
| `opt-level=3`, wrappers inlined | identical | identical | identical |
| `opt-level=3`, not inlined | 22 / 27 | 27 / 35 | 25 / 30 |
| `opt-level=0` (debug), not inlined | 44 / 49 | 39 / 46 | 42 / 47 |

Not inlined, a `[u64; 3]` or `[u64; 2]` argument travels by pointer. Inlined, both forms compile
to identical code, so the `#[inline]` on the wrappers is what makes the array form free in a
release build, and a change that removes it is a code-size and speed regression. The release
profile sets no LTO. `script/board-image` packs debug programs, and `user_mode_runtime` builds
there at `opt-level=0`. Even with the wrappers forced `#[inline(always)]`, the aarch64 debug caller
grows from 178 to 188 instructions and from a 624-byte to a 672-byte frame, a cost a debug image
pays for the rule.

## The count (before the rule)

All counts are of lowercase identifiers with an underscore-separated segment from `w0`..`w5` or
`r0`..`r5` (`w1`, `carries_w1`, `r0_after`, `_w1`), on `main` at `830de0623`, excluding `vendor/`:

```sh
P='(?<![A-Za-z0-9_.])(?:[a-z0-9]+_)*_?(?:[wr][0-5])(?:_[a-z0-9]+)*(?![A-Za-z0-9_])'
git grep -I -o -P "$P" 830de0623 -- ':!vendor' ':!bench' ':!*.s' ':!*.S' ':!*.svg' | wc -l   # 2245
git grep -I -l -P "$P" 830de0623 -- ':!vendor' ':!bench' ':!*.s' ':!*.S' ':!*.svg' | wc -l   # 257
```

A heuristic classifier, posted on the pull request, made the split; a row may be off by a few
uses. It counts the excluded paths as foreign, so its rows add up to more than 2245.

| Kind | Uses | Files |
|---|---:|---:|
| Use of a local or a parameter | 963 | 142 |
| Local binding (`let`, a pattern, a closure parameter) | 467 | 120 |
| Parameter of a private function | 63 | 29 |
| Parameter of a `pub fn` (public API) | 55 | 16 |
| Field in a struct literal or a pattern | 32 | 14 |
| `pub` struct field (public API) | 6 | 2 |
| Private struct field | 2 | 2 |
| Code identifiers, all | 1588 | 148 |
| Rust doc comments | 310 | 58 |
| Markdown prose | 202 | 58 |
| Markdown code blocks | 60 | 11 |
| String literals (assertion and panic text) | 30 | 11 |
| Doc prose, all | 602 | 131 |
| Falsification patches and a metrics file, which quote the code above | 47 | 16 |
| Foreign: aarch64 assembly, disassembly in notes, bench logs, SVG namespace URLs | 108 | 9 |

- Code identifiers are 1183 message-word uses and 405 result-word uses.
- The public API is 61 names: 55 `pub fn` parameters in 16 files, the `pub` fields of `Message`
  and `Request`, and `filesystem_contract`'s `Verb::carries_w1`.
- About 130 further lines spell the words out (`word_zero`, `argument_zero`, `first_word`), and the
  2026-11-22 ruling refuses them under either candidate.
- Foreign names stay: AArch64 `w0`..`w30`, x86 `r8`..`r15`, the SD card's `R1` response.

### What (g) does to the declarations

A second classifier, posted on the pull request, sorted the Rust declaration sites. It counts a
tuple pattern as one group however many names it binds (`let (r0, _) = call(..)`), and drops 8
bindings that hold no word (`let r2 = memory_region::create(8)` is a region).

| Declaration | Sites | Names | Under (g) |
|---|---:|---:|---|
| Function taking several words | 30 | 65 | an array, or a result tuple |
| Tuple pattern | 161 | 300 | an array or tuple pattern by meaning |
| Struct field | 6 in 2 files | 6 | a `words` array beside fields by meaning |
| Function taking one word | 52 | 52 | `word`, or a name by meaning |
| Binding of one word | 70 | 70 | a name by meaning |

The first three rows hold a group: 197 sites. The last two, 122 sites, never held one. Under (a)
all 319 carry a digit; under (g) none needs a positional name, because a word with no meaning yet
stays inside its array.

## Refused options

- **A digit: `word_0`, `result_word_0`, `argument_0`.** Refused. Every position would keep a name,
  numbered as the tables number it, and the diff is the smallest of the candidates (a rename of
  about 300 files). But it puts a positional name at all 319 declaration sites in the table above,
  which the compiler cannot check against the length, and it leaves a numbering convention that a
  reviewer applies by hand. The tree's numbered names for a fixed layout
  (`RECEIVE_DESCRIPTOR_WORD_3_PACKET_LENGTH_BYTES_MASK`, milestone 1074 (`designware_ethernet`: what an outside maintainer would fix before the move)) describe a hardware
  descriptor, not a message.
- **A spelled number: `word_zero`.** Refused: a spelled number in an identifier.
- **An ordinal: `first_word`.** Refused: an ordinal in an identifier.
- **Name every word by its meaning, per verb.** It fails where the code is generic: `send` carries
  words with no meaning of its own, and `READ`'s first word packs three values. Where the code has
  decoded a value it is already the rule, and the array form keeps it.
- **Keep `w0`..`w5` and `r0`..`r2` and define them in design/naming.md.** Refused. The stay list
  holds terms whose expansion nobody says (section 154), and people say "word zero" aloud.
- **Keep `w0`..`w5` in the three register-level modules.** Refused. The register already has a
  name, `x0` or `a0`, so a third spelling adds a word to remember.

## The follow-on: a check that refuses spelled numbers and ordinals

The ruling says the tree must not permit these spellings, so the naming check gains a rule. Three
further rulings of 2026-11-22 set its scope: the check refuses positions and instances, and allows
range boundaries and quantities.

- `first` and `last` stay only as a range or sequence boundary, as in std's `slice::first` and
  UEFI's `FirstLBA`. The check refuses `first_` or `last_` before a position noun (`word`,
  `register`, `argument`, `parameter`, `result`, `field`, `operand`), and any ordinal past
  `first`, except `second` after `per`. A name whose sense the check cannot tell carries a
  reviewed exemption.
- No ordinal tells instances of one kind apart, tests and locals included: not `second_region`,
  `second_boot` or `first_ethernet_port_transmit_parent`. The replacement names the role
  (`remounted`, `after_reboot`, `parent`); where no role exists, `_0` and `_1`.
- Number words that state a quantity stay: `twice`, `once`, `half`, `power_of_two`, and a count
  before a noun (`two_halves`, `one_tree`).

Measured on Rust code outside `vendor/`, comments and strings blanked, on `main` at `08d17a3b0`,
by a script posted on the pull request:

| Hit | Names | Uses |
|---|---:|---:|
| A segment is a spelled number | 708 | 1231 |
| ... and it ends the name | 132 | 378 |
| An ordinal past `first`, `per_second` excepted | 109 | 177 |
| `first_` or `last_` before a position noun | 8 | 43 |
| `first` or `last` otherwise, allowed as a boundary | 283 | 993 |
| `second` after `per` (`NANOSECONDS_PER_SECOND`), allowed | 7 | 113 |

- Positions. Beyond the `word_*`, `argument_*` and `result_word_*` names this decision replaces,
  the spelled hits include `domain_one`, `slot_one`, `rows_two`, `window_zero`,
  `bus_device_function_zero`, the fixture constants
  `GLOB_ONE`, `RM_ONE`, `REDIRECTION_ONE` and `LABEL_ONE`, and `mainline_port_one` in a test name.
- The position-noun pattern. Its 8 hits include `first_word` (the resolver's word 0) and
  `HolderWiring::first_word`, refused, and `split_first_word` and `paint_last_word` in the line
  editor, where `word` is a word of text and the sense is a boundary: those take exemptions.
- Instances. The 109 later ordinals are the instance sweep; 61 are test names written as
  sentences (`a_second_bind_of_a_bound_space_is_refused`). `first` in an instance sense
  (`first_partition`) hides among the 283 boundary names, so the sweep reads those by hand.
- Quantities, allowed. 106 names (289 uses) use a number as an amount: `is_power_of_two`,
  `ONE_GIBIBYTE`, `split_once`, `draw_twice`. The check passes them.

## Prior art

I read each source below for this decision.

- seL4 (`libsel4`, `shared_types.h` and `functions.h`): the IPC buffer holds
  `seL4_Word msg[seL4_MsgMaxLength]`, and `seL4_GetMR(int i)` returns `seL4_GetIPCBuffer()->msg[i]`.
  Message words are an indexed array.
- Linux (`include/asm-generic/syscall.h` and `arch/arm64/include/asm/syscall.h`):
  `syscall_get_arguments(task, regs, args)` fills `args`, an "array filled with argument values",
  six long. The arm64 version reads `args[0]` from `regs->orig_x0`, because the return value has
  overwritten `x0`. That is (g)'s risk in one line: index 0 can hide that position 0 is special.
- RISC-V SBI (`binary-encoding.adoc`): arguments by register, `a0` to `a5`; results a struct named
  by meaning, `struct sbiret { long error; union { long value; unsigned long uvalue; }; }`. That is
  the model for (g)'s typed results.
- Redox (`redox_syscall` 0.9.5): the scheme protocol's `Sqe`
  holds `opcode`, `tag`, `args: [u64; 6]` and `caller`; `Cqe` holds a `result`. An array for the
  words beside names by meaning, the shape (g) gives `Message`.

Where nife differs: seL4, Linux and Redox index the words but keep single-letter or numbered names
at the register edge. Under (g), nife's position appears only as an index, never as a name.

## A concrete example

`filesystem_contract`'s `MKDIR` today:

```rust
/// ... `w1` is the requested [`super::directory`] mask, or ...
/// Reply `r0` = the new directory handle, or an error.
pub const fn destination_handle(w1: u64) -> u64 { ... }
```

Under (g):

```rust
/// ... `words[1]` is the requested [`super::directory`] mask, or ...
/// The reply's `results[0]` is the new directory handle, or an error.
pub const fn destination_handle(word: u64) -> u64 { ... }
```

and at the server: `let destination = fs::destination_handle(message.words[1]);`. Under (a) the
parameter is `word_1` and the reply's word is `result_word_0`.

## Effort

The 197 group sites change shape, and every caller of `send`, `call` and `receive` changes. The
typed first word reaches 113 `let (..) = call(` bindings and 112 `reply(` calls in 43 files. The
single-word sites need names by meaning, chosen one at a time. `byte_sink_contract` changes first
in its own repository and nife bumps its pin, and the lane regenerates and replays the 16
falsification patches. The Kani harnesses and mutation baselines name functions, not parameters;
the lane checks that. One lane, in commits by area, after the in-flight renames of the same files
land. The naming check's instance sweep is a second lane: 109 names (177 uses), and a reading of the
283 `first` and `last` names. The implementation sweep is the proposed milestone
[design/roadmap/proposals/message-words-are-arrays.md](../roadmap/proposals/message-words-are-arrays.md).

## How reversible it is

A two-way door today. nife has published no crate, and nothing outside the tree reads these names.
The public fields (`Message::w0` and the rest, `Verb::carries_w1`, `MessageWords::word_zero`) and
the shapes of `send` and `call` become one-way at each crate's first release. The wire format does
not change.

## What waits on this

- The implementation sweep above.
- The naming check above.
- The name reviews of every contract that still writes `w0` (`filesystem_contract`,
  `credential_contract`, `terminal_contract`, `program_table`'s spawn protocol and the rest).
