# Prior art

An appendix to [the milestone](../1098-line-editor-outside-review.md).

This table sets the engine against what a reader already knows. Recent downloads are the last 90 days
at the time of the review, from the crates.io API.

| what | recent / all-time downloads | shape | what a user coming from it will miss here |
|---|---|---|---|
| `rustyline` 19 | 14.8M / 59.9M | `std`, allocating, per-call `readline()`, Emacs and vi modes, completion and hint traits, file history | multi-line, UTF-8, vi keys, a completion trait, history files, a configurable history size |
| `reedline` 0.53 | 1.17M / 4.16M | `std`, allocating, per-call, menus, validators | the same, plus menus and syntax highlighting |
| `noline` 0.5 | 2.3k / 37.4k | `no_std`, sans-IO core, async and blocking fronts, UTF-8, history as a feature | UTF-8 input, a terminal-size probe (`ESC [ 6 n`), a configurable history; its initialization waits for a cursor-position report (recorded in `notes/line-discipline.md`; not re-verified here) |
| `embedded-cli` 0.2 | 17.4k / 64.8k | `no_std`, a command framework with its own editor and autocompletion | command parsing, which is the shell's altitude here |
| `menu` 0.6 | 61k / 155k | `no_std` menu-driven CLI | the same |
| linenoise (C, antirez) | n/a | a single file, blocking `read()`, 100-line history, optional UTF-8 module, multi-line mode | multi-line editing, a history file, UTF-8 |
| Linux `n_tty`, BSD `tty` | n/a | kernel line discipline: canonical mode, echo, `^C`/`^Z` as signals, `^W` and `^U` kills, no history, no cursor movement | job-control signals (here an `Event` the caller routes), flow control, canonical-mode flags |

Where this crate differs: it is **always on**. It edits and echoes bytes that arrive before any read. A later `start_line` paints the prompt and the type-ahead, which no per-call readline offers. It allocates nothing and has a `const` constructor, so it sits in a `static` on a one-page stack. It holds one row (256 bytes), eight lines of history and a kill buffer. It serializes its carried state as a versioned blob, so a replacement process takes over mid-line. It knows nothing about IO, which lets `crates/video_terminal` run the real editor against the real VT parser on the host. What it costs: ASCII only (it drops a byte at or above 0x80) and one row (a line that wraps redraws wrongly). The eight history lines live only as long as the process or its handoff. It has no Meta or Alt keys and no completion of its own. The concrete example: linenoise's `linenoiseEditFeed` is the same byte-at-a-time shape this crate's `feed` has. Linenoise ships it with a 100-entry history and an optional UTF-8 encoder. A user arriving from it will name those two gaps first.
