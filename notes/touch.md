# `touch`: create if absent, and now the mtime half

Milestone 47. The create half built 2026-08-22; the mtime half built 2026-08-24 (DECISIONS §112).
The contract side is `filesystem_contract::fs::CREATE` (milestone 31 (a capability shell: designation is authorization) phase 2), `GETMTIME`, `SETMTIME`
and `SETMTIME_AT`. The mtime verbs come from the mtime lane of milestone 47 (navigation and naming: `cd`, `pwd`, `ls`, `mkdir`, `rm`, paths, and environment), and all four names are provisional except `CREATE`. The
builtin is `Nav::touch` in `components/src/swish/navigation.rs`, parsed by `grant_plan::Command::Touch` into a
`TouchOperands`.

## What this does

`touch <name>` creates an empty file at `<name>` if it is not there (a no-op if it already is,
whatever kind it is), then bumps its modification time to now.

`touch -t <RFC-3339-instant> <name>` does the same create-if-absent step, then sets the
modification time to the instant you assert rather than to now: `touch -t 2030-01-01T00:00:00Z
report.txt` backdates or postdates the name, deliberately, which is the ability DECISIONS §112
calls "the ability to lie about history."

A builtin, in `mkdir`'s category rather than `rm`'s, for both halves. Even `-t` takes no more than a
directory capability (`dir::WRITE` for the bare form, `dir::WRITE | dir::SETTIME` for `-t`). So
there is nothing to attenuate and nothing gained by confining it to a program with its own grant.
`mkdir` already established the shape (`fs::MKDIR` mints a name and hands the capability
straight back); the create half is the same call with the handle closed instead of kept. There is
nothing useful to do with a handle to an empty file the moment after making it. Setting the time is
a second, separate wire request. It is resolved directly under the directory handle exactly as
`UNLINK`'s name is: neither mtime verb opens what it acts on.

## Rights: DECISIONS §112, and why it needed a decision at all

`touch` does two different things to a timestamp, and the question milestone 47 left open for two
days was whether they are the same authority. They are not. Setting a name's mtime to *now* is
bounded by what the server itself observed: the value recorded is one nothing the caller supplied,
the same way a `WRITE`'s byte count is a fact rather than an assertion. Setting it to an *arbitrary*
instant is unbounded: the caller can claim the file is older or newer than it has any reason to be,
which matters for anything reasoning from mtime, backups included.

So:

- **Bare `touch`** needs `dir::WRITE`, the same right `mkdir`'s create half already uses. No new
  grant is needed for a shell that could already write into the directory.
- **`touch -t`** needs `dir::WRITE` and `dir::SETTIME`, a seventh rung added to the directory
  rights ladder (DECISIONS §47 extended by §112) alongside `ENUMERATE`/`READ`/`WRITE`/`CREATE`/
  `REMOVE`/`DESCEND`. A shell holding `WRITE` but not `SETTIME` can `touch` a name to now and cannot
  `touch -t` it to anything else; see `kernel::user::shell_navigation_tests`'s
  `TOUCH_NOW_NEEDS_ONLY_WRITE` / `TOUCH_AT_REFUSED_WITHOUT_SETTIME` pair for the proof against a
  real, narrower-than-`dir::ALL` grant over the real wire.

Two independent precedents converge on this split rather than inventing it. POSIX's own `utime()`
needs only write permission to set the current time and ownership to set an arbitrary one. This
tree's own DECISIONS §43 already treats reading the wall clock as broadly grantable and setting it
as a separate, more tightly held authority. `touch`'s two behaviors are the same shape one level
down, applied to a file's timestamp instead of the machine's clock. See
`design/decisions/0112-touch-mtime-authority.md` for the full argument.

## What "now" is, honestly

The FS server has no RTC wired to it. "Now" for `SETMTIME` is the server's own advancing logical
clock. It is the same mechanism that has stamped every `WRITE`, `CREATE`, `TRUNCATE` and the
extended attribute verbs since before this lane. A `touch` on two names in sequence is guaranteed to
observe the second mtime strictly greater than the first (what a make-style staleness check depends
on). It is not guaranteed to agree with a wall-clock reading (`date`, DECISIONS §43 (reading the clock is a page)) taken at the same
instant. Wiring the FS server to the real wall clock milestone 51 landed is a follow-up (see `BUGS`
below). It is not a difference in what `SETMTIME` promises today.

## The wire: three verbs, and why not one

`filesystem_contract::fs`:

- `GETMTIME` (name-taking, needs `dir::READ`): reads a name's mtime in Unix seconds. Resolved
  directly under a directory handle, like `UNLINK`, because `touch` never opens what it acts on and
  a getter that needed a handle when the setters do not would be an asymmetry nothing forces.
- `SETMTIME` (name-taking, needs `dir::WRITE`): sets it to now. No payload beyond the name; there is
  nothing for the caller to supply, because "now" is not the caller's to assert.
- `SETMTIME_AT` (name-taking, needs `dir::WRITE | dir::SETTIME`): sets it to a caller-supplied
  Unix-seconds value carried in the request's second word, `TRUNCATE`'s reason for using `w1` rather
  than the length field for an offset-shaped quantity.

Three verbs rather than one with a "now or arbitrary" flag, because the rights differ per verb and
`filesystem_contract::verb::TABLE` (the table every caretaker's dispatch is built from) encodes one
fixed `needs_all` per opcode. A single opcode would need dynamic, per-call rights outside that
table's model. This is the same reasoning that gave `rm` (`UNLINK`) and the structural bound
(`RMDIR`) two verbs instead of one that tries to do both.

**`SETMTIME_AT` takes no range check on the seconds value.** Any `u64` is accepted, including one
that predates this image or postdates the machine's own clock: asserting an impossible history is
the same authority as asserting a merely surprising one, and this contract does not referee which
lies are plausible.

## `-t`'s syntax: RFC 3339, not Unix's `[[CC]YY]MMDDhhmm[.ss]]`

`grant_plan`'s parser does not interpret the `-t` operand's text at all; it only recognizes the `-t`
flag and hands the following token, unparsed, to `TouchOperands::at`. Converting it to Unix seconds
happens where the grant is made (`Nav::touch` in `components/src/swish/navigation.rs`), using
`rfc3339_unix_seconds` there, which checks RFC 3339's grammar and hands the date and time of day to
`jiff`, the crate `date` itself uses for the read side. `date`'s own `FMT_RFC3339` output is therefore a valid `-t` operand:
`touch -t "$(date ...)" name` round-trips through this shell without either side inventing a
format.

This is an earned divergence from Unix's compact `-t [[CC]YY]MMDDhhmm[.ss]` format, not an
oversight. There is no existing parser for that format in this tree, and RFC 3339 is what `date`
already emits. Building a second, redundant date grammar for one flag was not worth it. See `BUGS`.

## EXAMPLES

```
$ touch report.txt
$ ls
  report.txt
$ touch report.txt
$ ls
  report.txt
```

The second `touch` is silent and changes nothing to the file's bytes. Its modification time still
moves, which is the mtime half this section is about.

```
$ mkdir logs
$ touch logs
$ ls
  logs/
```

`touch` on a name that already exists as a directory is also a no-op for the create half (`ls` still
just shows `logs/`). Its mtime still bumps, because mtime is part of what a node *is*,
not a property files alone carry.

```
$ touch -t 2030-01-01T00:00:00Z report.txt
$ touch -t not-a-date report.txt
  -t needs an RFC 3339 instant, e.g. 2030-01-01T00:00:00Z
```

A malformed `-t` operand is refused before the create half runs, so a caller does not see a file
appear and then learn its `-t` text was unusable.

## Tests

`crates/grant_plan` and `crates/swish` host suites: `touch` and `touch -t <instant> <name>` both
parse correctly (including the `-template` guard: a name that merely starts with `-t` is not the
flag). `touch` is reserved from the program namespace like every other builtin, and both forms
appear in `help`.

`redoxfs_server`'s host suite (`cargo test --manifest-path redoxfs_server/Cargo.toml --features hosttest`)
covers the rights logic directly against the in-process `Server`, no emulator. `each_rung_of_the_
ladder_gates_exactly_its_own_verb` isolates `dir::READ` (`GETMTIME`), `dir::WRITE` (`SETMTIME`) and
`dir::SETTIME` (`SETMTIME_AT`, alongside `WRITE`) each against a handle that lacks only that one
bit. `setting_mtime_to_now_twice_observes_two_different_times_and_neither_is_the_callers_choice`
proves two bare touches in sequence never read back equal. `setting_an_arbitrary_mtime_round_
trips_exactly_and_is_not_now` proves the caller's value comes back byte-for-byte and that a
following bare touch does not preserve it.

`kernel::user::shell_navigation_tests`, both ISAs, over the real wire (a real `swish` binary behind
a real `fs_subtree_caretaker` in front of a real `redoxfs_server`):

- `TOUCH_CREATED` / `TOUCH_PRESERVED` (built 2026-08-22): the create half's own pair, unchanged by
  this lane.
- `TOUCH_MTIME_ADVANCED`: a bare `touch`, typed as a real command line, observes a strictly later
  mtime through `GETMTIME` than the file had before.
- `TOUCH_AT_ROUND_TRIPPED`: `touch -t 2030-01-01T00:00:00Z`, typed as a real command line the
  parser has to split into four tokens and `jiff` has to decode, lands on exactly
  `NAV_TOUCH_AT_UNIX` and not the value `TOUCH_MTIME_ADVANCED` just observed. The exact-equality
  check, not merely "later", is what makes this bit mean the caller's assertion moved rather than
  the server's own clock falling back to "now".
- `TOUCH_NOW_NEEDS_ONLY_WRITE` / `TOUCH_AT_REFUSED_WITHOUT_SETTIME`: a fresh directory this shell
  mints with a raw `MKDIR` carrying `dir::ALL & !dir::SETTIME` (deliberately narrower than every
  other grant this script runs under), against which a bare `SETMTIME` succeeds and `SETMTIME_AT`
  is refused. This is what makes the split in DECISIONS §112 (`touch`'s two behaviors need two rights) provable over the real wire rather than only
  in `redoxfs_server`'s in-process host tests. A wrong row in `filesystem_contract::verb::TABLE`, or a
  caretaker that forwarded rights incorrectly, would not be caught by a test that calls
  `Server::set_mtime_at` directly.

Witnessed from the host by `xtask`'s `shell_navigation_landed` for the create half, as before; the
mtime probes are not (yet) independently witnessed from the host. See `BUGS`.

## BUGS

- "Now" is this server's own advancing logical clock, not a reading of the real wall clock
  milestone 51 (wall-clock time) landed (`clock_contract`, DECISIONS §43). Two bare touches in sequence are guaranteed
  to observe strictly increasing mtimes. A bare touch is not guaranteed to observe a mtime close to
  what `date` reports at the same instant. The FS server could get a read-only mapping of the clock
  page (the same authority `date` itself holds), so `SETMTIME` records a real wall-clock second. That
  is a follow-up, not started here. It is a new capability grant to a process that currently holds
  none beyond the block-IPC endpoint. DECISIONS §43's own read/set split argues for exactly the
  narrow, read-only half. **Since 2026-09-19 it reaches `std` too**: milestone 64 bound
  `Metadata::modified` to `GETMTIME`, so every std program reading a file this system wrote gets the
  counter as a `SystemTime` in early 1970 (notes/std/file-times.md). Proposed as
  design/roadmap/0497-a-filesystem-server-that-knows-the-time.md.
- **`-t` accepts RFC 3339, not Unix's compact `[[CC]YY]MMDDhhmm[.ss]]`.** See "`-t`'s syntax" above
  for why. A script written against Unix's `touch -t` syntax will not work unmodified here.
- **No `-c` (don't create)**, because Unix's `-c` exists to suppress the create half, and there is
  nothing else here to fall back to.
- **The mtime probes' platter effect is not independently witnessed from the host** the way the
  create half's is (`xtask::shell_navigation_landed`). The kernel-boot assertions are a real proof
  (real wire, real caretaker, real `redoxfs_server`, real RedoxFS transaction), but nothing outside the
  guest currently re-reads the image and checks a node's stored `mtime` field directly. A follow-up
  host check, reading the `NAV_TOUCH_NO_SETTIME`-prefixed directory `tools/redoxfs_host` left
  behind, would close this the same way the create half's host check already does.
