# Pipes and redirection: `>`, `<` and `|` are one substitution

*Milestone 50, the operators lane and its closure. `crates/grant_plan/src/line.rs`,
`components/src/wc.rs`, `components/src/swish/`, `components/src/date.rs`, `components/src/terminal_sink_caretaker.rs`,
`components/src/progenitor.rs`, `fixtures/src/hello.rs`, `crates/grant_plan/src/spawnproto.rs`,
`script/swish-check`. The protocol half is notes/sink-protocol.md and you should read that first.*

**All five operators run at a real prompt on both ISAs.** `|` landed first; `>` and `<` needed a
boot in which one shell holds both a filesystem and a spawn channel, and building that turned up a
reason the file end of a redirection cannot be a separate process. That finding is the section
["The file behind a `>` is this shell"](pipes/the-file-end.md),
and it is the most reusable thing in this note. `>>` is the cheapest of the four,
which is that finding paying out: the shell already holds the file, so append is one bit about how
it opens one.

`2>` came last of all, on 2026-08-03, and it is the one operator that is not a spelling for
something the system already had: it needed a second stream to exist first. It exists per program,
by declaration (DECISIONS §67 (A program's second stream is a declaration, not)), which is what makes the digit a familiar spelling rather than a
number everybody has to agree on. See ["`2>`: built as a
declaration"](pipes/second-stream.md).

**The draining lane came after all of them, on 2026-08-04**, and it is the only one that was raised
by somebody trying to *use* this. Milestone 40 built a documentation viewer, could not show a page
at the prompt, and spent three boot-gate failures finding out that all three reasons were here. Two
were bugs and are fixed; the third is not a bug at all, and naming it correctly is the reusable
part. See ["One wait point, and what it decides about a
line"](pipes/one-wait-point.md).

## What this lane had to add, which was less than it looks

The protocol lane established that a program's output destination is a capability its spawner
chose, and unified the four "write these bytes there" protocols into one. After that, `>` and `|`
are not two features. They are two spellings of *put a different capability in slot 0*, and the
whole of this lane is the grammar that lets a person choose it and the wiring that carries the
choice from the prompt to the child.

The kernel did not change. No new object, no new syscall, no `dup2`, no pipe buffer.

## The one-line summary of the mechanism

```text
  date | wc

  shell: SPLIT a region off its own budget
         RETYPE one page of it into a Rendezvous          <- this is the pipe
         spawn date, delegating the rendezvous  WRITE      <- into date's slot 0
         spawn wc,   delegating the rendezvous  READ       <- into wc's slot 1
         read wc's answer off its own result rendezvous
         DESTROY the region                              <- this is what ends a stalled writer
```

`date` is not recompiled, not told, and cannot ask. The endpoint in its slot 0 is the same kind of
object that was there before, held with the same right.

## The grammar, and the three rules that are not Unix's

`crates/grant_plan/src/line.rs` splits a line into stages and the names on the ends. It is host-tested and
runs in milliseconds; that is where nearly all of this lane's tests are.

```text
line  := stage ('|' stage)*
stage := <command words> ('<' name | '>' name | '>>' name | '2>' name | '2>>' name)*
```

Three refusals that bash does not make, each because a line should mean what it looks like:

- **A redirection goes last in its stage.** `wc < report.txt`, never `< report.txt wc`. Bash accepts
  the second; refusing it is what lets a stage's command text be a *slice* of the line rather than
  something reassembled, which matters in a shell with no allocator.
- **Only the first stage takes input and only the last is redirected.** `date > f | wc` says where
  `date`'s bytes go and then pipes them somewhere else. That is two answers to one question, so it
  is refused rather than resolved by a precedence rule nobody should have to remember.
- **A redirection names one file.** `date > *.txt` is refused where the token is read, not expanded
  and then counted. Even a pattern that matched exactly one name would make what the line writes to
  depend on what is in the directory.

## What the shell checks before it spawns anything

A program's manifest says what it reads and writes, so the prompt can refuse a line that would
misbehave. `OutputDeclaration` says whether a program writes a byte stream: `least_authority_demo 9 >
out.txt` is `NotAByteStream`, because that program answers in a register. `InputDeclaration` says whether
it reads one. A `wc` with neither a `<` nor a pipe is refused rather than left blocked forever, and
`date < report.txt` is `InputForbidden`.

A source is the sink contract received rather than sent, so `<` and the right side of `|` are one
convention. A pipe's ends travel as `WRITE` and `READ`, so the program on the right of a `|` cannot
write back up its own input. A builtin can lead a pipeline, because the shell itself can be a
writer. The declarations, the input slot's shape and the spawn wire are in
[their appendix](pipes/declarations-and-wire.md).

## The file behind a `>` is this shell, and that was not the plan

The shell backs the file end of a redirection itself instead of asking the progenitor for an
adapter process. The FS contract shares one page between the server and its clients, and two client
processes using it at once race. `ls > out.txt` would corrupt both the listing and the file. So in
`date > out.txt`, `date` writes to the shell's own endpoint and the shell drains it into the file.
The program still holds one endpoint with `WRITE`, and cannot seek, truncate or stat.

That made `>>` one bit: append is only where the shell starts its offset when it opens the file.
`>` truncates before the command runs, so `ls > out.txt` lists `out.txt`. The race, the costs and
the `>>` design are in [the file-end appendix](pipes/the-file-end.md).

## `2>`: built as a declaration (DECISIONS §67)

A program with diagnostics declares a second output in its manifest (built 2026-08-03). `2>` names
where those bytes go, and aimed at a program that declares none it is refused at the prompt. `date`
is the only declarer. With no `2>`, the second stream goes to `terminal_sink_caretaker`, so a
complaint reaches the screen without passing through the shell. `date > when.txt` therefore never
writes a complaint into the file. The progenitor inserts the stream at the slot the manifest names,
not at the next free one.

What was built and what building it found are in [the second-stream appendix](pipes/second-stream.md).
The analysis written while the fork was open is in [the fork appendix](pipes/second-stream-fork.md).

## One wait point, and what it decides about a line

A process here has one wait point: no select, no poll, no timed wait. When the shell feeds a stage,
it cannot also read that stage's output, so a filter that writes while it reads would deadlock the
prompt. A line runs if some stage reads to the end before writing (a barrier; `wc` is the only one).
It also runs if its tail's output goes to the terminal's sink rather than back to this shell, under
§106 (the `terminal_sink_caretaker` narrowing). A line with neither is
`Refusal::NoReaderButThisShell` at the prompt, before any `>` has emptied a file. The draining lane
of 2026-08-04, its two bugs and the ways out are in [the wait-point
appendix](pipes/one-wait-point.md).

## Buffering: measured, and the answer is to build nothing

Measured on 2026-08-03 under HVF, `a | b` costs 1146 ns per sixteen bytes (13.3 MiB/s). A macOS
pipe at the same write size costs 348 ns. The gap that matters is the sixteen-byte message, not the
lockstep, and a buffering process would add a second rendezvous per message. So nothing was built.
The table and its caveats are in [the buffering appendix](pipes/buffering.md).

## SIGPIPE, and why the pipeline gets its own region

Deleting every capability that names an endpoint does not destroy the endpoint: the object lives
in a page of an untyped region, and only reclaiming the region frees it. So a pipeline whose reader
has finished while its writer is still blocked in a `SEND` would leave that writer blocked forever.

Each pipeline therefore takes its own region, split off the shell's budget, and the shell `DESTROY`s
it when the line is over. That is what turns a producer's next `SEND` into `abi::Error::Gone`, which
is `SIGPIPE` as a return value. The classification itself is asserted by value in
`kernel::user::sink_tests`.


## The boot, the tests and the gate

The kernel brings up the block and FS servers before the progenitor, and the progenitor narrows the
FS endpoint into the shell at slot 4. On a machine with no disk the same shell ELF runs with no
directory. `kernel::user::pipeline_tests` and `redirection_tests` run the real shell with the kernel
serving the far ends, and assert what a person would see. `script/swish-check` boots the real
progenitor on both ISAs and types at the prompt. The boot's layout and the four stack overflows are
in [the boot appendix](pipes/the-boot.md). The tests and the gate are in
[their appendix](pipes/tests-and-the-boot-gate.md).

## EXAMPLES

At a real prompt, on the RedoxFS fixture. `script/console` is the aarch64 spelling and builds
everything it needs (the FS server into the initrd, and the image, because the runner attaches the
disk only when the file is there). RISC-V has no `xtask` verb for the interactive boot, so it is two
commands:

```sh
script/console                                   # aarch64

cargo xtask initial-ramdisk-riscv                         # riscv64
NIFE_INITIAL_RAMDISK=target/initial-ramdisk-riscv.img NIFE_DISK=target/nifefs.img \
  cargo run -p kernel --features shell --target riscv64imac-unknown-none-elf
```

```text
$ ls
  globset/
  motd
  other/
  redir/
  rmtree/
  scratch
  sub/
$ ls > out.txt
$ wc < out.txt
  8 8 57
$ date > when.txt
$ wc < when.txt
  1 11 66
$ ls
  globset/
  motd
  other/
  out.txt
  redir/
  rmtree/
  scratch
  sub/
  when.txt
```

Read the numbers rather than the fact that it ran. `wc < out.txt` says eight lines where the
listing above it had seven, because `>` creates and truncates its file before the command runs,
so `ls` sees `out.txt` in the directory it is listing. That is Unix's order and it is worth seeing
rather than being told. The 57 bytes are those eight names plus a newline each; the terminal's
two-space indent is the terminal's manners and is not in the file.

The riscv64 prompt is the same session with different numbers, because that leg's image had two more
names on it from an earlier test run (`10 10 77` rather than `8 8 57`). The numbers being *different*
and still internally consistent is the better demonstration: nothing here is a constant anybody
pinned.

And `>>`, at the same prompt. `date` writes one line of 66 bytes, so two of them is 132:

```text
$ caps date >> when.txt
  date would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    (clock: this shell holds none to delegate, so it will report the time
     as unknown. the clock is init's to endow; no token on the line can.)
    output   when.txt  (this shell writes the bytes there; the program holds
             an endpoint and cannot seek, truncate, re-read or stat)
             this shell keeps what is already in it and writes after it
    arg    (none)
  reading the command is reading its whole authority.
$ date > when.txt
$ date >> when.txt
$ wc < when.txt
  2 22 132
```

Read the `caps` output rather than the counts. **The last line of the `output` row is the only thing
`>>` changes**, and it is a sentence about *this shell*, not about `date`. The `cap 0` row above it
is identical to the one `date > when.txt` prints, and so is everything else, because the two
spellings hand the child the same endpoint with the same right. That is the property `>>` was built
to be a test of, printed where a person meets it.

And the same at a prompt with no filesystem, which is the same binary:

```text
$ ls > out.txt
  you hold no such capability: this shell was granted no directory to narrow
```

The rest of the operators:

```text
$ echo hello world | wc
  1 2 12

$ date
  date: the time is unknown: this process holds no clock capability
$ date | wc
  1 10 63

$ echo hello world | wc | wc
  1 3 7

$ wc
  wc: reads an input stream: name a file, redirect with '<', or pipe into it

$ least_authority_demo 9 | wc
  least_authority_demo: does not write a byte stream, so there is nothing for > or | to redirect

$ date | date
  date: reads no input; there is no slot for those bytes to go in

$ caps date | wc
  date would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    output   an endpoint into the next stage. no file, no buffer, no object:
             the rendezvous IS the pipe
    ...
  wc would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    output   this shell's result endpoint (it reads the bytes and prints them)
    input    the previous stage's output
```

That last one is the demonstration the milestone owed: **`caps` can print where your output goes**,
because the destination is a capability rather than an integer with a convention attached. On Unix
the same question has no answer at that point, since fd 1 is whatever the shell's fd 1 happened to
be and nothing records what that was.

And the operand at the head of a pipeline, which is the line the draining lane fixed. Read the
numbers rather than the fact that it ran. `2 4 24` plus a newline is seven bytes and three words on
one line. So the second `wc` counting the first one's answer is what says the file reached the
**head** stage rather than nothing reaching it:

```text
$ wc gate.txt
  2 4 24
$ wc gate.txt | wc
  1 3 7
```

The failure it replaces is the one worth recognizing, because it does not look like a failure. The
head stage was spawned with an empty input slot, a `receive` there answers `NoSuchSlot` instead of
blocking, and that reads as end of document, so this line used to print `0 0 0` and mean it. **A
pipeline that reports zero of everything is a pipeline in which nothing was ever fed**, and it is
the only symptom an empty input slot has.

Three stages, to show the operand is the *head*'s and travels no further (`1 12 70` plus a newline
is eight bytes, and `1 3 8` plus a newline is six):

```text
$ wc motd
  1 12 70
$ wc motd | wc
  1 3 8
$ wc motd | wc | wc
  1 3 6
```


## BUGS

One line each.

- An unredirected tail stage's output goes to the terminal's sink adapter (DECISIONS §106), which
  carries a display race tracked at milestone 151 (notification objects). notes/documentation.md's `BUGS` has the worked case.
- `OPERATION_PRINT` carries eight bytes, so each sixteen-byte sink message is two calls to the terminal.
- `script/swish-check` is not in `script/test` or CI, and it is the only gate on the real
  progenitor.
- `fixtures/src/file_sink.rs` and `file_source.rs` are proven by `sink_tests` but not on the
  shell's path.
- The interactive prompt holds the image root, unnarrowed. That is a default, not a recorded
  decision.
- `rm` and per-file capabilities are not reachable from the prompt. The progenitor would have to
  build a caretaker per invocation.
- Slot 1 is the clock, the input source or the `--mem` untyped, and slot 0 moves behind a directory
  grant. That is safe only while no manifest declares two; a numbered slot convention is owed.
- A pipeline is full lockstep: every sixteen bytes is a rendezvous, with no buffer.
- A line whose bytes all come from this shell needs a barrier stage unless its tail goes to the
  screen. A redirected filter (`doc page.md > out.txt`) is still refused.
- That refusal reads as a complaint about `doc` when it is a fact about the line.
- `2>` works only on a program that declares a second stream, and only `date` does.
- Under a `2>`, a declaring program must finish its diagnostics before any output, or the line
  deadlocks. At the default destination the rule does not apply.
- A `2>` on a builtin is refused as "declares no second output", an odd sentence about `ls`.
- The shell's diagnostic endpoint is never destroyed, so a writer parked on it would stay parked.
  It cannot happen today.
- There is no here-document, and `<<` is refused as "a redirection needs a name".
- Quoting delimits and never rewrites, since milestone 67 (`swish` the language): there is no
  backslash escape, and `a"b"` is refused.
- `wc` has no `-l`, `-w` or `-c`.
- A `date` whose reader stopped early stays parked until its region is reclaimed.

## Appendices

| appendix | what it holds |
|---|---|
| [declarations-and-wire](pipes/declarations-and-wire.md) | `OutputDeclaration`, `InputDeclaration`, the input slot and the spawn wire |
| [the-file-end](pipes/the-file-end.md) | why the shell backs the file, and `>>` |
| [second-stream](pipes/second-stream.md) | `2>` as built, and what building it found |
| [second-stream-fork](pipes/second-stream-fork.md) | `2>` as it stood open |
| [one-wait-point](pipes/one-wait-point.md) | the rendezvous limit and the barrier declaration |
| [buffering](pipes/buffering.md) | the throughput measurement and its caveats |
| [the-boot](pipes/the-boot.md) | the boot's processes, its filesystem, and the stack |
| [tests-and-the-boot-gate](pipes/tests-and-the-boot-gate.md) | the guest tests and `script/swish-check` |
