# The shell edits its own line

Milestone 47 (navigation and naming) built this on 2026-09-26 (UTC), under calef's ruling on
DECISIONS §227 (how Tab reaches the shell): option D. The shell turns the terminal's raw mode on
and runs the same `line_editor::LineDisc` engine in its own process, the way bash runs readline.
So a Tab reaches the process that holds the authority completion needs, and no new message
crosses the terminal wire. The note's name is provisional.

## What happens to a keystroke

At the prompt, `components/src/swish/prompt.rs` (`edit_line`) holds the terminal in raw mode
(`OPERATION_RAWMODE`). Each `OPERATION_READRAW` reply carries up to eight bytes. The shell feeds them to its
engine and sends the echo back with `OPERATION_WRITE`. Enter, `^D` and `^C` end the read with the same
answers `OPERATION_READLINE` gave: a line, end of input, or interrupted. Bytes that arrive after the one
that ended a line are kept for the next one, so a pasted pair of lines runs as two.

Tab is `line_editor::Event::Tab`. The shell reads the word left of the cursor
(`swish::complete::completing`) and finishes it:

- In command position (the first word, or after `|`, `;`, `&&`, `||`, `caps`, `time` or `xargs`):
  a builtin (`grant_plan::BUILTINS`) or a program the image carries (`grant_plan::Program::ALL`).
- Anywhere else, or for a word with a `/`: an entry of the directory the word leads into, read
  the way `ls` reads it. That needs `ENUMERATE`, the right `echo *` needs, so completion cannot
  offer a name the shell could not already list.

One match is inserted whole, ending in `/` for a directory or a space otherwise. Several matches
insert what they all share. When nothing more is shared, they are listed and the line is painted
again. No match rings the bell.

## `^C`, in both places it can be pressed

§24 (interrupting the foreground process) watches a supervised job by polling the terminal's
`^C` count (`OPERATION_INTRCOUNT`). The terminal counts only in its line discipline, and raw mode
bypasses that. So the shell leaves raw mode just before it spawns a supervised job, and takes its
watermark after the switch. It turns raw mode on again at the next prompt.

- At the prompt, `^C` is a byte. The engine prints `^C`, discards the line, and the prompt comes
  back. The terminal's count does not move, so a `^C` at the prompt can never be charged to the
  next job.
- While a supervised job runs, `^C` is counted by the terminal exactly as before, and the
  two-tier escalation is unchanged.
- While a plain command runs, raw mode stays on. A `^C` then waits in the raw queue and discards
  whatever was typed ahead of it at the next prompt. Before this change it was counted, and
  nothing reset the watermark, so it could reach the next supervised job's watch as a spurious
  interrupt. That was found by reading the code and not reproduced; the watermark is now taken
  when raw mode is left, which closes it either way.

## Measured

| | Before | After | Change |
|---|---|---|---|
| `swish` text, aarch64 debug | 299,831 B | 331,379 B | +31,548 B (10.5%) |
| `swish` text, aarch64 release | 147,901 B | 167,549 B | +19,648 B (13.3%) |
| `swish` bss | 7,800 B | 11,136 B | +3,336 B, the engine held in a `static` |

The image bound is 896 KiB (milestone 206 (a program image has under 896 KiB)), so the shell stays
well inside it. Measured with `llvm-size` on the ELF, on 2026-09-26, from `cargo build -p components
--bin swish`.

The per-keystroke cost is two more IPC round trips for the shell: an `OPERATION_READRAW` reply and an
`OPERATION_WRITE` of the echo. The terminal already paid the console flush before, and still does.
`kernel::user::raw_mode_tests::a_keystroke_edited_by_the_client_costs_two_more_round_trips` times
both paths against the real `line_editor` process and prints a `measure:` line. Its reading is below, under
[What was measured under QEMU](#what-was-measured-under-qemu). On the EL0 figure
[benchmarks.md](benchmarks.md) records, 350 ns a round trip, that is under a microsecond a
keystroke, against a person typing ten a second.

### What was measured under QEMU

CI run 36262762671 (2026-09-26, TCG on the GitHub runner), four legs of the kernel suite:

| | Per keystroke |
|---|---|
| Cooked, the terminal echoes | 400 to 483 µs |
| Raw, the client reads and echoes | 692 to 807 µs |

So the client-side editor costs about 1.7 times the cooked path per keystroke under emulation,
roughly 0.3 ms more. Emulated IPC is what dominates both, so the absolute numbers say little about
hardware. On the 350 ns EL0 round trip [benchmarks.md](benchmarks.md) records, the two extra round
trips are under a microsecond, far below a person's ten keys a second.

## EXAMPLES

```text
$ printe<Tab>          ->  $ printenv
$ caps wc doc/kernel/ipc-nam<Tab>
                       ->  $ caps wc doc/kernel/ipc-naming.md
$ c<Tab>               ->  lists the names starting with c, and paints "$ c" again
$ echo half^C          ->  nothing runs; a fresh prompt
```

## BUGS

- Typed-ahead text is lost at a supervised job's edges. `OPERATION_RAWMODE` discards the queue it
  leaves, so keys typed at the prompt just before a supervised job starts, or typed during one,
  are dropped. A plain command keeps them. Fixing it needs the terminal to count `^C` in raw mode
  for a client that asks, which is a change to `OPERATION_RAWMODE`'s meaning, so it is recorded rather
  than made.
- The engine assumes one terminal row (`line_editor`'s own BUGS), and a completion listing wraps at
  78 columns for the same reason.
- `swish::complete` carries its own BUGS. It does not complete inside a quote, and it does not
  quote a completed name with a space in it. Arguments are not completed from the program's
  manifest. An installed program is not offered by bare name until §229 (how a bare name reaches an installed program) is ruled.
- A terminal that refuses raw mode gets the old `OPERATION_READLINE` path and no Tab. Every terminal in
  the tree serves raw mode, so this is a fallback, not a configuration.
