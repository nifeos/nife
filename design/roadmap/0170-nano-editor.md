---
status: NOT-STARTED
raised: 2026-08-25
milestone_dependencies: 809, 801
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 170. `nano`: a real, full-featured screen editor on the primitive milestone 169 builds, compiled unmodified against the C library

GNU nano is the editor a newcomer expects to find. This milestone brings it to nife the way SQLite's
`speedtest1`, ioping and hackbench arrived: compiled unmodified against `c_library`. That is the C
library §265 (a C library started from relibc) started, and milestones 835 (a C library, stage 1:
files, clock and memory) and 836 (a C library, stage 2: threads) built. It does not go through §31
(the foreign-language seam) or milestone 181 (the foreign-language seam, extended to a process the
shell calls more than once).

calef, 2026-10-18 (UTC): *"Yes: rewrite milestone 170 for the C-library route, with a kilo.c port
as the first step that proves termios; the port itself still waits on 801 and 809."*

Reuse: nano and ncurses, both taken whole as upstream source and built by basalt. Nothing here is
written in place of an existing program. §135 (running GPL software is aggregation) covers nano's
GPL: it arrives as a package.

## Stage 1: antirez's `kilo.c`, unmodified, to prove the terminal layer

[`kilo`](https://github.com/antirez/kilo) is one file of about 1,000 lines. Read 2026-10-18 (UTC),
it includes `<termios.h>` and `<sys/ioctl.h>` and needs nothing nano would not also need. No
ncurses, no regex, no `fork` or `exec`. It enters raw mode with `tcgetattr` and `tcsetattr`, draws
with hand-written ANSI sequences, and installs a `SIGWINCH` handler.

Its job is to prove the terminal layer below before nano's much larger surface sits on it. A
failure in a 1,000-line program points at the C library. The same failure under nano and ncurses
could be in any of three code bases.

The stage is done when `jig install kilo` from basalt's index puts `kilo.c` on a nife system, and it
edits and saves a file on all three architectures. It rides the same package path as nano, so it
waits on the same milestones. It is a stage and not a milestone of its own because nothing outside
this block depends on it, and as a milestone its dependency fields would match this one's exactly.
If another program comes to need the terminal layer before nano does, split it then
(`notes/roadmap.md`: a dependency on part of a milestone means split it).

The raw-keystroke primitive from milestone 169 (`kilo`: the smallest real text editor),
`OPERATION_RAWMODE` and `OPERATION_READRAW` in `crates/line_editor`, is what termios sits on.

## The architect's rulings

calef ruled the block's three forks on 2026-10-18 (UTC), on #2328:

- Terminal grant: *"B: a terminal grant only for programs whose manifest declares one; the slot
  number is reserved in std's convention and filled only on request."*
- Terminal size: *"A: a new line_editor operation answering rows and columns; c_library maps
  TIOCGWINSZ onto it, and the server learns the size from its medium (direct on screen, ESC [6n
  probe on serial)."*
- Regex: *"musl in c_library."* Then, on learning nano is GPL-3.0-or-later: *"Yes: musl in
  c_library for every C program, and nano also builds against its own gnulib regex so GNU search
  syntax works."*

The terminal grant and the size operation are formats two programs share, and the regex is a
dependency, so each is owed a `design/decisions/` section. The maintainer mints it; until then this
list is where the rulings are written.

## Stage 1 work: the terminal layer, checked against the tree 2026-10-18 (UTC)

### A terminal grant, declared in the manifest

A C program has no terminal today. `c_library` reads standard input through `std::io::stdin()`
(`c_library/src/platform/nife.rs`), and nife's `std` answers every read of it with end of file
(`patches/std-nife/overlay/std/src/sys/stdio/nife.rs`). Standard output goes over the sink
contract, not the terminal contract. `isatty` is always false (`c_library/README.md`, BUGS), and
`kilo` refuses to start when `isatty(STDIN_FILENO)` is false.

The ruling makes the terminal a declaration. A program's manifest (`grant_plan::Manifest`,
`notes/program-manifest.md`) gains a `terminal` declaration beside `clock`, `config` and `entropy`.
The manifest note's encoding (`crates/capability_manifest_note`) carries it in a new version word,
since that crate takes no tolerated extensions. The naming precedent is
`design/naming/precedents-set-by-example.md`: a manifest says Declaration for what a program asks
and Grant for what it gets.

`crates/std_runtime_contract` reserves a slot number, `TERMINAL_SLOT`, for the terminal endpoint.
The progenitor fills it only for a program that declares a terminal; for every other program it
stays empty, and `std` treats an empty slot as no terminal, the way it treats every other empty
slot.

With the slot filled, `c_library` gains a terminal description: `read` and `write` on standard input
and output speak the `line_editor` contract, and `isatty` answers true.

*Name: provisional.* The `terminal` declaration and `TERMINAL_SLOT` are this block's proposals, and
the slot's number is assigned when it is built. The alternatives considered were `console` and
`tty`. `terminal` won because it is the word §21 (the terminal is a userspace component) and
`crates/line_editor` already use for the same endpoint.

### `termios.h`

No `termios` module in `vendor/relibc/header`. The work is a header and its functions in
`c_library`: `tcgetattr`, `tcsetattr`, `cfmakeraw` and the flag constants. Clearing `ICANON` and
`ECHO` maps onto `OPERATION_RAWMODE`, and a read in raw mode onto `OPERATION_READRAW`. Flags the
terminal cannot honor are accepted and listed in BUGS rather than faked.

### The terminal's size, a new `line_editor` operation

relibc's `ioctl` (`vendor/relibc/header/sys_ioctl/mod.rs`) answers every request with `ENOTTY`.
The terminal contract carries no size (`notes/terminal-contract.md`), which is why a program
cannot learn the window size.

Per the ruling, `crates/line_editor` gains an operation, `OPERATION_WINDOWSIZE`, that answers rows
and columns. `c_library`'s `ioctl` maps `TIOCGWINSZ` onto it and fills a `struct winsize`. The
terminal server learns the size from its medium. On a screen it is direct, from the surface it
draws. On serial it probes with `ESC [6n` after moving the cursor to the far corner, which a
terminal emulator answers with its position. A probe nobody answers falls back to 80x24 rather
than blocking.

That keeps `kilo`'s own `ESC [6n` fallback in `getWindowSize` from ever running: it is taken only
when `TIOCGWINSZ` fails. ncurses reads the same `ioctl` before `LINES` and `COLUMNS`.

*Name: provisional.* `OPERATION_WINDOWSIZE` follows POSIX's `winsize` and `TIOCGWINSZ`; its opcode
is the next free one in `crates/line_editor/src/proto.rs` when built. The alternative considered,
`OPERATION_SIZE`, does not say what is measured.

### Signals

`c_library` records a handler and delivers only a signal the program raises itself (BUGS). So
`SIGWINCH` never arrives: the size is right at start and wrong after a resize, until the program
asks again. `kilo` and nano are both built without suspend (`^Z`), since there is no job control
to suspend into. Both are BUGS entries in the package, not workarounds.

## Stage 2 work: ncurses, regex and the rest of nano

### ncurses, wide-character build

nano will not build without it, and the tree has none. basalt cross-builds `ncursesw` against
`c_library` with its terminal descriptions compiled in (`--with-fallbacks`, `--disable-database`),
so no terminfo database has to exist on nife. The first set: `vt100`, `xterm-256color` and `linux`.

### `regex.h`, from musl

Absent from `vendor/relibc/header`. Per the ruling, `c_library` takes musl's regex for every C
program, and nano also builds against its own gnulib `regex` module so GNU search syntax works.

musl's regex derives from Ville Laurikari's `libtre`. musl's `COPYRIGHT`
([source](https://git.musl-libc.org/cgit/musl/plain/COPYRIGHT)) says that code, in
`src/regex/reg*` and `src/regex/tre*`, is "licensed under a 2-clause BSD license". `regcomp.c`
carries that license's text. The rest of musl, including `src/regex/fnmatch.c` and `glob.c`, is
MIT. Those files go under `vendor/` with their notices, recorded in `vendor/README.md`.

nano's tarball carries gnulib's `regex` (read from nano's `autogen.sh` 2026-10-18 UTC). basalt's
recipe forces gnulib's copy into nano, so nano searches with GNU syntax while every other C program
uses musl's POSIX regex.

### Wide characters and locale

Present in `c_library`: `setlocale`, `nl_langinfo`, `mbrtowc`, `wcrtomb`, `mbstowcs`, `mbtowc`,
`btowc`, `mbsinit`, `wcwidth`, `wcswidth`, `iswprint`, `iswalnum`, `towlower` and `wctype`.
`nl_langinfo(CODESET)` answers `UTF-8`, which is what nano checks before it handles UTF-8.

Two limits. `wcwidth` is 1 for every printable character, wrong for East Asian wide characters and
combining marks (`c_library/README.md`, BUGS). nano's tarball also carries gnulib's `wcwidth`,
which may cover nano's own column math; ncurses still calls the C library's. And `setlocale` with
any name other than `C`, `POSIX` or the empty string opens `/usr/share/i18n/locales/<name>`, which
nife does not have, so it fails. nano calls `setlocale(LC_ALL, "")`, which succeeds.

### The speller and `^T` (execute a command)

Both start another program. §264 (`fork` is declined for good) rules out `fork`, and
`posix_spawn` is milestone 838 (a C library: `posix_spawn`, and no fork), not started. nano is
configured with the speller off and with execute unreachable, and the package's BUGS section says
so. They can return once milestone 838 is built.

### `pwd.h`, and `~/.nanorc`

No `pwd` module in `vendor/relibc/header`, and nife attaches no user to a process. nano is built
without reading an rc file or keeping histories, so it never looks up a home directory.

### nano itself

With the terminal layer proven by `kilo`, basalt builds `ncursesw` and then nano against
`c_library`. Configuration is a recipe in basalt, not a patch to nano. nano's file work (swap files,
backups, lock files) is ordinary file I/O against the directory it is granted.

## Done when

calef, 2026-10-12 (UTC), under §297 (software reaches a nife system as a package `jig` installs
from basalt's index, the kernel included): "They should all be installed via jig through basalt."

`kilo` and then `nano` run on nife installed by `jig install` from basalt's package index, on all
three architectures. basalt builds each package against the nife commit it pins. The install waits
on milestone 809 (the package client becomes a program, `jig`, with the verbs an index needs). It
also waits on milestone 801 (packages over the internet: a host name, HTTPS and the distribution's
index). No work toward that starts until those milestones are built (calef, 2026-10-12 UTC: "For
our other programs let's block start on packaging.").

## Shipping nano's source

nano is GPL-3.0-or-later. Every source file's header says so; `src/nano.c` reads "either version 3
of the License, or (at your option) any later version"
([source](https://cgit.git.savannah.gnu.org/cgit/nano.git/plain/src/nano.c)). Its `COPYING` is the
GNU General Public License, version 3.

So basalt keeps the corresponding source for every nano package it publishes available for as long
as it offers that package. That is nano's tarball by digest, basalt's recipe and configure flags,
and gnulib's modules as carried in the tarball. The `ncursesw` and musl code linked into nano are
permissively licensed and travel with their notices. §135 (running GPL software is aggregation)
covers serving GPL packages from basalt.

## Index row

nano, compiled unmodified against the C library (§265, milestones 835 and 836) and installed from
basalt with `jig`. Stage 1 ports antirez's `kilo.c` to prove a terminal layer the C library lacks:
a declared terminal grant, `termios.h` over `line_editor`'s raw mode, and a window-size operation.
Stage 2 adds `ncursesw`, musl's regex and nano, with the speller and `^T` off until `posix_spawn`
exists.
