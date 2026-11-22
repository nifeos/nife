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

Three rulings have already spelled them out, each for one package, and they do not agree:

- `user_mode_runtime` (ruled 2026-11-20, built in #3856): parameters `word_zero` to `word_two`,
  `result_word_zero` and `result_word_one`, and `argument_zero` to `argument_two` for a system
  call's arguments.
- `byte_sink_contract` (ruled 2026-11-21, built in #3880): fields `word_zero`, `word_one` and
  `word_two` of `MessageWords`, chosen to match `user_mode_runtime`.
- `domain_name_resolution_contract` (ruled 2026-11-19, milestone 1086 (`domain_name_resolution_contract`:
  what an outside maintainer would fix before it leaves)): parameters `first_word` and
  `second_word`, ratified for that crate only, with other contracts to follow when their names come
  up.

So the tree now writes one concept four ways. This decision picks one convention for every package,
so each later name review applies it rather than choosing again.

## The decision

What nife calls a word of a message and a result word of a system call, in identifiers and in
prose, across nife, its `std` overlay and the kernel.

## Recommendation: (a), the zero-based position as a digit

- A message word is `word_0` to `word_5`, by its zero-based position in the tables of notes/abi.md.
- A result word, including a `CALL`'s reply, is `result_word_0` to `result_word_2`.
- A system call's argument that is not a message word is `argument_0` and onward.
- Once the code decodes a word, the value takes the name of what it is (`offset`, `handle`),
  as design/naming.md already requires.
- Prose uses the same names in backticks, and the wire tables in notes/abi.md list the positions
  as `word_0` to `word_5` beside the registers that carry them.
- design/naming.md gains one bullet stating this, and `w0`..`w5`, `r0`..`r2` join the
  `[spelled_out]` list so the naming check refuses them in new code.

The reason is consistency with the analogous case the tree has already ruled. The tree's ratified
names for numbered slots and instances use a digit: `RECEIVE_DESCRIPTOR_WORD_3_PACKET_LENGTH_BYTES_MASK`
(milestone 1074 (`designware_ethernet`: what an outside maintainer would fix before the move),
ruled 2026-11-14), the `receive_descriptor_word_3` parameter beside it,
`phase_locked_loop_0` and `post_divider_1`. A descriptor word and a message word are the same kind
of thing: a word whose position a table defines and whose meaning the table gives per position.

This reopens two rulings made this week (`user_mode_runtime` on 2026-11-20, `byte_sink_contract`
on 2026-11-21). Neither record shows that anyone weighed a digit: both reviews say only that the
names are spelled out, and a digit is not an abbreviation.

Would I still choose this if every option cost the same? Yes. The costs are close: (a) renames
what (b) renames plus about 100 lines that already say `word_zero`. The case for (a) is one
spelling for a numbered position across the tree, not effort.

## Three choices inside one name

The name of a word answers three separate questions. The options below combine the answers.

### Where the count starts

Zero-based. notes/abi.md numbers the words from 0, the registers that carry them are `x0` and
`a0` onward, and the Ethernet databook numbers descriptor words `RDES0` to `RDES3`. A one-based
name (as Linux writes `arg1`) would put every position one off from the table that defines it.

### Digit or spelled number

`word_0` or `word_zero`. This is the open question, and the tree points both ways:

- For the digit: every ratified name in the tree that numbers a slot or an instance uses a digit
  (the four names above), and names not yet reviewed do the same (`mainline_port_1`). The digit
  matches the table, the register (`x0`) and the descriptor words.
- For the spelled number: the two rulings of this week chose it, and about 100 lines in 40 files
  already carry it. Outside the `word_`, `argument_` and `result_word_` families, a spelled number
  in an identifier is a value or a count, never a position (`is_zero`, `is_power_of_two`,
  `fires_at_two`).
- Neither form abbreviates, and design/naming.md has no rule on digits in names.

The descriptor-word case is the closer precedent: a ratified public name for a numbered word of a
fixed layout, made because a gate refused the specification's abbreviation. The spelled rulings
came from parameter reviews of two packages and set no tree-wide rule.

### The prefix: `result_` with no `request_`

A message word carries no prefix and a result word carries `result_`. The asymmetry follows what
each name has to cover.

- A message word is the same word at both ends. The sender passes `word_1` to `SEND` or `CALL`,
  and the receiver reads `word_1` from `RECEIVE`, in a `Message` that holds it beside `badge` and
  `label`. A direction prefix would have to change at the kernel boundary. A `request_` prefix would also misname most
  messages: a `SEND` is not a request and expects no reply (the reason `byte_sink_contract` gave for
  replacing `req`), and a death message comes from the kernel.
- A result word comes back from a system call, in the result registers, and only some calls are
  `CALL`s. `reply(to, result_word_0, result_word_1)` names what the caller of `call` receives; the
  same prefix covers `badge`'s new slot and every other return. A `reply_` prefix would fit `CALL`
  and misname the rest.
- `RECEIVE` hands back `word_0` in the result register. The name follows what the value is, the
  message's first word, not the register it lands in. The receiver names it `word_0`.

Fuchsia is the prior art for a matched pair. `zx_channel_call_args_t` names the outgoing message
`wr_bytes` and `wr_handles` and the reply `rd_bytes` and `rd_handles`, with `rd_num_bytes` for the
reply's length: write and read, from the caller's side. That pair works because one struct
describes one call from one side. nife's word names appear on both sides of a rendezvous, so a
pair named from one side would read backwards at the other. A matched
`message_word_0` would lengthen every message word for symmetry a bare `word_0` does not need.

## Is the premise true

Checked on `main` at `nifeos/nife@830de0623`: design/naming.md and design/naming/ contain no
`w0`..`w5` or `r0`..`r5`. The names are live in the kernel (39 code uses in 8 files), the `std`
overlay (82 in 9 files) and 23 other packages. `user_mode_runtime`'s `Message` and `Request` carry
public fields `w0`, `w1` and `w2`, which the ruling of 2026-11-20 left out.

## The count

All counts are of lowercase identifiers with an underscore-separated segment from `w0`..`w5` or
`r0`..`r5` (`w1`, `carries_w1`, `r0_after`, `_w1`), on `main` at `830de0623`, excluding `vendor/`.
The headline numbers come from one command:

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

- Code identifiers are 1189 message-word uses (`w0` 540, `w1` 363, `w2` 222) and 399 result-word
  uses (`r0` 296), at 593 declaration sites.
- The public API is 61 names: 55 `pub fn` parameters in 16 files (mostly contract decoders, and
  the `std` overlay's `rt`), the `pub` fields of `user_mode_runtime`'s `Message` and `Request`,
  and `filesystem_contract`'s `Verb::carries_w1`.
- 25 packages carry code uses; `components`, `system_tests`, `filesystem_contract`,
  `user_mode_runtime` and `fixtures` hold most.
- About 15 uses in 3 files reuse the spelling for something else, and foreign names stay: AArch64
  `w0`..`w30`, x86 `r8`..`r15`, the SD card's `R1` response. No option renames them.
- Outside the tree, `clock_contract` has 4 prose uses in 1 file.

## The options

Counted from the table above. All of them start from zero.

### (a) Digit: `word_0`, `result_word_0`, `argument_0` (recommended)

As set out under the recommendation.

- Changes: everything (b) changes, plus about 100 lines in 40 files that say `word_zero`,
  `result_word_zero` or `argument_zero` today. Outside the tree, `MessageWords`'s three public
  fields change in `byte_sink_contract`, which has no tag yet. About 300 files in all.
- Reopens: the `user_mode_runtime` and `byte_sink_contract` parameter and field rulings, and the
  `domain_name_resolution_contract` one.
- Keeps: the descriptor-word and clock-tree names exactly.

### (b) Spelled number: `word_zero`, `result_word_zero`, `argument_zero`

The spelling two packages carry now.

- Changes: 1588 code uses in 148 files, 602 prose uses in 131 files, 16 falsification patches
  regenerated, and `first_word` and `second_word` in `domain_name_resolution_contract` (about 30
  uses in 5 files). About 260 files, and `clock_contract` in its own repository.
- Keeps: the `user_mode_runtime` and `byte_sink_contract` rulings exactly.
- Costs: the tree then numbers a message word one way and a descriptor word another, for the same
  kind of thing. A later reader asks why `word_three` and `descriptor_word_3` differ, and the only
  answer is the order the reviews happened in.

### (c) Ordinal: `first_word`, `second_word`

The spelling `domain_name_resolution_contract` ratified for itself.

- It reads naturally for two words and less well for six (`sixth_word` against `word_5`), and it
  is one-based in effect: `second` names position 1.
- It reopens the two rulings that agree with each other instead of the one that does not.

### (d) Name every word by its meaning, per verb

`destination_handle(w1: u64)` would become `destination_handle(packed: u64)` or similar, and
`offset` would replace `w1` wherever the code means a `READ`'s second word.

- It fails where the code is generic. The `std` overlay's `rt::send(slot, w0, w1, w2)` and
  `user_mode_runtime`'s `send(slot, word_zero, word_one, word_two)` carry words with no meaning of
  their own, and a word such as `READ`'s first one packs three values, so no single meaning can
  name it.
- Where the code has decoded a value, (d) is already the rule, and (a) and (b) keep it.

### (e) Keep `w0`..`w5`, `r0`..`r2` and define them in design/naming.md

- Changes: design/naming.md gains a definition and the abbreviations join the stay list. No code
  changes, unless calef reverts the three package rulings, which would touch the 40 files that
  spell the words out today plus `domain_name_resolution_contract`.
- Costs: two spellings for one concept, or three recent rulings undone. The stay list is for
  terms whose expansion nobody says (§154 (the acronym test is whether the phrase is spoken,
  applied recursively)); people say "word zero" aloud, so `w0` does not pass that test.
- Cheapest, and the only option whose case is effort.

### (f) Hybrid: keep `w0`..`w5` in the register-level modules, spell out elsewhere

The modules where a word is one register: `user_mode_runtime`'s `system_call.rs` and
`inter_process_communication.rs`, and the `std` overlay's `rt.rs`.

- Changes: 1437 code uses in about 145 files, and prose as in (a).
- Keeps 151 code uses in those 3 files. But `user_mode_runtime`'s parameters there are already
  spelled out, so (f) would leave locals and public fields in the old spelling beside parameters
  in the new.
- The register already has a name, `x0` or `a0`; a third spelling adds a word to remember.

### What happens to `byte_sink_contract`'s `first_word`

`byte_sink_contract` has a ratified function `first_word(operation: u8, len: u32)` that packs a
message's first word, beside `operation(word_zero)` and `len(word_zero)` that unpack it. Under every
option the function keeps its name. The convention governs names that stand for a position: a
parameter, a field, a local, a word in prose. A function's name says what it returns, and
`first_word` says it in plain English, as `end_of_stream()` does. Its parameters already carry
meanings. Under (a) its doc says it builds `word_0` and the unpackers' parameter becomes `word_0`;
under (b) nothing in it changes.

The function has no callers in nife: the crate calls it in its own codec and tests, and nife's
components use `pack` and `unpack`. The two `first_word` names in nife are unrelated and stay.
`kernel/src/user/disk_service.rs`'s `HolderWiring::first_word` reads the first word of a block
roster frame, and `xtask/src/swish_check/gauge.rs`'s `first_word` binds the first word of a
template's text. Neither names a message word.

## Prior art

Read for this decision unless marked otherwise.

- RISC-V SBI binary encoding (`riscv-sbi-doc`, `binary-encoding.adoc`): the specification names
  arguments by register, `a0` to `a5`, with the function in `a6` and the extension in `a7`. The two results are a
  struct named by meaning, `struct sbiret { long error; union { long value; unsigned long uvalue; }; }`,
  in `a0` and `a1`.
- Linux `syscall(2)` (man7.org): the calling-convention tables name the positions `arg1` to `arg7`,
  one-based, and the results `Ret val`, `Ret val2` and `Error`, then map each to a register per
  architecture (riscv `a0`..`a5`, arm64 `x0`..`x5`).
- seL4 (`libsel4`, `functions.h`): words are message registers, reached by a zero-based index with
  `seL4_GetMR(int i)` and `seL4_SetMR(int i, seL4_Word mr)`. I recall, and did not read, that the
  seL4 manual writes a message register as `MR0` onward.
- Fuchsia (`zx_channel_call` reference): a message is bytes and handles, and nothing names a word
  at all. The call's parameters take names by meaning: `handle`, `options`, `deadline`, `args`,
  `actual_bytes`, `actual_handles`. Inside `args`, a matched pair names the two directions:
  `wr_bytes` and `wr_handles` for the message out, `rd_bytes`, `rd_handles` and `rd_num_bytes` for
  the reply.
- Redox (`redox_syscall` 0.9.5, read from the local cargo registry): the raw calls take single
  letters, `syscall4(a, b, c, d, e)`. The scheme protocol's `Sqe` holds `opcode`, `tag`,
  `args: [u64; 6]` and `caller`, and `Cqe` holds a `result`: an indexed array for the words and a
  meaning name for the answer. I recall an older `Packet { a, b, c, d }` and did not read it.

Where nife differs: like seL4, a nife message is positional words whose meaning each contract sets,
so a position needs a name, which Fuchsia's byte messages never do. Unlike seL4 and Redox, nife
spells its names out, so nife writes the word and its position (`word_0`) where seL4 indexes `MR`
and Redox writes `a`. The result side follows SBI and Linux in naming the answer for what it is,
a result.

## A concrete example

`filesystem_contract`'s `MKDIR` today:

```rust
/// ... `w1` is the requested [`super::directory`] mask, or ...
/// Reply `r0` = the new directory handle, or an error.
pub const fn destination_handle(w1: u64) -> u64 { ... }
```

Under (a):

```rust
/// ... `word_1` is the requested [`super::directory`] mask, or ...
/// Reply `result_word_0` = the new directory handle, or an error.
pub const fn destination_handle(word_1: u64) -> u64 { ... }
```

and at the server, once decoded: `let destination = fs::destination_handle(word_1);`. Under (b)
the same lines say `word_one` and `result_word_zero`.

## Effort, stated plainly

(a) and (b) are mechanical renames, but neither is a one-line `sed`. About 10 files already hold
both `w1` and `word_one` (a `MessageWords` field beside a local), and under either option both
become one name, so each of those needs reading for a shadowed name. (a) adds one step: a pull
request to `byte_sink_contract` renames `MessageWords`'s fields, and nife bumps its pin after it
lands. rustfmt rewraps the lines that grow. The 16 falsification patches quote the old
lines, so the lane regenerates and replays each one. The Kani harnesses and mutation baselines name
functions, not these parameters, so they should not move; the rename lane checks that. One lane,
in a few large commits by area, after the in-flight renames of the same files land.

## How reversible it is

A two-way door today. nife has published no crate, a Rust parameter's name is not part of a crate's
compatibility promise, and nothing outside the tree reads these names. The public struct fields
(`Message::w0` and the rest, `Verb::carries_w1`, `MessageWords::word_zero`) become one-way at each crate's first release, which
is why the convention should come before the release reviews reach them. The wire format does not
change under any option.

## What waits on this

- The rename lane for (a) or (b), or the naming.md bullet for (e).
- The name reviews of every contract that still writes `w0` (`filesystem_contract`,
  `credential_contract`, `terminal_contract`, `program_table`'s spawn protocol and the rest), which
  would otherwise each choose again.
