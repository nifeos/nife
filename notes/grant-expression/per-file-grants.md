# Per-file grants: the verb table, the caretaker, the proof and the cost

An appendix to [notes/grant-expression.md](../grant-expression.md), moved there on 2026-10-11
(UTC) under §212 (a prose budget). The main page keeps the mechanism and its rules; this is
the evidence behind each.

## The verb surface is part of what the capability is (milestone 61)

This is the caretaker that needs a per-verb answer, and the other two do not. They serve the
directory protocol their client speaks, so every verb means what it always meant and the attenuation
lives elsewhere. A file capability is a *different protocol*: nothing to enumerate, nothing to
create a name in, one handle. So "which verbs exist" is not a filter over a wider surface, it is
what the capability is, and it is written down in `filesystem_protocol::verb::file_grant::POLICY`, one row
per opcode, in a crate host tests and Kani can reach.

Three policies, and the third is the one worth keeping distinct:

- `Local`: answered here, from what this process holds. `OPEN` resolves the one granted name; `CLOSE`
  is a truthful no-op, because the caretaker owns the underlying handle for its whole life.
- `Forward`: sent on the caretaker's own handle, with the caller's substituted, gated on the
  direction when the verb mutates.
- `Refused(errno)`: not offered, and the errno says which kind of "no" this is. `ENOTDIR` means
  the request does not *mean* anything here, which is a stronger statement than a refusal. Flattening
  that into "allowed / not allowed" is exactly what a table is at risk of doing, so the rows carry
  the errno rather than a boolean.

Milestone 61 (caretakers) closed the attribute gap: `GETXATTR`, `SETXATTR`, `LISTXATTR` and
`REMOVEXATTR` used to answer `EOPNOTSUPP`, so a program handed one file could read the file and not
what was attached to it. See [xattr.md](../xattr.md).

## Why the caretaker is a process and not a check inside the FS server

The FS server receives on one endpoint. Serving a second, narrower one would need a receive over
a *set* of endpoints, which this kernel does not offer. The way to add it is to give endpoint
capabilities a badge (seL4's answer), and that is a design fork, recorded rather than taken. The
caretaker needs nothing new: it is an ordinary FS client above and an ordinary FS server below.

It is the stronger form of the claim. The confined program holds an endpoint to the caretaker
and nothing that names the FS server, so "it cannot reach a second file" is a property of its
capability table, not of a branch it is trusted to take. The boundary is an address space. That is the same
reason milestone 36's checker lives outside the component it checks.

The grant costs no memory. The name and the direction ride in the caretaker's three `START` argument
words (`filesystem_protocol::grant`, 16 bytes of name). One frame is shared by all three processes.
That is sound because every request on both hops is a blocking `CALL`: the client is parked inside its own
call for the whole time the caretaker touches the page.

## How it is proven, and why one test would not have been enough

An attacker (`fs_test_client`'s third role) reports a bitmap of what got through, not a pass. It is run
twice, on both ISAs:

- Read-only grant of `motd`: every bit must be clear. It tries to open `scratch`, which exists,
  sits one directory entry away, and the caretaker could open on any request it liked. It tries to
  write and truncate the file it *can* read (refusing a write to a file it cannot even name would
  prove nothing). It tries to create. It sprays handle numbers.
- Read/write grant, same shape: the write bits must be SET and everything else clear. Three of
  them since milestone 61: `WROTE`, `TRUNCATED`, and `WROTE_ATTR`, which is the third way to change
  a file and would have been missed by a direction check that only covered the first two.

The read-only run carries the attribute half of milestone 61, in the clear bits: the listing
and the get reached the store (so the caretaker does forward them) and the set did not (so a
read grant does not). Before that milestone all four answered `EOPNOTSUPP` and the first half
would have failed.

The second run is what makes the first mean anything. A caretaker that refused every request would
pass the read-only test, and so would a grant that reached nothing at all; it fails the writable
one. Each accepted write is read straight back, because "the server accepted my write" and "my write
landed" are different claims. This is milestone 36's two-witness shape, and milestone 33's rule that
an attacker must be pointed at a real neighbor rather than a fictional one.

## At the interactive prompt (milestone 50 (pipes and redirection) wired it; milestone 31 (a capability shell) phase 3 finished it)

The shell's refusal, "you hold no such capability: this shell was granted no directory to narrow",
states a fact about its capability table rather than a release date, so it stops printing on its own
once a grant exists, with no edit to the condition that prints it. A refusal that names the milestone
it waits on becomes a lie the moment that milestone lands.

At the prompt the shell holds the image root, because milestone 50 gave the interactive boot a
RedoxFS disk and the progenitor narrows the file service into the shell. `holdings().dir` is true, a
name on the line resolves, and the progenitor builds a `fs_subtree_caretaker` per directory grant,
so `rm` runs. `script/swish-check` types four lines on both ISAs:

- the preview: `caps rm rmtree/rm-solo` names the directory and says what `-r` would have added;
- the removal: `rm -v rmtree/rm-solo` prints the name it was given;
- the check: `ls rmtree | wc` counts two entries where there were three, so the sibling file and
  the whole doomed subtree inside the same capability are untouched;
- the one grant shape that is still a refusal: `rm gate.txt`.


`FileSpec::Required` still has no consumer, and that is unchanged. The per-file caretaker is built
and proven on both ISAs (below), and no shipped program declares a file. The one the milestone block
names is `wc`, and milestone 50 deliberately made it something else: a stream consumer that cannot
speak the filesystem contract at all, on the argument that "a `wc` that could open the file it counts
would be a `wc` that could open any file". That argument is right and it is *later*, so the block's
`wc report.txt` is a proof of designation rather than of the file-capability path.

## What it cost, against what this note predicted before building it

The section this replaces named four obstacles, before any code. Recording how each came out is worth
more than deleting it, because three landed as written and the fourth was worse than predicted.

The wire. As predicted: a request is three words and what follows it is *capabilities*, so a
directory grant travels as its own messages. It is two, not one, and the shape is better than the
"five words of names" this note guessed: each message is a process's three `START` words, the
caretaker's and then the program's, forwarded by the progenitor without being decoded. That is what lets
`grant_plan` carry a filesystem grant while keeping its deliberate non-dependency on `filesystem_protocol`.

The depth. As predicted, and it is the one thing still open. Every caretaker wiring in this tree
descends exactly one name, so `rm rmtree/rm-solo` works and `rm gate.txt` at the top prompt does not.
The root has no name to descend into and the contract has no verb for narrowing a directory you
already hold. Chaining answers depth two and beyond; the root needs a decision, and
`design/roadmap/0031-capability-shell.md` states the two options rather than guessing.

The lifetime. Predicted as "a supervision question rather than a filesystem one", which is what
DECISIONS §92 (caretaker supervised) then decided: the caretaker is built out of the client's own region, so §40 (supervisor's death)'s
ownership cascade ends both. What the decision did not predict, and what cost the most care, is that
the *first* reclaim of such a region is refused by construction. `reap_region_objects` sweeps a
region's endpoints before it looks at its threads, which is exactly what wakes a caretaker parked in
`RECEIVE` so it can be collected, and a thread that can be scheduled is `RefuseAndArm`. So the mechanism
that makes the caretaker collectable is the same mechanism that makes the first attempt fail.
`job_undertaker` used to trap on any refusal; it now yields and retries, and this is the first
ordinary command that meets `reclaim_region`'s documented retry contract, which until now only the
shell's `^C` escalation did.

The capability table. Predicted at seven slots held for life plus two for the file service. That is what it
is: nine at rest, and a directory-granted spawn peaks at fifteen of sixteen (the region, the
narrowed endpoint, the readiness endpoint, and a `build_child` retyping an address space and a TCB).
One slot from the wall, and `crates/system_initializer`'s BUGS is right about what running out looks
like: nothing at all.

And one thing this note did not predict at all, which is the argument for building rather than
reasoning: two shipped programs disagreed with their own declarations and nothing noticed, because
nothing had ever run them for real. `rm` declared the sink contract and never sent its end-of-stream,
so it could not have been piped. `fs_subtree_caretaker` panicked on a refused descent, which was a
watchdog in a test and would have been the whole machine with the progenitor as the waiter.

## `wc report.txt`: the input operand, and what it does and does not prove

Naming a file to a program that declares an input grants it that file's bytes. `wc report.txt` is
`wc < report.txt` with the operator left out, and it runs down the same path: the planner puts the
designated name in the stage's source, the shell opens it and streams it, and the child holds an
endpoint.

This is milestone 47's move applied to the other direction. The `file:` prefix came out on the
finding that the manifest was doing all the work, and it is doing all of it here too. A program that
declares `InputSpec::Required` reads a stream and declares no file and no directory, so a bare name
after its declared grants can only be the thing feeding it. There is nothing for the parser to
classify. A manifest that declared both an input and a file would need positional arity, which is the
same widening `ArgSpec` is waiting on, and no manifest declares both today.

Designation is authorization here, and the negative control is what says so. `wc` with no name is
refused at the prompt, before anything is spawned, because its manifest says it reads a stream; on
Unix the same command is a shell that appears to hang. So the name is what moved something.

What it is not is the per-file capability `FileSpec::Required` describes, and the difference is
worth stating rather than glossing. The child gets bytes: it cannot seek, re-read, stat, or name a
second thing, and it cannot tell a file from a pipe from a builtin typing at it. That is *narrower*
than a file capability, not wider, so nothing is over-granted; what is missing is the claim that the
child holds the file. `caps` prints which it is, in the child's own terms:

```text
$ caps wc gate.txt
  wc would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    output   this shell's result endpoint (it reads the bytes and prints them)
    input    gate.txt  (this shell reads it and streams it in; the program
             holds an endpoint, not a file)
```

`script/swish-check` types `wc gate.txt` and `wc < gate.txt` at the real prompt on both ISAs and
requires the same three numbers from both. That is what makes "the same designation" a claim about
the machine. One line reaches the file through an operator and one through a name, so if they
disagree one of them opened something else.
