# The manifest declarations, the input slot and the wire

*An appendix to [`notes/pipes.md`](../pipes.md), which is the page to read. This file holds
`OutputDeclaration` and `InputDeclaration`, how the input slot's shape was decided, the spawn request's bits and
delegation order, and why a builtin can lead a pipeline. It was moved here verbatim from the main
page on 2026-09-25 (UTC), under [§212 (a prose
budget)](../../design/decisions/0212-a-prose-budget-for-every-document.md). The directory
`notes/pipes/` and this file's stem are provisional names, minted that day by the lane that split
the file. Naming is calef's.*

The records this file cites by number:

- §67 (a program's second stream is a declaration)
- milestone 292 (`fixtures/src/sink.rs` is three programs wearing one name)
- milestone 50 (pipes and redirection)


## The two manifest declarations, which were not in the plan

### `OutputDeclaration`: not every program has bytes to redirect

This system has two conventions for "a program said something", and only one of them can be
redirected:

| | what slot 0 carries | can it be `>` or `\|`? |
|---|---|---|
| `least_authority_demo`, `memory_grant_depleter` | a `u64` answer in a register | no |
| `interrupt_heeder`, `interrupt_ignorer` | nothing; they report through a shared frame | no |
| `date`, `wc`, `rm` | the sink contract's byte messages | yes |

`date` went one further on 2026-08-03 and declares a second byte stream as well (DECISIONS §67),
which is what `2>` binds to. `OutputDeclaration` is where that is written down too.

`least_authority_demo 9 > out.txt` would otherwise put a raw word into a file sink, producing a file
with nothing legible in it and no error anywhere. Declaring the convention makes it
`Refusal::NotAByteStream` at the prompt. Unix has no equivalent because there every program's stdout
is bytes by construction; here the register fastpath is real and older than the sink contract, and
the manifest is where the two are told apart.

### `InputDeclaration`: the refusal Unix cannot produce

`wc` reads a stream. A `wc` with nothing feeding it blocks on a receive forever, and on Unix
that is a shell that appears to hang, because there fd 0 always exists and "nobody is ever going to
write to it" is not a property of the command line. Here it is: `wc` declares
`InputDeclaration::Required`, so a line that gives it neither a `<` nor a pipe is refused before anything
is spawned.

The mirror holds too. `date < report.txt` is `Refusal::InputForbidden`, on the same grounds an
unplaceable token is refused: authority that moved for no reason.

## The input slot's shape, which this lane had to decide

The protocol lane left it open ("both `< file` and a pipe's read end need an input-slot convention
that does not exist"). The decision is the smallest one available:

> A source is the sink contract received rather than sent. An endpoint the program holds with
> `READ`, on which `OPERATION_BYTES` messages arrive until `OPERATION_EOF`.

No new protocol, no new opcodes, no reply. Three consequences fall out and all three are wanted:

1. `<` and the right-hand side of `|` are the same convention, exactly as `>` and the left-hand
   side of `|` are. A program that can be piped into can be redirected into, with no second code
   path.
2. A source's producer is an ordinary writer. A file behind a `<` is a process that opens the file
   and writes the sink contract at it, which is what `fixtures/src/file_source.rs` already was
   (`fixtures/src/sink.rs`'s verify role, until milestone 292 gave it its own name). The shell
   itself is a producer when a builtin leads a pipeline.
3. `OPERATION_EOF` becomes load-bearing rather than tidy. A reader has to be told the producer is
   finished; inferring it from a death notification would be a fact about process supervision
   standing in for a fact about a stream. This is why `date` gained an end-of-stream message.

The asymmetry with `OutputDeclaration` is real and worth naming: output has three shapes because the system
grew two of them before the sink contract existed, and input has one because nothing read a
stream at all until this milestone. There was never a chance for a second convention to establish
itself.

## The wire, and the one thing the progenitor had to learn

`grant_plan::spawnproto` grew two bits in the request word and two positions in the delegation
order:

```text
  w2 = mem_pages | INTERRUPTIBLE_BIT | SINK_BIT | SOURCE_BIT | DIAG_BIT

  then, over SEND_CAP, in this order:
    job untyped, job frame   (if INTERRUPTIBLE)
    the sink                 (if SINK)          -> the child's slot 0
    the source               (if SOURCE)        -> the child's slot 1
    the diagnostic endpoint  (if DIAG)          -> the slot the MANIFEST names (§67)
    the --mem untyped        (if mem_pages > 0)
```

`DIAG_BIT` is the odd one and the difference is the decision: the other three say *which slot*, and
this one does not, because the slot is the program's declaration rather than the shell's choice.
What the wire says is only "expect one more capability". It is also the only one the shell sets from
an operator rather than from the wiring. With no `2>` on the line a declaring child still gets a
second stream, endowed by the progenitor from the manifest the way the clock is.

Order rather than tags, because both sides read the same word: a `SEND_CAP` nobody expects and a
`RECEIVE_CAP` nobody answers each deadlock both parties.

The rights are narrowed per direction and that is load-bearing. A pipe's write end travels as
`WRITE|GRANT` and its read end as `READ|GRANT`, and the progenitor inserts them as `WRITE` and
`READ`. So the program on the right of a `|` cannot write back up its own input. Nothing in either
program enforces that; the capability it holds simply cannot express it.

One ack was added. A child whose output was substituted owes the shell no answer, because its
answer is going somewhere else, so a failed spawn would be invisible and the pipeline would wait on
a producer that does not exist. `SPAWN_OK` on the result endpoint closes that. An unredirected spawn
is unchanged.

## A builtin can lead a pipeline, because the shell can be a writer

`echo hello world | wc` runs with one spawned process in it: the shell mints the endpoint, spawns
`wc` with the read end, and then writes `echo`'s bytes into the write end itself.

That costs no new mechanism, and it is the register-only sink contract paying out: being a writer
needs one capability and nothing else, so anybody can be one. The builtins already rendered their
output through a callback rather than through `print`, because the witness roles have no terminal.
That was done for testability and it means `echo`, `ls` and `pwd` feed a pipe with no branch in any
of them.

`ls | wc` therefore works, in a shell that holds a directory, and since milestone 50's second half
the interactive boot grants one.
