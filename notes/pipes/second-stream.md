# `2>`: built as a declaration

*An appendix to [`notes/pipes.md`](../pipes.md), which is the page to read. This file holds what
DECISIONS §67 built: its five moving parts, the transcripts, the bugs building it found, the default
destination and the ordering rule. It was moved here verbatim from the main page on 2026-09-25
(UTC), under [§212 (a prose
budget)](../../design/decisions/0212-a-prose-budget-for-every-document.md). The directory
`notes/pipes/` and this file's stem are provisional names, minted that day by the lane that split
the file. Naming is calef's.*

The records this file cites by number:

- §67 (a program's second stream is a declaration)
- milestone 22 (trusted init)
- milestone 266 (one progenitor)
- milestone 96 (one init)


## `2>`: built as a declaration (DECISIONS §67)

Decided and built 2026-08-03. A program that has diagnostics declares a second output in its
manifest. The shell plans a second endpoint only for the programs that declare one. `2>` names where
those bytes go. And aimed at a program that declares none it is a refusal at the prompt, in the same
voice as every other refusal here. `date` is the first and so far only declarer, because `date` is
the program the loss was measured on.

The analysis of the fork as it stood open is kept in [its own appendix](second-stream-fork.md),
because the reasoning is the reusable part and because reading it explains why the built thing has
the shape it does. What follows first is what exists.

### The five moving parts, and none of them is a number everybody agrees on

```text
  crates/grant_plan            OutputDeclaration::BytesAndDiagnostics { slot }   the declaration
                               line::Diagnostics { None, Printed, File }  where they go
                               Refusal::NoDiagnosticStream                aimed at a non-declarer

  crates/grant_plan/line.rs    `2>` and `2>>`, at the tail, like `>` and `>>`

  components/src/swish/running.rs    an endpoint the shell mints ONLY for a `2>`, because that is the
                               case where it has to back a file

  components/src/terminal_sink_caretaker.rs    where the bytes go with no operator on the line: the terminal,
                               served by an adapter, so nothing goes through the shell at all

  fixtures/src/hello.rs            receives whichever it is and inserts it at the slot the MANIFEST
  crates/system_initializer    names, not at the next free one. Yes, both: see below
```

The two progenitor lines are the ones to read twice. The slot is high (eight) and placed explicitly,
and that is not a style choice. How many low slots a child gets depends on what the command line
granted it (`date` gets a clock from the progenitor and none from the guest-test harness). So a
diagnostic stream that landed "next" would sit at slot 2 in one wiring and slot 1 in another, and a
program that probes one number would read the wrong slot. `abi::tcb::CAP_INSERT` already had an
explicit target, added for `abi::fault::FAULT_EP_SLOT` for exactly this reason; §67 is its second
user.

So the number eight is not a convention anybody has to know. It is in the manifest, `caps` prints
the stream that uses it, and a program that declares nothing has no slot at all.

### What it looks like, and the case it was built for

At a real prompt, on a machine whose clock works, `date`'s second stream exists and is empty:

```text
$ date 2> err.txt
  Sun 2026-08-03 22:41:07 UTC
$ wc < err.txt
  0 0 0
```

That is the whole claim in four lines. The answer went to the terminal, the second stream went to
`err.txt`, and it carried nothing because there was nothing to complain about. Before §67 there was
one stream, so `err.txt` could not have existed and the timestamp would have been the only thing
either destination could have held.

And the case it was built for, which needs a `date` that was granted no clock (the interactive
prompt has one, so this is `kernel::user::date_tests`, asserted by value rather than read off a
transcript):

```text
  date, no clock, with a declared second stream:
     the diagnostic endpoint     "date: the time is unknown: this process holds no clock capability"
     the output endpoint         OPERATION_EOF, and not one byte before it
```

The second line is the fix. `date > when.txt` drains the output into the file, and the output is
empty, so the file is empty and the complaint is on the screen. Nothing in `date` decides that;
it wrote to a different endpoint, and which endpoint it had was its spawner's choice.

`caps` prints both destinations, because a preview that named only one of them would be a
half-truth about a line that has two:

```text
$ caps date 2> err.txt
  date would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 1  frame     clock    read-only. it can read the time and not set it,
                              and no token on the line could have asked for more
    output   this shell's result endpoint (it reads the bytes and prints them)
    diags    err.txt  (this shell writes them there; the program holds a
             second endpoint and still cannot seek, truncate or stat)
             this shell empties it first, before the command runs
    arg    (none)
  reading the command is reading its whole authority.
```

And with no operator, where the `diags` row names a capability this shell does not hold:

```text
$ caps date
  date would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 1  frame     clock    read-only. it can read the time and not set it,
                              and no token on the line could have asked for more
    output   this shell's result endpoint (it reads the bytes and prints them)
    diags    the terminal's own sink, a component this shell does not hold.
             declared by the program, so a > cannot swallow them, 2> can
             name them, and they reach the screen without passing here
    arg    (none)
  reading the command is reading its whole authority.
```

The refusal, which is the other half of "a declaration, not a number":

```text
$ wc gate.txt 2> err.txt
  wc: declares no second output, so there is nothing for 2> to name (its diagnostics ride its output)
```

Nothing was denied. `wc` writes one stream, its diagnostics ride it, and the operator has nothing to
bind to. That is `Refusal::NoSuchProgram`'s shape applied to a stream, and it is the sentence §67
asked for.

### A third bug of the same shape, and the pattern is now three deep

Building the terminal's sink adapter made the boot print nothing at all on aarch64, and it was the
progenitor's sixteen-slot capability table for the third time. The adapter was built between the
input driver and the shell, which looked like the natural place. At that moment the progenitor still
holds the terminal endpoint, two shared frames, the file service and its page, so one more endpoint
put `build_child` one slot short while it was retyping the *shell's* address space. The shell was
never built, so nothing ever printed.

The fix is where rather than what: build it after the shell, once every boot capability the
adapter does not need has gone back.

This was first written down as "build it last, which is the narrowest the capability table ever is",
and merging milestone 22's interactive boot proved the "last" half wrong. That lane added a
`job_undertaker` built after the adapter and a construction-budget giveaway between them, so the
adapter is now the fifth of six boot components, and the boot is fine. The constraint was never the
ordinal; it was that the adapter must not be holding a slot while `build_child` retypes the shell's
address space.

What the merge did pin down is the other end. The adapter has to be built before the progenitor
gives the construction budget away, because it is a system component and the root untyped is what
the system is built from. Afterwards the only budget left is the progenitor's own scratch pool,
sized for page tables, and spending a whole program out of it would surface much later as some child
failing to map a scratch page. So `term_ep` no longer goes back "immediately after" the adapter: it
stays until the progenitor has printed its dropped-authority sentence through it, which is the last
thing either the progenitor does with the terminal.

The pattern worth keeping is that all three instances of this presented identically, as a boot that
reaches userspace and then says nothing, and all three were a capability slot rather than memory. A
capability table that is sized in a constant and consumed in an order is a resource with no error
message. `build_child` returns `Err(())` and the progenitor halts, which is correct and silent.

### A correction: there are two inits, and this note said there was one

*(This section records milestone 96's state. Milestone 266 made it one program on all three
architectures, `components/src/progenitor.rs`, so "two inits" and the archive entry `init`
below are both history now. The section is left as it was written, because what it records is
the correction and not the arrangement.)*

*Written 2026-08-03; the duplication it describes was removed by milestone 96 the next day. The
finding is kept because it is the reason the crate exists.*

[The section on
`script/swish-check`](tests-and-the-boot-gate.md#the-gate-for-the-boot-itself-which-is-what-runs-the-real-progenitor)
says it "is the only thing in the tree that runs the real `system_initializer`". That was wrong
about which program it runs on aarch64. Building `2>` found it the hard way: the shell delegated a
diagnostic endpoint, nobody received it, and the prompt hung on the first `date` with no fault and
no message.

`kernel/src/main.rs` hands off to `user::initial_ramdisk()`, which loads the program named `init`, and on
aarch64 that is `fixtures/src/hello.rs`'s `init_boot` role. `components/src/progenitor.rs` is
riscv64's. Both serve `grant_plan::spawnproto`, and the serving loop was written twice, once in each
file, about a hundred and forty near-identical lines: the same delegation order, the same slot
ordering, the same clock rule, the same `build_child_at`.

That duplication was a rule-7 problem wearing a different hat. What two binaries must agree on is a
crate, and these two agreed on far more than a constant; a change to the protocol that landed in one
of them was a boot that hung on the ISA nobody tested.

Milestone 96 made it one. `crates/system_initializer` holds the construction and the spawn service,
and each init is now the table of slot numbers its own kernel granted plus a call into it: the two
grant orders differ, and nothing else does. The loader went the same way, from three copies
(`supervision_protocol` plus a `build_child` in each init) to `supervision_protocol`'s, which the
inits reach through the crate. What the two files still say for themselves is the thing that is
genuinely theirs, which is what the kernel put in which slot.

What the gate proves is unchanged and is the reason it caught this: `script/swish-check` boots both
ISAs, so it runs both inits, which is exactly what a single-ISA gate would have missed. It is still
the gate, because a shared crate removes the *drift* and not the risk. init's sixteen-slot
capability table and the shell's bounded stack are both still sized in a constant and consumed in an
order, and both still fail by printing nothing at all.

### Where the bytes go by default, and why it is not this shell

With no `2>` on the line, a declared second stream goes to the terminal's own sink, which is a
component (`components/src/terminal_sink_caretaker.rs`, notes/sink-protocol.md) and not the shell.
The progenitor endows it from the manifest, exactly as it endows the clock and for the same reason:
the shell holds no terminal capability it could delegate, and a person does not designate a screen.

That is stronger than the shell printing those bytes, and the difference is worth stating. They
reach the screen without passing through the shell at all, so nothing the shell does to a program's
output can reach them: `date > when.txt` drains the output into the file and never sees the
complaint. `caps` prints it as a row that names a capability this shell does not hold.

It is also what keeps the shell single-threaded and honest. The shell reads one endpoint at a time
(there is no receive-on-a-set in this kernel, the gap `fs_file_caretaker` records too). So a default
that came back here would put it in the business of multiplexing two streams. The ordering
constraint below would then apply to every declaring program rather than to the ones a `2>` names.

### The one rule a `2>` costs, and why only a `2>` costs it

A program whose second stream this shell is backing must say everything it has to say, and close
that stream, before it writes a byte of output.

`2> err.txt` is the case where the shell holds both ends: it mints an endpoint, hands it to every
declaring stage, and drains it into the file. Two streams, one single-threaded reader, and it must
read the diagnostics first. Consider `date | wc 2> err.txt`: the shell is not the reader of `date`'s
output, `wc` is. A shell that drained the output first would be waiting on `wc`, which is waiting on
`date`, which is blocked in a rendezvous `SEND` on a diagnostic endpoint nobody is listening to.
Diagnostics first is the only order in which nobody waits on somebody who is waiting on them.

`date` keeps the rule for free, because every complaint it has is a reason it has no answer, and it
keeps it unconditionally: it closes its second stream before its first either way. A program with
running commentary (a `rm -rv` that hit one unremovable name halfway through a thousand) could not,
and would be fine at the default destination and wrong under a `2>`. That asymmetry is the honest
cost and it is in [this note's BUGS](../pipes.md#bugs).

### What it deliberately does not do

- There is no `Diagnostics::Pipe`. A diagnostic that flowed down a `|` into a `wc` would be
  counted as output, which is the conflation the second stream exists to end. `2>` names a file or
  nothing.
- One diagnostic stream per line, not per stage. The shell mints one endpoint and hands it to
  every declaring stage, so `2>` at the tail names where *the line's* diagnostics go. Two declaring
  stages would interleave on it, which is exactly what Unix's shared fd 2 does; the difference is
  that here they interleave because the shell chose one destination and not because a number was
  ambient.
- No `2>&1`. Merging the streams back is not expressible and should not be: the point of the
  declaration is that the two are different capabilities. A program that wants one stream declares
  one.
