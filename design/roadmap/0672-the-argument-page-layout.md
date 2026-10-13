---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: the-argument-page-layout
milestone_dependencies: none
decision_dependencies: 170
machine_requirements: none
specific_machine: none
needs_person: no
---
# 672. The argument page's layout

*Title and slug are drafts.*
Raised by milestone 205 (how a foreign program is told what to do), which built the layout below provisionally so nothing
waits on this. §170 (how a foreign program is told what to do) left the layout open because two
programs agree on it: the shell writes the page and `std` reads it. The slug, the crate name
`argument_protocol` and every constant here are a lane's coinage.

## What is being decided

Six things, all in `crates/argument_protocol` and `crates/std_runtime_protocol`:

| | built provisionally | the alternative |
|---|---|---|
| where the page sits | its own slot, 8, and a page at `0x1400_0000`, one above the configuration page | std's `_start` entry registers, which it ignores today |
| the header | an 8-byte magic `nifeargv`, a `u32` count, a `u32` total of record bytes | no magic, as a raw argv block |
| each argument | a `u32` length, then its bytes | NUL-terminated, as POSIX has it |
| `argv[0]` | present: the name as typed | absent |
| the encoding | bytes; `OsStr` on nife is bytes | UTF-8 only |
| the ceiling | one page: 4,080 bytes of records, four of them per word | several pages |

The environment does not ride on the page. §170 is silent on it, so it stays on §111 (inert
configuration is a validated page)'s page, and this one carries argv alone.

## Why each, answering the seven questions

What the tree already does. Slot 7 and the configuration page are the exact precedent: a fixed
slot, a read-only page at a fixed address, a magic checked first, and a zeroed frame reading as
nothing (`environment_protocol`). Every choice above copies it except the validation, which §170
ruled out: a regex is arbitrary bytes.

Prior art, read. The measurement in
[notes/foreign-program-arguments.md](../../notes/foreign-program-arguments.md) read four systems
from source. Fuchsia and seL4 use NUL-terminated strings; Xous, the nearest neighbor (a Rust
microkernel whose `std` backend is in-tree), passes a tagged parameter block by pointer. None of the
four puts authority in the bytes, which is what §170 ruled.

Length-prefixed rather than NUL-terminated, because a NUL is a legal byte in an `OsStr` here and
the round-trip test puts `0x00` and `0xff` inside one argument. A C program ported later needs a
NUL-terminated `argv`, and its runtime builds one from this page in a few lines; the reverse
conversion would lose arguments with a NUL in them.

A slot rather than the entry registers. The registers are cheaper (no slot, no page table
entry) and are the one alternative with a real claim. They lose on two counts. The probe that tells
`std` "no page" is the same `NO_SUCH_METHOD` probe slot 7 uses, where a register would need a
sentinel value. And `caps` can print a slot as a row, which is how a reader learns the child holds
the page at all; a register is invisible there.

`argv[0]` present, because `ripgrep` skips the first element and `clap` treats it as the
binary's name. Without it the pattern is lost. This is measured, not a preference.

**One page.** 4,080 bytes is sixteen times what this prompt's line editor holds
(`line_editor::LINE_MAX`, 256). A line that does not fit is refused, never truncated
(`Refusal::ArgumentsTooLong`). Several pages would cost a length word and a loop and buy nothing a
person can type; a script runner (§219 (how the shell names an installed program to the spawner)) is the first caller that could want more.

**Cost, measured.** The codec crate is 351 lines, most of them documentation and tests; the PAL
reader is 129, most of it std's iterator boilerplate copied from `sys/args/zkvm.rs`. At run time
the shell spends one page of its budget per spawn, returned when the child answers, and the
progenitor one copy and one slot.

**Reversibility.** Nothing outside this tree has read the page. Changing the layout is a change to
one crate, generated into the PAL, and a rebuild; the shell, the progenitor and `std` move together.
The irreversible moment is the first program built against a released `std`, which has not
happened.

Would we still choose this at equal cost? Yes. The registers were the cheaper option and lost on
visibility, not on effort.

## What is blocked on the answer

Nothing is blocked: the layout is built and every consumer uses it. What the answer changes is
whether the names and numbers above stop being provisional.

## The mark's spelling is a separate question

§170's clause 4 needs a mark on a word ("this word is writable", "this word may be created"). That
is a naming decision with its own proposal,
[`0675-the-mark-on-a-foreign-programs-word.md`](0675-the-mark-on-a-foreign-programs-word.md).

## Index row

The layout of the page the shell writes and `std` reads for a foreign program's arguments was built provisionally by milestone 205 (how a foreign program is told what to do). Proposed: ratify it or choose the alternatives, since two programs agree on it.
