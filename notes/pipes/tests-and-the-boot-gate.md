# What the tests prove, and the gate for the boot

*An appendix to [`notes/pipes.md`](../pipes.md), which is the page to read. This file holds what the
guest tests assert on both ISAs and why `script/swish-check` exists. It was moved here verbatim from
the main page on 2026-09-25 (UTC), under [§212 (a prose
budget)](../../design/decisions/0212-a-prose-budget-for-every-document.md). The directory
`notes/pipes/` and this file's stem are provisional names, minted that day by the lane that split
the file. Naming is calef's.*

The records this file cites by number:

- milestone 86 (`time`)
- milestone 31 (a capability shell)
- milestone 51 (wall-clock time)


## What the guest test proves, on both ISAs

`kernel::user::pipeline_tests` wires the real shell binary in a role that reads a script instead
of a keyboard, with the interactive endowment slot for slot. The kernel plays the two parties on the
other ends: it serves the terminal contract and collects every byte the shell prints, and a second
thread serves `grant_plan::spawnproto` as the progenitor.

So the assertions are made against what a person would see, and the headline one is not a
constant:

- `date` alone: the shell prints N bytes.
- `date | wc`: `wc` reports N bytes.

Same ELF, same argument, two destinations, and the second number has to be the first one's length.
Comparing against the observed first arm rather than a literal is what makes it hold whether or not
the boot has a clock and whatever `date` decides to say.

### And the same claim on the input side

`kernel::user::sink_tests::one_reader_two_sources_and_the_same_answer` is the mirror, and it is what
says the source convention is real rather than merely chosen. One `wc` ELF, spawned twice with
identical grants except for what is behind slot 1:

- a pipe: the kernel sends the transcript on an endpoint itself, sixteen bytes at a time, then
  `OPERATION_EOF`. That is exactly what a program on the left of a `|` does.
- a file: the same transcript is written into a real file on the real RedoxFS image by `sink`'s
  file role, then read back out by its source role, which streams it over the same contract. That is
  `wc < report.txt` minus the shell that would name the file.

The second arm crosses two userspace processes, an FS server, a block server and a virtio disk; the
first does not leave the kernel's address space. The answers must be equal, and must equal what
the transcript actually is, because two arms broken the same way would satisfy equality on their
own.

### And the redirections, at a prompt that holds a filesystem

`kernel::user::redirection_tests` is `pipeline_tests` with one more capability: the same shell ELF,
the same four slots, plus a directory at slot 4 narrowed by a `fs_subtree_caretaker` to one subtree
of the real RedoxFS image. Three claims, and none of them is "it printed something":

- One builtin, two destinations. `ls > out.txt` writes a listing into a file and prints nothing;
  the `ls` after it prints the same listing; `wc < out.txt` has to agree with what was printed, once
  the prompt's two-space indent comes off. The expected counts are *derived from the transcript*, so
  a `>` that dropped every second byte fails even though it would still produce a file.
- One program, two destinations. `date` printed and `date > date.txt` counted, and the file's
  byte count has to be the length of what was printed.
- The refusals a directory does not rescue. `wc < nosuch.txt` is the filesystem's own sentence (a
  `<` does not create, because a `wc` that truthfully reported zero for a file that is not there is
  a number a person would believe), and `least_authority_demo 9 > out.txt` is still
  `NotAByteStream`.

The pair of witnesses is the capability argument made twice with one binary:
`pipeline_tests::a_redirection_a_shell_cannot_back_is_refused_rather_than_dropped` refuses because
slot 4 is empty, and this writes the file because slot 4 holds a directory. Neither is a branch in
the shell.

## The gate for the boot itself, which is what runs the real progenitor

The guest tests above wire the shell from the kernel: it serves the terminal contract and, on a
second thread, `grant_plan::spawnproto` in place of the progenitor. The shell cannot tell the
difference, and that is the problem. `components/src/progenitor.rs` is not the same code, so a
change that broke the real spawn path failed nothing, and the interactive boot is the only
thing that runs it.

That cost this milestone three manual bisects. All three presented as a boot that printed nothing at
all. They were the shell's terminal page colliding with `FILE_VA_CLIENT`, the progenitor's
sixteen-slot capability table overflowing when the kernel handed it two more grants, and four stack
pages being one deep call short of the redirection path.

And the gap runs the other way too, which milestone 86 found. The kernel's stand-in progenitor put a
spawned program's argument in `arg0`; both real inits put it in `arg1`, and
`components/src/least_authority_demo.rs` reads `arg1`. Nothing failed for two milestones, because no
line in either script ever spawned a program that *takes* an argument: `date`, `wc` and `echo` take
none, and `least_authority_demo 9 | wc` is refused at the prompt before anything is built. The first
script to type `time least_authority_demo 3` got `3*3 = 0` back. So a harness that "the shell cannot
tell apart" can still be wrong in a way no shell would notice, and the fix is the same one as above:
the scripts have to exercise the shapes the boot exercises.

`script/swish-check` closes it. It boots that system on both ISAs, types five lines at the prompt,
and reads the answers back:

```text
echo hello world | wc      -> 1 2 12   the bytes went through a real spawned process
echo hello world > gate    -> nothing  the same bytes into a file the shell backs
wc < gate                  -> 1 2 12   ... and they are the same bytes
echo hello world >> gate   -> nothing
wc < gate                  -> 2 4 24   ... exactly twice, so `>>` kept the first line
wc gate                    -> 2 4 24   milestone 31: the name IS the grant, same bytes
wc                         -> refused  ... and with no name there is nothing to read
caps wc gate               -> input    ... and the preview says which file, and how
date                       -> ...UTC   milestone 51's wiring: a clock init endowed
caps date                  -> cap 1    ... and the visibility surface names it
```

One line would have caught all three bugs. Five is still seconds, and it walks the whole endowment:
a spawn through the real progenitor, the FS service the real progenitor narrowed into the shell, and
both redirection operators.

The `wc gate` trio is milestone 31's headline checked at the one interface a human touches. Its
answer has to equal the `<` line's, because it is the same designation with the operator left out.
The pair is what makes that a claim about the machine rather than an assertion: one line reaches the
file through an operator and one through a name, so if they disagree, one of them opened something
else. `wc` alone is the negative control the pair would be weaker without, refused at the prompt
before anything is spawned.

The last two arrived with milestone 51's wiring lane and check a different half of the same boot.
`date`'s answer cannot be a constant, so the assertion is `UTC`. `Format::Human` ends in the
offset's name and neither unknown-clock sentence contains those three letters. So one word fails the
gate if the clock service did not run, if the kernel granted the progenitor no page, if the
progenitor did not endow `date`, or if `date` was handed a page nobody published to. `caps date`
then requires that the shell's own visibility surface names the capability, because `caps` claims to
print a process's whole authority and a clock endowed but not printed would make that claim false.

Two things the machine corrected while it was being written, and both are the kind of thing a
harness gets wrong quietly:

- The line editor echoes a character the moment it arrives, whether or not the shell has asked
  for a line yet. So a harness that types ahead produces a transcript in which a command's echo
  appears *before* the `$ ` that should introduce it, and then fails to find its own echo. The gate
  waits for the transcript to end in a bare prompt, which is the unambiguous "ready".
- The script types `wc < gate.txt` twice on purpose, so every search is anchored at a cursor
  rather than run over the whole transcript. An unanchored search found the first answer for both
  lines, and would have passed a `>>` that truncated.

It drives `helpers/qemu-runner-aarch64.sh` directly rather than `cargo run`, so the process it owns
is QEMU (the runner `exec`s it) and the kill lands on the emulator instead of on cargo. It is not
part of `script/test`, because it builds a second kernel and boots it twice.
