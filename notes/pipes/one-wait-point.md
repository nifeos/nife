# One wait point, and what it decides about a line

*An appendix to [`notes/pipes.md`](../pipes.md), which is the page to read. This file holds the
draining lane of 2026-08-04: the two bugs, the rendezvous limit, the barrier declaration and the
ways out. It was moved here verbatim from the main page on 2026-09-25 (UTC), under [§212 (a prose
budget)](../../design/decisions/0212-a-prose-budget-for-every-document.md). The directory
`notes/pipes/` and this file's stem are provisional names, minted that day by the lane that split
the file. Naming is calef's.*

The records this file cites by number:

- milestone 40 (documentation as a system service)
- milestone 107 (the socket contract learns to accept)
- §67 (a program's second stream is a declaration)
- §106 (take the `terminal_sink_caretaker` narrowing)
- §55 (the file behind a `>` is the shell itself)
- §26 (the fault endpoint)


## One wait point, and what it decides about a line

*Written 2026-08-04, by the lane milestone 40 handed this to. Everything [the main
page](../pipes.md) describes was built and proved with one program that reads a stream, and `wc`
turns out to be the one shape of reader that hides the constraint. The second reader found it in an
afternoon.*

### The three limitations, and only two of them were bugs

Milestone 40's `doc` reads markdown on its input and writes it rendered on its output. At the prompt
it did three things and none of them showed a page:

| line | what happened | what it was |
|---|---|---|
| `doc page.md \| wc` | `0 0 0` | a bug, and a silent wrong answer rather than a failure |
| `doc page.md > out.txt` | `0 0 0` | the same bug |
| `doc page.md` | the prompt never came back | not a bug |

And a fourth, underneath: even with a page arriving, the shell printed at most 512 bytes of it.

### The two bugs

A named file did not reach a pipeline stage. `wc report.txt` is `wc < report.txt` with the
operator left out, and the resolution is the planner's, because deciding that a trailing
positional is a stream needs the manifest (`InputSpec::Required` plus a bare token, in
`grant_plan::plan_against_with`). `run` read that answer off the plan. `pipeline` did not: it wired
the head's input off the `Line`, which has no `<` on it, so a planned `Source::File` was thrown away
and the stage was spawned with an empty input slot.

It did not hang, and that is the part worth keeping. A `receive` on an empty slot answers `NoSuchSlot`
rather than blocking. The error word's top byte is an opcode `byte_sink_contract` does not define,
so it decodes as `Msg::Malformed`. And every reader in this tree treats a malformed message as the
end of the document, because a page silently missing a paragraph is worse than a page that stops.
Three correct local decisions compose into a stage that runs to completion over nothing and reports
an honest count of an empty stream. `doc page.md | wc` answering `0 0 0` is not a viewer that failed
to render, it is a viewer that rendered the empty document it was given.

That is why nothing caught it, and it is the shape to recognize: an empty capability slot on a
byte stream reads as an empty stream, everywhere, by construction. There is no reader in this
system that can tell "nobody granted me an input" from "the input was empty", because the sink
contract deliberately gives a reader nothing to ask with. The check that can tell them apart is the
shell's, at plan time, which is where `InputSpec` already lives.

Nothing caught it because the only line anybody had typed with an operand *and* an operator on it
was `wc out.txt`, which is one stage and goes down `run`. `wc out.txt | wc` is now in the guest test
and in `script/swish-check`, and its expected answer is derived from the line above it rather than
written down.

The output ceiling was two numbers for one job. Printing stopped at 32 sixteen-byte messages
and a `>` allowed 1024, so the same program's output was cut at 512 bytes on the screen and whole in
a file, with nothing saying which had happened. The number exists to bound a program that never
announces end of stream, and that bug is not more tolerable when the bytes are going into a file.
There is one number now (`MAX_OUTPUT_CHUNKS`, 64 KiB) and the reason on record for the split ("a
file is where output goes when there is too much of it to read") was a policy about length, which
the same comment disclaimed in its next sentence.

### The third one is the rendezvous, and no amount of shell code fixes it

A process here has exactly one wait point. `SEND` blocks until a receiver takes the message,
`RECEIVE` blocks until one arrives, and there is nothing else: no select, no receive-on-a-set, no poll,
no timed wait. Milestone 51 (wall-clock time) records the timed-wait fork and design/roadmap/106
is NOT-STARTED, which makes it a kernel-surface decision rather than something a lane may reach for.

So when the shell is the thing feeding a stage, it cannot also be the thing receiving from it:

```text
  doc page.md

  shell ──feeds the file──►  doc  ──renders as it reads──►  shell
        blocked in SEND                                     never gets here,
        because doc is                                      because it is still
        blocked in SEND                                     in the SEND above
```

No interleaving schedule fixes this, and that is worth stating because it is the first thing
anybody reaches for. Alternate send-then-receive and it deadlocks the moment the stage reads twice
before it writes (both sides in `RECEIVE`). Alternate the other way and it deadlocks the moment the
stage writes twice before it reads. The shell cannot know which, and *the whole point of the sink
contract is that it cannot ask*: a writer holds an endpoint and has no message that would tell it
what is on the other end. The property that makes redirection one grant is the same property that
makes the schedule unknowable.

This is milestone 107's wall from the other side. That lane's listener re-arms before `ACCEPT`
returns, so it serves connections one after another; simultaneous service still needs threads or a
select-shaped wait, and it recorded that rather than faking it. Same sentence, different verb.

### What makes a line runnable anyway: one barrier

A stage that reads to the end before it writes anything absorbs the stream. One of those
anywhere in a chain is enough: everything upstream of it can stream freely, the shell's feed
completes, and only then does anything travel back.

```text
  doc page.md | wc          shell ──►  doc  ──►  wc  ──►  shell
                                                 ^
                                       the feed finishes here, because `wc`
                                       says nothing until end of stream
```

`wc` is a barrier and is the only one in this tree, which is exactly why nobody found this until a
second reader existed. So it is now a manifest declaration, in the shape DECISIONS §67 used for the
second output stream: `InputSpec::Required { writes_while_reading }`, carried onto the `Endowment`,
and checked over a whole planned line by `grant_plan::check_chain`. That is because the barrier may
be any stage in the chain and no single stage can answer the question.

A line with no barrier is `Refusal::NoReaderButThisShell` at the prompt, before any file is
opened, which matters because a `>` truncates and a line that will not run must not have emptied
one:

```text
$ doc page.md
  doc: writes while it reads, and this shell can only wait on one thing at a time: give it a
  reader that is not this shell, as in '| wc'
```

Updated 2026-08-22 (DECISIONS §106): this transcript is no longer what `doc page.md` answers.
The refusal above is still exactly right for the analysis on this page (a shell that is both the
chain's feeder and its reader has to refuse, or hang). But the analysis had an unstated assumption:
that the tail's reader, if there is one, is always this shell. §106 gives an unredirected tail
somewhere else to be read (`terminal_sink_caretaker`), so `doc page.md` renders now; only the
redirected shape (`doc page.md > out.txt`) still meets this refusal, because a `>` still comes back
through this shell (DECISIONS §55). See notes/documentation.md's `BUGS` for the current transcript.

It is the same kind of sentence as `InputRequired`'s, one level up: the manifest knows something
about the program, so the prompt knows something about the line. And it is the same trade, because
a shell that hangs is strictly worse than a shell that refuses. This kernel has no way to interrupt
a process blocked in a rendezvous send, so the prompt is gone until the machine is rebooted.

Nothing in this tree declared `true` until milestone 40's `doc`, which needed one field set and
is now the reachable proof; see the transcript below (`$ doc motd`, updated by the same decision).

### Verified against the viewer, on a branch that is not merged

This lane's gates run without a streaming filter, because there is not one on `main`. So it was
also run against milestone 40's branch merged in, with `doc`'s manifest declaring
`writes_while_reading: true` and nothing else changed. Both ISAs, at a real prompt, through the
real progenitor. The `motd` file on the fixture image is 70 bytes of markdown:

```text
$ wc motd
  1 12 70
$ doc motd | wc
  1 12 72
$ doc motd
  doc: writes while it reads, and this shell can only wait on one thing at a time: give it a
  reader that is not this shell, as in '| wc'
$ doc motd > page.txt
  doc: writes while it reads, and this shell can only wait on one thing at a time: give it a
  reader that is not this shell, as in '| wc'
```

Updated 2026-08-22: the third line's answer changed; the fourth's did not. DECISIONS §106 gives
`doc motd`'s output somewhere to go that is not this shell, so it renders now instead of refusing;
`doc motd > page.txt` still refuses, because `>` still routes through this shell (DECISIONS §55).
The transcript above is kept as the record of what milestone 40's own lane found before that
decision; see notes/documentation.md's `BUGS` for the current one.

The second line is the whole claim, and the two numbers are the assertion rather than the fact that
it ran. The same 70 bytes went in and 72 came out, because the renderer wrapped a paragraph and put
a newline where the source had none. `0 0 0` is what that line answered before, and a viewer that
rendered nothing would still say it. The first line is the control: `wc` is the barrier, so the same
operand at the head of the same pipeline worked all along.

The third line was the wall, said out loud instead of hung on, and is now the fourth line's alone. A
person who wants a page on the screen no longer needs `| wc` in front of it to have one. What they
get for the redirected shape is still a sentence naming what to type instead, which is worse than a
pager and far better than a prompt that has to be rebooted.

### The two ways out that were not taken, and the third that was

Neither of the first two is taken here; they are recorded because the refusal above was a wall and
not an answer, and a person who wants `doc page.md` to render on the screen is asking a reasonable
thing. A third way, found later and not by this page, is taken: see below.

A pull-based source, which is the exact answer to the constraint. The reason the shell needs two
wait points is that it holds two channels to one child. Make it one: hand the stage a single
endpoint on which it `CALL`s for input and `SEND`s output, and the shell's loop is one `RECEIVE_CAP`
that either replies with bytes or writes bytes out. One wait point, arbitrary interleaving, no
deadlock ever. What it costs is everything [this note](../pipes.md) calls a finding: "a source is
the sink contract received rather than sent" stops being true, `<` and the right-hand side of `|`
stop being one convention. The read end and the write end stop being separate capabilities, so the
program on the right of a `|` could write back up its own input. That last property is called
load-bearing [in the wire
section](declarations-and-wire.md#the-wire-and-the-one-thing-the-progenitor-had-to-learn) and it
would be gone. A design fork, and calef's.

A buffering stage, which is the answer the roadmap already predicted. A barrier can be *inserted*
rather than declared: a component that speaks the sink contract on both sides, takes a memory grant,
and absorbs what it is given. That is precisely the shape ["Buffering:
measured"](buffering.md#buffering-measured-and-the-answer-is-to-build-nothing) said a buffer would
arrive in if it earned its place, and this is the case that earns it. The measurement there says a
buffer costs roughly double and buys decoupling, not bandwidth, and decoupling is exactly what is
wanted: that section's own caveat is that the benchmark did not measure the case buffering is for.
It needs a program, a name, a progenitor entry and a `Program` id, and a document larger than the grant
deadlocks again, so the bound has to be an honest part of it.

What is not a way out is an adapter process at the file end. The obvious move for `doc page.md >
out.txt` is to give the writing end to a component so the shell is only the producer. It does not
work, for the reason ["The file behind a `>` is this
shell"](the-file-end.md#the-file-behind-a--is-this-shell-and-that-was-not-the-plan) already gives:
`filesystem_contract` shares one page between the FS server and its clients, and this line has the
shell reading the filesystem while the adapter writes it. Same race, same page, and no ordering fix.

The way that was taken: hand an unredirected tail's output to an adapter that already exists
(DECISIONS §106, 2026-08-22). Not an adapter at the file end, which the paragraph above rules out,
but at the screen end. It is `terminal_sink_caretaker`, this milestone's own second-stream default
(DECISIONS §67), given the tail's *primary* output too when the line named neither `>` nor `|` for
it. This page did not find this option; the milestone 40 roadmap block did, later, and
notes/tail-output-narrowing.md worked it through the six-questions way before calef took it. It
costs neither of the two properties above (the pull-based source's separate-rights property, the
buffering stage's extra rendezvous). The child's output slot stays exactly as opaque to it as a `>`
or `|` destination always was, so nothing about what a program declares changes. The shell that used
to drain the tail's bytes now waits on DECISIONS §26's kernel exit-delivery instead. See
notes/documentation.md's `BUGS` for the mechanism and its one carried cost, the caretaker-hop
display race.
