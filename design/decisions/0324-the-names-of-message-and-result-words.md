---
status: PROPOSED
raised: 2026-11-22
---

# 324. The names of a message's words and a system call's result words

*(The section number and title are provisional until the merge lands them.)*

## Background

A nife message is a handful of 64-bit words carried in registers. `SEND` and `CALL` take up to
three, a plain `RECEIVE` hands back five (the fifth written only by the kernel), and a dead child's
label rides in a sixth. A system call answers in one or two result words, and a `CALL`'s reply is
two result words. Each contract decides what its words mean: in `filesystem_contract`, the first
word of a `READ` packs the operation, the handle and the byte count, and the second word is the file
offset. [notes/abi.md](../../notes/abi.md) lays the words out by position.

The tree names those positions `w0` to `w5` (the words of a message) and `r0` to `r2` (the result
words), in doc prose and as parameter, local and field names. Nothing defines them:
[design/naming.md](../naming.md) never mentions them, and its rule that every name the tree authors
spells out its abbreviations, parameters included, would refuse them if they were on its list.

Three package rulings spelled them out, each for one package:

- `user_mode_runtime` (ruled 2026-11-20, built in #3856): `word_zero` to `word_two`,
  `result_word_zero` and `argument_zero`.
- `byte_sink_contract` (ruled 2026-11-21, built in #3880): the fields `word_zero` to `word_two` of
  `MessageWords`.
- `domain_name_resolution_contract` (ruled 2026-11-19): the parameters `first_word` and
  `second_word`.

On 2026-11-22 calef ruled that spelled numbers (`word_zero`) and ordinals (`first_word`) are wrong
in identifiers, and that the tree must not permit them. That ruling refuses all three package
spellings and leaves two candidates: a digit (`word_0`), or no numbered name at all, with the words
held in an array (`words[0]`).

## The decision

How nife names the words of a message and the result words of a system call, in identifiers and
in prose, across nife, its `std` overlay and the kernel.

## Recommendation: (g), an array of words, and names by meaning once decoded

- The message words a call sends or a receive delivers are an array: `send(slot, words: [u64; 3])`,
  `call(slot, words: [u64; 2])`, `Message { words: [u64; 3], badge, label, delivered }`.
- Result words are never an array. The first result word is always signed (an error, or a value),
  so results are a tuple whose first element is `i64`, as SBI's `sbiret` is a struct:
  `call(..) -> (i64, u64)`, `reply(to, results: (i64, u64))`, and `survey` keeps
  `(i64, u64, u64)`. The 11 local `reply` helpers that take `r0: i64` already follow this.
- A value the code has decoded takes the name of what it is, as design/naming.md already requires:
  `let (handle, length) = call(slot, request);`, or `let [operation, offset, _] = message.words;`.
- A function that decodes one word takes a parameter named by meaning, or plain `word`.
- In prose and in notes/abi.md, `words[i]` and `results[i]` are notation for position `i` of the
  message table and the result table. They name a position, not a variable: a decoder that takes
  `word` documents that it reads `words[1]` of a `MKDIR`, and no `results` identifier exists.
- `w0`..`w5` and `r0`..`r2` join the `[spelled_out]` list.

The reasons are on the merits. An array leaves no numbered identifier to rule on, so the tree needs
no convention for one. The compiler checks the position: `words[3]` on a `[u64; 3]` fails to build,
where a name `word_3` is only a name. And the tree already works this way. The trap
boundary is `raw_system_call(number, words: [u64; 6]) -> [u64; 6]` and `trap6(number, a: [u64; 6])`,
and the kernel's saved registers are `x: [u64; 31]`. A parameter `words: [u64; N]` appears 16 times
in 12 files, three of them in `const fn` contract decoders.

Would I still choose this if both options cost the same? Yes, and they do not: (g) changes
signatures and call sites where (a) renames, so (g) is the dearer option. The recommendation is not
about effort.

## Where the count starts

Zero-based, under either candidate. notes/abi.md numbers the words from 0, the registers that carry
them are `x0` and `a0` onward, and a Rust index starts at 0.

## Is the premise true

Checked on `main` at `nifeos/nife@830de0623`: design/naming.md and design/naming/ contain no
`w0`..`w5` or `r0`..`r5`. The names are live in the kernel (39 code uses in 8 files), the `std`
overlay (82 in 9 files) and 23 other packages.

## The count

All counts are of lowercase identifiers with an underscore-separated segment from `w0`..`w5` or
`r0`..`r5` (`w1`, `carries_w1`, `r0_after`, `_w1`), on `main` at `830de0623`, excluding `vendor/`:

```sh
P='(?<![A-Za-z0-9_.])(?:[a-z0-9]+_)*_?(?:[wr][0-5])(?:_[a-z0-9]+)*(?![A-Za-z0-9_])'
git grep -I -o -P "$P" 830de0623 -- ':!vendor' ':!bench' ':!*.s' ':!*.S' ':!*.svg' | wc -l   # 2245
git grep -I -l -P "$P" 830de0623 -- ':!vendor' ':!bench' ':!*.s' ':!*.S' ':!*.svg' | wc -l   # 257
```

The split comes from a heuristic classifier posted on the pull request, so each row may be off by
a few uses. It also counts the excluded paths as foreign, so its rows add up to more than 2245.

| Kind | Uses | Files |
|---|---:|---:|
| Use of a local or a parameter | 963 | 142 |
| Local binding (`let`, a pattern, a closure parameter) | 467 | 120 |
| Parameter of a private function | 63 | 29 |
| Parameter of a `pub fn` (public API) | 55 | 16 |
| Field in a struct literal or a pattern | 32 | 14 |
| `pub` struct field (public API) | 6 | 2 |
| Private struct field | 2 | 2 |
| **Code identifiers, all** | **1588** | **148** |
| Rust doc comments | 310 | 58 |
| Markdown prose | 202 | 58 |
| Markdown code blocks | 60 | 11 |
| String literals (assertion and panic text) | 30 | 11 |
| **Doc prose, all** | **602** | **131** |
| Falsification patches and a metrics file, which quote the code above | 47 | 16 |
| Foreign: aarch64 assembly, disassembly in notes, bench logs, SVG namespace URLs | 108 | 9 |

- Code identifiers are 1183 message-word uses and 405 result-word uses. `survey_record` and
  `virtual_address_space_list` bind `let (r0, w1, w2, ..)`, all three result words (8 uses), and a
  `program_table` test binds a request's first word as `r0` (2 uses).
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

## The options

### (a) Digit: `word_0`, `result_word_0`, `argument_0`

Every position keeps a name, numbered as the tables number it. The tree's ratified names for a
numbered word of a fixed layout already use a digit: `RECEIVE_DESCRIPTOR_WORD_3_PACKET_LENGTH_BYTES_MASK`
(milestone 1074 (`designware_ethernet`: what an outside maintainer would fix before the move),
ruled 2026-11-14) and the parameter `receive_descriptor_word_3`.

- Changes: 1588 code uses in 148 files, 602 prose uses in 131 files, the 16 falsification patches,
  the 130 spelled-out lines, and `MessageWords`'s three fields in `byte_sink_contract`. About 300
  files, and `clock_contract`'s prose.
- Keeps: every signature's shape. It is a rename, with the smallest diff of the two candidates.
- Costs: a positional name at all 319 sites in the table above, which the compiler cannot check
  against the length, and a convention a reviewer applies by hand.

### (b) Spelled number: `word_zero`

Refused by the ruling of 2026-11-22: a spelled number in an identifier.

### (c) Ordinal: `first_word`

Refused by the ruling of 2026-11-22: an ordinal in an identifier.

### (d) Name every word by its meaning, per verb

It fails where the code is generic. `send` carries words with no meaning of its own, and `READ`'s
first word packs three values. Where the code has decoded a value, (d) is already the rule, and
(a) and (g) both keep it.

### (e) Keep `w0`..`w5`, `r0`..`r2` and define them in design/naming.md

Cheapest, and the only option whose case is effort. The stay list holds terms whose expansion
nobody says (§154 (the acronym test is whether the phrase is spoken, applied recursively)), and
people say "word zero" aloud, so `w0` fails that test.

### (f) Hybrid: keep `w0`..`w5` in the three register-level modules

They hold 151 code uses. The register already has a name, `x0` or `a0`, so a third spelling adds
a word to remember.

### (g) An array of words (recommended)

As set out under the recommendation. Where an array fits badly:

- **`Message` is not all words.** `x0` to `x2` are the sender's, `x3` the badge, `x4` the kernel's
  account of what arrived and `x5` a label. Only the first three form the array; the rest keep
  names by meaning, so the array index matches the register only up to 2.
- **The first result word is signed.** So results are a tuple, never an array, as set out above.
  `call` and `reply` change type from `u64` to `i64` for that word.
- **The kernel writes `x4`.** It is never a sender's word, so it stays outside the array as
  `delivered`.
- **A capability moves the words.** `receive_capability` documents its return as
  `(w0, capability_slot, w1)`: the slot is in `x1` and the sender's second word in `x2`. Its body
  binds `let (w0, w1, w2, ..)`, so the local `w1` holds `x1`, the slot, while the doc's `w1` is the
  sender's word. A reply carrying a capability uses another order: the kernel writes
  `[msg[0], msg[1], delivered, 0, 0]` (kernel/src/sched/rendezvous.rs, line 885), so the slot is
  in `x2`. An index would not survive either trip, so each function returns the slot by name beside
  the words. The current names hide both shifts.
- **`const fn` decoders.** Not a problem: constant indexing builds in `const fn`, and three
  `const fn` decoders take an array already.
- **Code generation.** I compiled `send`, `call` and `receive` both ways, with each
  architecture's `trap6` copied in, using the script posted on the pull request. Callers'
  instruction counts, digits against arrays:

  | Build | aarch64 | riscv64 | x86_64 |
  |---|---:|---:|---:|
  | `opt-level=3`, wrappers inlined | identical | identical | identical |
  | `opt-level=3`, not inlined | 22 / 27 | 27 / 36 | 25 / 29 |
  | `opt-level=0` (debug), not inlined | 44 / 51 | 39 / 49 | 42 / 51 |

  Not inlined, a `[u64; 3]` argument travels by pointer and a `[u64; 2]` result through memory.
  The wrappers carry no `#[inline]` and the release profile sets no LTO, so release callers pay
  this today. `script/board-image` packs debug programs, and `user_mode_runtime` builds there at
  `opt-level=0`. Even with the wrappers forced `#[inline(always)]`, the aarch64 debug caller grows
  from 178 to 192 instructions and from a 624-byte to a 688-byte frame. That cost stays: a debug
  image pays a few instructions per call for (g). The rename lane marks the wrappers `#[inline]`
  and checks a release build's assembly before and after.

## The follow-on: a check that refuses spelled numbers and ordinals

The ruling says the tree must not permit these spellings, so the naming check gains a rule: refuse
any identifier with a segment that is a spelled number (`zero` to `twelve`) or an ordinal (`first`
to `tenth`). Measured on Rust code outside `vendor/`, comments and strings blanked, on `main` at
`08d17a3b0`, by a script posted on the pull request:

| Hit | Names | Uses |
|---|---:|---:|
| A segment is a spelled number | 708 | 1231 |
| ... and it ends the name | 132 | 378 |
| A segment is an ordinal | 283 | 885 |
| ... and the only ordinal is `first` | 167 | 595 |

- **Positions.** Beyond the `word_*`, `argument_*` and `result_word_*` names this decision replaces,
  the hits include `domain_one` and `domain_two`, `view_one`, `slot_one`, `rows_two`,
  `window_zero`, `fs_window_zero`, `wire_disk_zero`, `bus_device_function_zero` and
  `double_word_three`. They also include the fixture constants `GLOB_ONE`, `RM_ONE`,
  `REDIRECTION_ONE` and `LABEL_ONE` with their `_TWO` partners, and `mainline_port_one` inside a
  test name. Ordinals
  include `first_partition`, `second_region`, `second_directory_grant`,
  `first_ethernet_port_transmit_parent`, both `first_word` and `second_word` families, the
  kernel's `HolderWiring::first_word` and a `first_word` in `xtask`'s swish check.
- **Test names written as sentences.** 544 of the 708 spelled-number names, and 125 ordinal names,
  have six or more segments (`a_serviced_note_names_its_exact_length_and_pads_to_four`).
- **Quantities.** Predicates and amounts number nothing: `is_power_of_two`, `ONE_GIBIBYTE`,
  `spawn_one` (spawn a single one). A pattern for predicates and units covers 11 of the 132
  ending names; once the sentence-style test names are set aside, 52 names (279 uses) remain to
  judge one at a time.
- **`second` the unit of time.** 44 names (154 uses) say `NANOSECONDS_PER_SECOND` and the like. That
  is not an ordinal, so the rule allows `second` after `per`.

The ruling refuses ordinals, so the rule refuses `first_` everywhere, including the 167 names where
it marks the start of a range or the earliest event (`first_logical_block_address`,
`first_error`). Permitting that sense would narrow the ruling, and that is calef's to decide, as a
question of its own. Two scope questions also stay open, each separate: whether the rule covers
instance names (`domain_one`, `first_partition`, `second_region`, `second_directory_grant`,
`first_ethernet_port_transmit_parent`), and whether it covers test names written as sentences.
The size of the quantity allowance follows from those answers.

## Prior art

Read for this decision unless marked otherwise.

- seL4 (`libsel4`, `shared_types.h` and `functions.h`): the IPC buffer holds
  `seL4_Word msg[seL4_MsgMaxLength]`, and `seL4_GetMR(int i)` returns `seL4_GetIPCBuffer()->msg[i]`.
  Message words are an indexed array. I recall, and did not read, that the seL4 manual writes a
  message register as `MR0` onward.
- Linux (`include/asm-generic/syscall.h` and `arch/arm64/include/asm/syscall.h`):
  `syscall_get_arguments(task, regs, args)` fills `args`, an "array filled with argument values",
  six long. The arm64 version reads `args[0]` from `regs->orig_x0`, because the return value has
  overwritten `x0`. That is (g)'s risk in one line: index 0 can hide that position 0 is special.
  The `syscall(2)` man page names the same positions `arg1` to `arg7`, one-based.
- RISC-V SBI (`binary-encoding.adoc`): arguments by register, `a0` to `a5`; results a struct named
  by meaning, `struct sbiret { long error; union { long value; unsigned long uvalue; }; }`. That is
  the model for (g)'s typed results.
- Redox (`redox_syscall` 0.9.5, read from the local cargo registry): the scheme protocol's `Sqe`
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

## Effort, stated plainly

(a) is a mechanical rename. About 10 files hold both `w1` and `word_one`, so each needs reading for
a shadowed name, and rustfmt rewraps the lines that grow. (g) is larger: the 197 group sites change
shape, every caller of `send`, `call` and `receive` changes, and the single-word sites still need
names by meaning chosen one at a time. Under both, `byte_sink_contract` changes first in its own
repository and nife bumps its pin, and the 16 falsification patches are regenerated and replayed.
The Kani harnesses and mutation baselines name functions, not parameters; the lane checks that.
One lane, in commits by area, after the in-flight renames of the same files land.

## How reversible it is

A two-way door today. nife has published no crate, and nothing outside the tree reads these names.
The public fields (`Message::w0` and the rest, `Verb::carries_w1`, `MessageWords::word_zero`) and
the shapes of `send` and `call` become one-way at each crate's first release. The wire format does
not change under any option.

## What waits on this

- The rename lane for (a) or (g).
- The naming check above, and its separate question on instance names.
- The name reviews of every contract that still writes `w0` (`filesystem_contract`,
  `credential_contract`, `terminal_contract`, `program_table`'s spawn protocol and the rest).
