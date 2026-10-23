# The `swish` crate: what the shell decides with no capability in hand

An appendix to [a shell at EL0](../shell.md), whose section "The program and the crate (milestone
70)" draws the line between `components/src/swish/` and `crates/swish`. This page holds the design
the crate's doc comments link to. The stem `the-swish-crate` is a provisional name.

## What a bare word runs

`swish::bare` answers DECISIONS §229 (how a bare name at the prompt reaches an installed program),
option B2. A word with a `/` in it is a path and always means that file; that is decided before
`bare` is asked. A word without one is a builtin, a program the image carries, or the live
installed-programs table's default for that name.

The default is found with `installed_programs::lookup`. It reads the generation's default pointer
and then the row it names, so it never answers with an owner's vouch. Under milestone 614 (two
installed versions of one program, each runnable, and a caller granted the one it needs), ruling 3,
that pointer is what "the newest install" means. Every install of a program moves it, so the bare
word means what it always meant while the other versions stay live beside it.

There is no search order. A name that is both the image's and an installed package's is refused,
naming both, because either choice would be a guess. Install refuses an image program's name (§229,
calef's ruling of 2026-09-27), so the pair arises only when a later base adds a name a package
already holds.

Two words select a version (milestone 614, ruling 4):

- `program@version` is an explicit ask: the table's row for that program at that version, or
  nothing. `@` is reserved for it. The image's claim on the bare name does not reach it, because
  the word is not the contested name.
- A version set selects among the live versions for the bare word. It is the nearest
  `.jig-versions` file at or above the working directory, asdf-style, read by the caller and handed
  in. A set can only select among installed versions. When it names a version that is not live,
  the default runs and the result carries a `Notice`, which the spawn line prints:
  `uptime 0.2.0 (.jig-versions asks for 0.1.0)`.

The shell reads the live table on each line that needs it, rather than caching it. It is two small
file reads, against a spawn that builds a whole process, and a table the shell never caches cannot
be stale.

What the module's BUGS mean in full:

- A bare installed name in a pipeline is refused as a program this image does not carry, as only a
  plain line runs an installed program by its path.
- A table that cannot be read resolves nothing. An installed program's bare name is then refused as
  unknown, and an image program runs without the collision check. The progenitor's rule is the
  same: a table it cannot read vouches for nothing. A version set that cannot be read selects
  nothing, which is harmless for the same reason in reverse.
- `script/swish-check` cannot install a package named after an image program (§229, 2026-09-27),
  and no gate boots a second base that adds a name. So the both-names refusal first reaches a
  machine when a base update under §235 (the OS is built and updated from packages) adds a program
  some installed package already provides. A gate that boots such a base is what would cover it.

## The version set

`swish::jig_versions` reads a version set: a committed file of `<program> <version>` lines a project
carries, named `.jig-versions`. It is milestone 614's ruling 4, and the capability's name is
provisional, as the ruling says. For each program it names, it selects among the versions already
installed.

The file holds version strings, because people write it. It is resolved to digests through the live
table at activation, so enforcement stays digest-authoritative one layer down (ruling 1). That is
also why little is validated: a version set vouches for nothing and can run nothing. A cloned
repository can ask, and still cannot install or run uninstalled bytes.

The shell consults the nearest set at or above the working directory, asdf-style, on calef's
usability ruling. That walk is over directories for one file name, not over program names, so
§229's one answer per name stands and there is no search order.

A set that names a program twice is the project's own contradiction, and the first line wins. That
resolution is order, which a reader may not expect. Nothing refuses it, because the worst a
contradictory line can do is select a version the table answers for anyway. The dev tool that
installs what a set asks for is ruling 4's follow-on and is not built, so a set naming an
uninstalled version can only ever produce the divergence notice.

## Completion

`swish::complete` is DECISIONS §227 (how Tab reaches the shell), option D. The shell edits its own
line, so a Tab reaches the process that holds the authority completion needs. Completion offers
exactly what that authority can name, and nothing more:

- In command position, a builtin (`grant_plan::BUILTINS`), a program the image names
  (`grant_plan::Program::ALL`), or an installed program's bare name (§229 B2), read from the live
  installed-programs table on each Tab.
- Anywhere else, or for a word with a `/` in it, an entry of the directory the word's lead names,
  read the way `ls` reads it. That costs `ENUMERATE`, the same right `echo *` costs, so a shell that
  cannot list a directory cannot complete in it either.

Nothing in the crate reads a directory. `Matches` is fed candidates by the program, twice when it
must list them: once to decide, once to print. Holding no names is deliberate. The shell's stack
has run out before (notes/pipes.md), and a listing streamed a second time costs one more `READDIR`
walk rather than a buffer.

The module's BUGS, in full:

- A quoted word is not completed. Tab inside `'...` does nothing, because a completion would have
  to decide whether to close the quote, and nothing yet needs that.
- A name with a space in it completes to text the parser splits in two, since nothing quotes the
  insertion. Files here rarely have one; a completion that quoted would fix it.
- Completing an argument does not ask the program's manifest what the argument is, so `rm <Tab>`
  offers directories and files alike. Manifest-aware completion is a proposed follow-on.
- A name that is both the image's and installed is offered once and refused when run.

## What `time` may say

`swish::timing` renders milestone 86 (`time`: the shell times a command). The number is a
difference of two reads of the monotonic counter, as §72 (`time` needs no clock) rules, so `time`
has no clock to be refused and nothing to disclaim. A monotonic counter cannot be stepped. The one
reason a line is not timed is a usage error, `time` with nothing after it.

A duration is printed in one of three units, seconds, milliseconds or microseconds. A spawn on this
system is milliseconds and a builtin is microseconds, so those are the two a person meets. Seconds
is there because a long job exists, and `1234567.891 ms` is not a number anybody reads.

Three digits after the point is a rendering choice, not a resolution claim. The counter ticks at
62.5 MHz on the aarch64 board (16 ns) and 10 MHz on the RISC-V one (100 ns). So the last digit of a
microsecond reading is real on one board and rounded on the other. What it must never do is print
more precision than the arithmetic has, which is why there is no nanosecond unit at all.

`real` is Unix's word for wall-clock time, here from the shell deciding to run the line to the line
being over. It is not CPU time. There is no `user` or `sys` row, because nothing in this kernel is
asked how much processor a thread consumed. That is a scheduler fact, unqueried today; if it
arrives it is another row, not another command.

## What a word designates

`swish::words` decides whether a word is a designation to resolve or text to print. It asks
`glob::has_magic` first, because an `echo` word is arbitrary text: `a:b` is not a path and must not
be refused as one. A pattern anywhere but the last component is refused, since selecting
directories to walk is an authority question (notes/glob.md).

The shell expands and then plans, as Unix does. The consequence differs: the planner must see the
set rather than the pattern, because the endowment is the set. A quoted word designates itself, and
that is the whole of what quoting does to authority. It is a narrowing: `rm "*.txt"` hands over one
name where `rm *.txt` hands over the set.

`echo` is the half of globbing that costs no authority. Words with no magic are copied byte for
byte, spacing included, and only a pattern is replaced by what it designates. A quoted word prints
what is between its quotes and never expands. `echo "$?"` prints the two characters, and the bare
`$?` prints the status. A pattern that matched nothing stops the line rather than printing itself,
which is the answer `rm` gets too. If `echo` printed the pattern where `rm` refuses, the two would
disagree about what the line designates, and that is the one thing the pairing exists to rule out.

A program that hears words (milestone 205 (how a foreign program is told what to do), §170 clauses
2 to 5) has nothing expanded. Its words are designated instead, and the planner turns them into
one directory grant at the current directory. Every word after the program's name is asked, in
`grant_plan::each_word`'s reading, so the words designated are the words the program hears:

- a word shaped like an option (`-i`, `--color=never`) designates nothing;
- an absolute path or one that starts with `..` designates nothing, because the program's root is
  this directory and `std` refuses both before they reach the wire;
- `.` (or `./`) designates this directory itself, which is calef's N1 ruling
  (2026-09-27T06:27Z): a line that names no file grants nothing, and `rg pattern .` is how a person
  says "here";
- otherwise its first component is looked up with the shell's directory read. A name that is here
  is designated. One that is not is inert bytes (a pattern, a count), unless the program may
  create, which is §170's clause 3.

So `rg needle src/main.rs` designates `src`, and the program can read all of `src`, not only
`main.rs`. The caretaker filters at this directory and nowhere below, as milestone 205's BUGS
records. A pattern that happens to be a file's name is designated too, read-only. An unquoted
pattern is refused for such a program by `grant_plan::check_words`, because it would designate
names and grant nothing they name.

Substitution has exactly one seam, `pieces`. The line is split on the whitespace that was typed,
and a substituted value is one word whatever its bytes are, so word splitting cannot happen here
(§141 (application is grant)).
