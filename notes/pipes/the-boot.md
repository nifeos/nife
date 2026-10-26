# The interactive boot: its processes, its filesystem and its stack

*An appendix to [`notes/pipes.md`](../pipes.md), which is the page to read. This file holds the five
processes an interactive boot runs, how the filesystem reaches the shell, and the four stack
overflows. The lane that split the file moved it here verbatim from the main page on 2026-09-25
(UTC), under [§212 (a prose
budget)](../../design/decisions/0212-a-prose-budget-for-every-document.md). The directory
`notes/pipes/` and this file's stem are provisional names, minted that day by the lane that split
the file. Naming is calef's.*

The records this file cites by number:

- §67 (a program's second stream is a declaration)
- milestone 50 (pipes and redirection)
- milestone 31 (a capability shell)


## The five processes an interactive boot now runs

For a reader arriving at this file cold, the shape of the system `2>` completed:

```text
  init ──builds──► console server            reads a page, writes the UART
              ├──► line_editor               the terminal contract: OPERATION_WRITE, OPERATION_READLINE,
              │                              OPERATION_BYTES, OPERATION_INTRCOUNT, OPERATION_PRINT
              ├──► input driver              the UART receive interrupt, into the terminal
              ├──► swish                     the prompt
              └──► terminal_sink_caretaker             the sink contract, into OPERATION_PRINT
```

The fifth is new (DECISIONS §67, notes/sink-protocol.md), and it is the only one a person never
interacts with directly. It exists so "the terminal" can be a destination a capability designates
rather than a thing only the shell can reach, and its whole job is turning sink messages into
terminal prints. The progenitor keeps the endpoint it serves and hands it to any child whose
manifest declares a second stream, the way it hands out the clock.

## The boot that has a filesystem, which is what `>` was actually waiting for

The kernel brings the block server and the FS server up before the progenitor exists and hands the
progenitor the file-service endpoint plus the frame its clients map. The progenitor narrows both
into the shell: slot 4, and the page at `FS_VA`. Nothing else in the system changed shape; the shell
simply holds one more capability.

```text
  kernel  ── wires blk + redoxfs_server, drains both readiness sentinels ──┐
                                                                     v
          ── spawns init, granting the FS endpoint and the page (GRANT on both)
                                                                     |
  init    ── builds console, line_editor, input ──────────────────────── |
          ── builds the shell: slot 4 = the FS endpoint (WRITE, no GRANT)
                                          + the page at 0x60_0000
                              slot 5 = the clock page (READ, no GRANT)  [milestone 86]
                                          + the page at 0xd0_0000
          ── starts it with arg1 = the dir rights that endpoint carries
                            arg2 = the clock's slot, 0 for none         [milestone 86]
```

The clock is granted after the FS pair so a boot with no disk takes exactly the path it took
before it existed. That means its slot moves (4 without a disk, 5 with one) and the shell is told
the number rather than assuming one. See notes/time-command.md.

`arg1` is `0` on a machine with no RedoxFS disk attached, and then the shell is exactly the shell it
was: `Nav::empty()`, and every verb that would need a directory says so. The same ELF is in both
positions, which is why `kernel::user::pipeline_tests` (no slot 4) and
`kernel::user::redirection_tests` (slot 4) are each other's control.

Three things had to move to make room, and each is a fact worth keeping:

- The shell's terminal page is at `0xc0_0000`, not `0x60_0000`, which is `FILE_VA_CLIENT` and
  mapped by six programs. The terminal page is the one address only the shell and its progenitor
  know, so it is the one that gives way.
- The progenitor's capability table is sixteen slots and two more kernel grants overflowed it. The
  console's `build_child` had no slot left to retype an address space into and returned an error,
  which presented as a boot that brought the console up and then printed nothing at all. The
  progenitor now retypes the spawn and result endpoints after the drivers are built, and gives the
  console's three capabilities back before the shell, which is the same discipline the file already
  had one step later.
- Every child the progenitor builds gets eight stack pages, not four. The redirection path carries a
  parsed line, an array of planned endowments, a listing buffer and a file buffer by value, and four
  pages overflowed at the first `ls > out.txt` (a data abort one word below the lowest stack page).
  The kernel's own scripted wiring had already found the same floor and maps seven.

## The shell's stack, a fourth time, and what the pattern is now

Milestone 50 hit "a boot that printed nothing" three times and one of them was four stack pages
being one deep call short of the redirection path. Milestone 31's input operand hit the same
symptom in the same file and it is worth naming the shape rather than the instance.

`wc out.txt` reaches `run_pipeline` through `dispatch` -> `dispatch_one` -> `run`, where a line with
an operator on it reaches it through `dispatch` -> `pipeline`. One frame deeper, on a program whose
frames carry parsed lines and planned endowments by value, and the scripted wiring's six extra pages
were not enough. The result was a data abort one word below the lowest mapped page, mid-script, with
`far` equal to `sp`.

Two things came out of it, and the second is the one to keep:

- An `Endowment` is not small: it carries a whole `NameSet`. So a line that is one stage by
  construction does not declare `[Option<Endowment>; MAXIMUM_STAGES]`. `run_pipeline` takes a slice,
  and `&[Some(endow)]` is the same call without the array cost. The reflex of reaching for the
  full-width array makes a one-stage line pay for four.
- The two wirings gave the shell different stacks, and that was the real oddity.
  `system_initializer` maps eight pages and `pipeline_service` mapped seven, so the wiring that is
  *not* the one a person types was the smaller one, and it is where the overflow landed. They are
  both eight now. A test wiring with less headroom than the boot wiring will keep finding faults the
  boot does not have, which is a bug in the harness rather than a signal.

And a fourth time, for `2>`. A second `FileOut` on `run_pipeline`'s frame overflowed eight pages by
twenty-four bytes, in the same place, with the same signature. Each carries a 256-byte staging
buffer by value, because the filesystem's write unit is a page and the sink contract's is sixteen
bytes. All three wirings are twelve now, and the number went up by four rather than one on purpose:
every previous instance bought exactly enough headroom and the next change found the wall again. 48
KiB of address space per child is not worth a fifth bisect.

The pattern the four instances share is worth more than any of them. This shell's frames carry
whole values that grew as the milestone grew (a parsed `Line`, an array of `Endowment`s each
holding a `NameSet`, a listing buffer, now two file buffers). None of that shows up anywhere a
reader would look. The symptom is always a data abort one word below the lowest mapped page.
