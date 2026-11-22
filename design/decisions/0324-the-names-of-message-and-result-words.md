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

## Recommendation: (a), spell the position out, in the words already ruled

- A message word is `word_zero` to `word_five`, by its zero-based position, which is the spelling
  `user_mode_runtime` and `byte_sink_contract` already use.
- A result word, including a `CALL`'s reply, is `result_word_zero` to `result_word_two`, the spelling
  `user_mode_runtime` already uses for `reply`.
- A system call's argument that is not a message word stays `argument_zero` and onward, as ruled.
- Once the code decodes a word, the value takes the name of what it is (`offset`, `handle`, `operation`).
  That is design/naming.md's general rule and most of the tree already does it; this section states it
  so nobody reads (a) as license to pass `word_one` around once its meaning is plain.
- Prose uses the same names in backticks (`word_one`), so a reader meets one spelling in the
  documentation and in the signature beneath it. A wire table in notes/abi.md lists the positions
  as `word_zero` to `word_five`, beside the registers that carry them on each architecture.
- design/naming.md gains one bullet stating this, and `w0`..`w5`, `r0`..`r2` join the
  `[spelled_out]` list so the naming check refuses them in new code.

The reason is elegance more than effort: one spelling for one concept, the same in the
documentation and in the code it documents, and already the spelling two packages carry. The two
rulings that agree stand, and only `first_word` and `second_word` change.

Would I still choose this if every option cost the same? Yes. Option (c) costs the least and loses
on that test, because it leaves the tree writing one concept two or more ways.

## Is the premise true

Checked on `main` at `nifeos/nife@830de0623`:

- design/naming.md and design/naming/ contain no `w0`..`w5` or `r0`..`r5`
  (`git grep -c -P '\b[wr][0-5]\b' -- design/naming.md design/naming` prints nothing).
- The names are live in every layer the convention reaches: the kernel (39 code uses in 8 files),
  the `std` overlay (82 code uses in 9 files), and 23 other packages.
- `Message` and `Request` in `user_mode_runtime` carry public fields `w0`, `w1` and `w2`. The
  ruling of 2026-11-20 ratified the struct names and left the field names out.

## The count

All counts are of lowercase identifiers with an underscore-separated segment from `w0`..`w5` or
`r0`..`r5` (`w1`, `carries_w1`, `r0_after`, `_w1`), on `main` at `830de0623`, excluding `vendor/`.
The headline numbers come from one command:

```sh
P='(?<![A-Za-z0-9_.])(?:[a-z0-9]+_)*_?(?:[wr][0-5])(?:_[a-z0-9]+)*(?![A-Za-z0-9_])'
git grep -I -o -P "$P" 830de0623 -- ':!vendor' ':!bench' ':!*.s' ':!*.S' ':!*.svg' | wc -l   # 2245
git grep -I -l -P "$P" 830de0623 -- ':!vendor' ':!bench' ':!*.s' ':!*.S' ':!*.svg' | wc -l   # 257
```

The split comes from a classifier posted on the pull request. It strips Rust comments and strings,
and tells a parameter, a struct field, a binding and a use apart by where the name sits. It is a
heuristic, so each row may be off by a few uses. It also reads the paths the commands exclude and
counts them as foreign, which is why its rows add up to more than 2245.

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

- Code identifiers are 1189 request-side uses (`w0` 540, `w1` 363, `w2` 222, `w4` 18, `w3` 13, `w5`
  11, compounds 22) and 399 result-side uses (`r0` 296, `r1` 67, `r2` 24, compounds and `r3`, `r4`
  12). They sit at 593 declaration sites.
- The public API is 61 names: 55 `pub fn` parameters in 16 files (the decoders of
  `filesystem_contract`, `credential_contract`, `program_table`, `terminal_contract`,
  `system_log_contract`, `graphics_contract`, `jh7110_clock_and_reset_contract`, `disk_tree`,
  `system_log` and the `std` overlay's `rt`), the `pub` fields of `user_mode_runtime`'s `Message`
  and `Request`, and `filesystem_contract`'s `Verb::carries_w1`.
- 25 Cargo packages and areas carry code uses; `components` (460 uses of every kind), `system_tests`
  (216), `filesystem_contract` (167), `user_mode_runtime` (160) and `fixtures` (156) hold most.
- About 15 uses in 3 files reuse the spelling for something else: `capability_plan`'s proof rows, a
  Python helper's work list, a terminal script's test text. They are not message words and no
  option renames them.
- Foreign names are out of scope and stay: AArch64 `w0`..`w30` registers, x86 `r8`..`r15`, RISC-V
  `a0`..`a7`, the SD card's `R1` response (`R1_ERROR_MASK`), and the register accessors `r64`, `w8`.
- Outside the tree, the extracted crate `clock_contract` has 4 prose uses in 1 file;
  `byte_sink_contract` already says `word_zero`; `designware_mobile_storage`'s `r1` is the SD
  response, so foreign.

## The options

What each option changes, counted from the table above.

### (a) Spell the position out: `word_zero`, `result_word_zero` (recommended)

As set out under the recommendation.

- Changes: 1588 code uses in 148 files, 602 prose uses in 131 files, 16 falsification patches
  regenerated, and `first_word`/`second_word` in `domain_name_resolution_contract` (about 30 uses
  in 5 files). About 260 files in all; `clock_contract` in its own repository.
- Keeps: the `user_mode_runtime` and `byte_sink_contract` rulings exactly.
- I weighed a one-based spelling, `request_word_1` and `reply_word_0`, and it loses. A digit inside a
  name is the shape design/naming.md spells out elsewhere, and one-based numbering would turn
  position 0 into `_1` and break the mapping to the wire tables. Both would also reopen two rulings
  made this week.
- A variant, (a2), takes the ordinal `first_word`, `second_word` from
  `domain_name_resolution_contract`. It reads naturally for two words and less well for six
  (`sixth_word` against `word_five`), and it reopens the two rulings that agree instead of the one
  that does not.

### (b) Name every word by its meaning, per verb

`destination_handle(w1: u64)` would become `destination_handle(packed: u64)` or similar, and
`offset` would replace `w1` wherever the code means a `READ`'s second word.

- It fails where the code is generic. `send(slot, w0, w1, w2)` and the trap wrappers in
  `user_mode_runtime` carry words with no meaning of their own, and a word such as `READ`'s first one
  packs three values, so no single meaning can name it.
- It is a judgment at each of 593 declaration sites, and the sender and the receiver of one word
  would name it differently.
- Where the code has decoded a value, (b) is already the rule, and (a) keeps it.

### (c) Keep `w0`..`w5`, `r0`..`r2` and define them in design/naming.md

- Changes: design/naming.md gains a definition and the abbreviations join the stay list. No code
  changes, unless calef reverts the three package rulings, which would touch 38 files for `word_zero` and
  its siblings plus `domain_name_resolution_contract` and `user_mode_runtime`'s `reply`.
- Costs: either the tree keeps two spellings for one concept, or three rulings made in the last
  week are undone. The stay list is for terms whose expansion nobody says (§154 (the acronym test is
  whether the phrase is spoken, applied recursively)); people say "word zero" aloud, so `w0` does not
  pass that test.
- Cheapest, and the only option whose case is effort.

### (d) Hybrid: keep `w0`..`w5` in the register-level modules, spell out elsewhere

The modules where a word is one register: `user_mode_runtime`'s `system_call.rs` and
`inter_process_communication.rs`, and the `std` overlay's `rt.rs`.

- Changes: 1437 code uses in about 145 files, and prose as in (a).
- Keeps 151 code uses in those 3 files. But `user_mode_runtime`'s parameters there are already
  spelled out under the 2026-11-20 ruling, so the module that (d) exempts is the one that has
  already spelled them out. (d) would leave locals and public fields in the old spelling beside
  parameters in the new.
- The register itself already has a name, `x0` or `a0`, and that stays in doc comments as ruled.
  A third spelling for the same register adds a word to remember.

## Prior art

Read for this decision unless marked otherwise.

- RISC-V SBI binary encoding (`riscv-sbi-doc`, `binary-encoding.adoc`): the specification names arguments by
  register, `a0` to `a5`, with the function in `a6` and the extension in `a7`. The two results are a
  struct named by meaning, `struct sbiret { long error; long value; }`, in `a0` and `a1`.
- Linux `syscall(2)` (man7.org): the calling-convention tables name the positions `arg1` to `arg7`,
  one-based, and the results `Ret val`, `Ret val2` and `Error`, then map each to a register per
  architecture (riscv `a0`..`a5`, arm64 `x0`..`x5`).
- seL4 (`libsel4`, `functions.h` and the Arm `syscalls.h`): words are message registers, reached by
  index with `seL4_GetMR(int i)` and `seL4_SetMR(int i, seL4_Word mr)`. The fast-path wrappers
  `seL4_CallWithMRs` and `seL4_SendWithMRs` take `seL4_Word *mr0` to `*mr3`: abbreviated and
  zero-based, the shape nife has now. I recall, and did not read, that the seL4 manual defines `MR0` as a term.
- Fuchsia (`zx_channel_call` reference): a message is bytes and handles, and nothing names a word
  at all. The call's parameters take names by meaning: `handle`, `options`, `deadline`, `args`,
  `actual_bytes`, `actual_handles`.
- Redox (`redox_syscall` 0.9.5, read from the local cargo registry): the raw calls take single
  letters, `syscall4(a, b, c, d, e)`. The scheme protocol's `Sqe` holds `opcode`, `tag`,
  `args: [u64; 6]` and `caller`, and `Cqe` holds a `result`: an indexed array for the words and a
  meaning name for the answer. I recall an older `Packet { a, b, c, d }` and did not read it.

Where nife differs: like seL4, a nife message is positional words whose meaning each contract sets,
so a position needs a name, which Fuchsia's byte messages never do. Unlike seL4 and Redox, nife
spells its names out, so nife spells the position (`word_zero`) where seL4 writes `mr0` and Redox
writes `a`. The result side follows SBI and Linux in naming the answer for what it is, a result.

## A concrete example

`filesystem_contract`'s `MKDIR` today:

```rust
/// ... `w1` is the requested [`super::directory`] mask, or ...
/// Reply `r0` = the new directory handle, or an error.
pub const fn destination_handle(w1: u64) -> u64 { ... }
```

Under (a):

```rust
/// ... `word_one` is the requested [`super::directory`] mask, or ...
/// Reply `result_word_zero` = the new directory handle, or an error.
pub const fn destination_handle(word_one: u64) -> u64 { ... }
```

and at the server, once decoded: `let destination = fs::destination_handle(word_one);`.

## Effort, stated plainly

(a) is a mechanical rename, but it is not a one-line `sed`. About 10 files already hold both
`w1` and `word_one` (a `MessageWords` field beside a local), so each of those needs reading for a
shadowed name. rustfmt rewraps the lines that grow. The 16 falsification patches quote the old
lines, so the lane regenerates and replays each one. The Kani harnesses and mutation baselines name
functions, not these parameters, so they should not move; the rename lane checks that. One lane,
in a few large commits by area, after the in-flight renames of the same files land.

## How reversible it is

A two-way door today. nife has published no crate, a Rust parameter's name is not part of a crate's
compatibility promise, and nothing outside the tree reads these names. The public struct fields
(`Message::w0` and the rest, `Verb::carries_w1`) become one-way at each crate's first release, which
is why the convention should come before the release reviews reach them. The wire format does not
change under any option.

## What waits on this

- The rename lane for (a), or the naming.md bullet for (c).
- The name reviews of every contract that still writes `w0` (`filesystem_contract`,
  `credential_contract`, `terminal_contract`, `program_table`'s spawn protocol and the rest), which
  would otherwise each choose again.
