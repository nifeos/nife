# `xargs`: batching at the bound (milestone 109)

*Name: provisional, minted 2026-10-11 (UTC) by lane/fourth-ten-longest-notes, after the prefix word
it is about, whose own name is provisional.*

An appendix to [`notes/glob-grant.md`](../glob-grant.md), which states the conclusion. Milestone 109
(`xargs`) built it.

## Why the bound is eight

Eight is a measurement, and sixteen was the first answer. The reasoning said sixteen names of
sixteen bytes is 256 bytes at each end and both ends can hold that. The machine disagreed twice. The
shell ran off the bottom of its stack planning one grant, by 256 bytes with two extra pages and by
768 more with four, presenting both times as a data abort on the shell's own `sp`. The 60-second
lost-wakeup watchdog followed (the test was still waiting for a report from a process that had
died). The cause is a set traveling by value through four frames a debug build does not collapse:
the expander holds one, `Expansion` carries one into `plan`, `designate` returns one, and the
`Endowment` that comes back carries one more. Eight names of sixteen bytes is 152 bytes a copy.

The fix was the one `spawn_fs_client`'s own comment already prescribes: its four-page cap says a
client needing more is a client whose frames want looking at, not a number that wants raising. So
the bound came down instead of the cap going up.

## The bytes a set carries

A name may be 252 bytes (milestone 946 (a granted file name can be longer than sixteen bytes)).
Eight slots of 252 would make every set cost eight times what ordinary names need. So a set has a
second bound, `nameset::NAME_BYTES`: 512 bytes of name, all its names together. That is two of the
longest names, or eight of 64. The shell's `NameSet` packs its names into those bytes. A copy is
544 bytes, where eight slots of 252 would be 2,040. Over either bound is `Refusal::TooManyNames`.
A batch is cut at whichever bound it meets first. Everything it leaves out sorts after everything
it keeps, so the next batch skips nothing.

The shell's stack was then measured, not scaled. On 2026-10-24 (UTC), `script/swish-check`'s
aarch64 leg booted the same tree with the shell's stack at each size. Twelve pages fault on the
boot's first pipeline. Sixteen fault in `installed/jig rollback`. Seventeen pass both boots. The
growth is mostly the file grants an `Endowment` holds, each with a name of up to 252 bytes. Three
things answer it:

| change | where | effect |
|---|---|---|
| the planner compiled optimized in a debug build | `Cargo.toml`, `grant_plan` | `plan_against_with`'s frame from 16,528 bytes to 5,504, below the branch point's 7,504 |
| a set packed, and no name kept to cut a batch | `grant_plan::expand` | `Expander` and `NameSet` copies in `Nav::expand` |
| the shell's own stack, 21 pages | `system_initializer::SHELL_STACK_PAGES` | seventeen measured and four of headroom |

The eight names stand. They are what a set costs in names, and the bytes are what it costs in
stack. Frame sizes come from the debug `swish`'s disassembly. Each is the sum of a function's `sub
sp` instructions, set beside the same build of the branch point.

## The sweep

The eight stands, and the answer at the bound is no longer a refusal. `xargs` is a third prefix
word in the shell (`caps` and `time` are the other two): its operand is a whole command line, and
it re-dispatches that line once per batch of what its pattern matched. The name is provisional, like
every name here.

Transcribed from a real prompt, `--features shell` on aarch64, over the `globmany` fixture's eleven
names:

```sh
$ echo globmany/m-*.txt
  that pattern matched more names than one grant can carry (at most 8)
$ xargs echo globmany/m-*.txt
  batch 1: m-00.txt m-01.txt m-02.txt m-03.txt m-04.txt m-05.txt m-06.txt m-07.txt
m-00.txt m-01.txt m-02.txt m-03.txt m-04.txt m-05.txt m-06.txt m-07.txt
  batch 2: m-08.txt m-09.txt m-10.txt
m-08.txt m-09.txt m-10.txt
  11 names, in 2 batches
```

And the half that says what would actually move, which is the acceptance evidence: the authority per
batch is exactly that batch, not the eleven the pattern matched and not the directory they live in.

```sh
$ xargs caps rm globmany/m-*.txt
  batch 1: m-00.txt m-01.txt m-02.txt m-03.txt m-04.txt m-05.txt m-06.txt m-07.txt
  rm would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 2  endpoint  dir      /globmany  (the directory holding m-00.txt ... m-07.txt)
           ...and nothing under it: no -r, so it cannot even look
  reading the command is reading its whole authority.
  batch 2: m-08.txt m-09.txt m-10.txt
  rm would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 2  endpoint  dir      /globmany  (the directory holding m-08.txt m-09.txt m-10.txt)
           ...and nothing under it: no -r, so it cannot even look
  reading the command is reading its whole authority.
  11 names, in 2 batches
```

(The two `cap 2` rows are elided in the first block only; the prompt prints all eight names.)

### Milestone 47's claim, quoted and then corrected

> `ARG_MAX` becomes a capability limit rather than a buffer limit. Unix's "argument list too
> long" is why `xargs` exists; here the ceiling is that you cannot hand a child a hundred thousand
> capabilities. The same failure with a more honest cause, and it wants the same answer (batching),
> so `xargs` earns its place for a better reason than Unix had.

The conclusion is right and the reason recorded for it is the weaker of the two available. "A more
honest cause" is a claim about why the *limit* exists, and a nicer-sounding limit is still a limit.

The claim worth making is about the batching. Unix's `xargs` is a workaround with no upside: if
`ARG_MAX` were infinite, nobody would ever type it. Here, batching bounds the authority in flight.
A single grant of five hundred names would mean one process holding five hundred names at once; a
sweep means at most eight are reachable by anything at any moment, and each batch's caretaker dies
with its batch. We would want this even if the carrier were unlimited, which is the whole
difference, and it is why the scope note's "the ceiling moves rather than disappearing" understates
the case: a better carrier would make `xargs` optional and would not make it pointless.

### Why it is a prefix word and not a program, which is the fork

A batching program has to hold, at once, at least the union of every batch it hands out. There is no
carrier for that union: that is the premise of the milestone, since a set larger than eight is
exactly what cannot be handed over. So an `xargs` program could only be given the directory plus a
pattern, and that is the second row of this note's own table (`the directory plus a name list`,
*over-grants catastrophically*) wearing a different hat. It would hold authority over every name in
the directory, including the ones the pattern did not match, in order to hand out subsets of the
ones it did.

Unix can make `xargs` a program precisely because its `xargs` holds nothing. A name in a pipe is
text there, and `rm`'s authority comes from its uid, so the batcher in the middle is not a party to
the delegation at all. Here a name in a pipe is still text, which means the analogue of `find . |
xargs rm` would spawn an `rm` holding no authority whatsoever: the pipe cannot carry the thing that
has to be batched. What can mint a per-batch caretaker is whatever holds the directory capability
with the right to delegate it, so the batching belongs where the authority already is.

It is not the table's third row either (*make `rm` a builtin*, "dodges the question"). `rm` stays a
program taking an attenuated grant. What became a builtin is the iteration, which is a property
of how the shell delegates and not of anything any command does. That is exactly what `caps` and
`time` are: prefix words about how a line is run.

And it is opt-in rather than automatic. A shell that silently swept a large match would make
`caps rm *.txt`'s single printed grant a lie, and would turn one command into N spawns with a
possible partial effect without the user asking for either.

### The resume point is a name, not a cursor

This is the decision the rest of batching hangs on, and the one Unix never has to make.

A sweep cannot expand the whole match and then chunk it, because holding the whole match is the
thing that is impossible. So batch *k+1* has to re-enumerate the directory, and it needs to know
where to carry on from. A cursor into the listing is invalidated by exactly what a batched command
is usually doing: `rm` takes eight names away and every later entry shifts. This note already
records that hazard from the other end, where `rm`'s namespace sweep re-reads from cursor 0 because
a set namespace is fixed and a real directory is not.

So a batch is the eight smallest matches strictly greater than the previous batch's last name, in
byte order. A name is stable under removal and under insertion both, so the rule terminates whether
the command destroys what it was handed or leaves it alone, and the watermark is one 16-byte name
rather than a structure. The host tests sweep a static directory and a shrinking one and assert the
same thing of both: every name exactly once.

The order is a total order chosen for resumability, not a display preference, which is why an
unbatched expansion still comes out in listing order. Sorting that one too would be a gratuitous
change to what `echo *.txt` prints.

### One command with a partial effect, and where it stops

A batched line is the one thing no unbatched line here can be: one command that can half-happen.
Batch one's `rm` has taken its names before batch two starts, and there is no rollback for a
removal.

So the sweep stops at the first batch that does not run, and says where the boundary fell:

```
  batch 3 did not run: 16 names were handed over in 2 batches, and nothing after them was attempted
```

A sweep whose *first* batch did not run says nothing at all, because the refusal printed above it is
the whole story and "0 names in 0 batches" is a sentence about an event that did not happen. Neither
does a sweep of exactly one batch: that is the line the user would have typed without `xargs`, and a
footer under it would claim an event.

Unix carries on past a failed invocation and reports 123 at the end. That is its mechanism talking:
its `xargs` cannot know what a child did to the names it was handed, so trying the rest is as good
as anything. Here each batch's set is printed before that batch runs, so a stop can name the
boundary, and what happened is a *prefix* of the match: a thing a person can hold in their head and
resume from. Carrying on would leave an arbitrary subset that only the transcript could reconstruct.
Batch four succeeding after batch three failed is the outcome the design is against.

The child sees none of this. Each batch is a complete invocation with a complete grant, and
nothing tells it that it is batch three of seven. Unix agrees by accident, because each `exec` is a
fresh process; here it follows from the model, since the grant *is* the argument and a batch's grant
is whole.

### Where it lives, and why that is one place rather than a command path

The sweep is a policy on `Nav::expand`, the shell's single expander, and not a second way to run a
line. Two things fall out. Every command path inherits batching for free: `echo`, an invocation, a
`caps` preview and a pipeline stage all reach a pattern through that function, so none of them knows
a sweep is running. And a batched line cannot disagree with an unbatched one about what a pattern
designates, because membership is decided in `Expander::offer` for both. Only the collection policy
differs (`grant_plan::expand::Expander::batch` takes the eight smallest and reports that more
remain, where `Expander::new` refuses the ninth).

The refusals survive batching, deliberately. A matched name that cannot travel in a grant still
stops everything, because a batch missing a name the pattern matched is the silent prefix this whole
mechanism exists to refuse. Batching splits an authority; it never shrinks one.

## BUGS

The limitations of batching, moved here from the main page's `BUGS`, which keeps a summary of them.

- A sweep cannot run anything that takes a directory grant yet, which is the same missing
  delegation chain as the main page's entry on a set at the prompt. `xargs rm *.txt` plans and prints each batch's grant and then
  stops at the first one, because every batch is a set and the progenitor builds no set caretaker. What is proven at a real
  prompt today is the half that costs no authority (`xargs echo`) and the half that previews it
  (`xargs caps rm`). The loop above it is the same loop either way.
- The sweep's stop rule can only see failures the shell prints. A refusal, a spawn failure and a
  filesystem error stop it; a child that ran and did the wrong thing does not, because there is no
  exit status on this path for any program but `least_authority_demo` and `memory_grant_depleter` (`swish::write_outcome`). So
  "batch three did not run" is honest and "every batch before it did what it was asked" is not a
  claim this makes. Exit statuses would fix it and are their own piece of work.
- A sweep re-enumerates the directory once per batch, so it costs batches × the directory rather
  than one listing. That is the price of never holding the whole match, and it is the same trade
  `rm`'s recursive walk already makes.
- A name created mid-sweep above the watermark joins it; one created below does not. The same
  class of caveat as any concurrent directory walk, and it is a consequence of resuming by name
  rather than a bug in doing so.
- Only the first pattern of a batched line is batched, which is the same limitation as the main
  page's entry (only the first pattern on a line is expanded at all) and not a second one.
