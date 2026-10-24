# How the shell runs a line with operators

[`notes/pipes.md`](../pipes.md) is the page to read. This appendix holds the reasoning behind
`components/src/swish/pipeline.rs` and the slot and delegation rules in
`components/src/swish/wiring.rs`, which their doc comments link here.

## Plan everything, then move anything

`pipeline` designates the redirection targets, plans every stage, and only then moves a capability.
A pipeline that half-starts leaves a process blocked on a channel nobody will write to, and the
shell has no way to reach it. So nothing spawns until the whole line is known to be runnable.

The plans live on the heap, sized to the line's own stage count rather than to `MAX_STAGES`. An
array of endowments, each carrying a whole name set, is what the four stack overflows in
[the-boot](the-boot.md) had in common, and this function's frame was the deepest in the program.

## A builtin at the head

`echo`, `ls` and `pwd` render text and grant nothing, so the shell can be their sink's client. A
stage of that shape is not a program: the shell writes its bytes into the pipe itself. That costs
no process and no new mechanism, and it is what makes `ls | wc` possible in a shell that holds a
directory.

A builtin may stand only at the head, and only when nothing feeds it. A builtin with an input would
be a builtin that reads, and none of them do. A builtin has no manifest, so it declares no second
stream, and a `2>` on it is refused with the sentence a program that declares none gets.

The builtins write through a callback rather than through `print`, because the witness roles have no
terminal. The same `echo` and `ls` therefore feed a pipe and a file with no branch in either. A
builtin whose output is a file spawns nothing at all: `ls > out.txt` is one process, the shell
reading the directory and writing the file.

## The head's input comes off the plan

An input operand is a trailing positional at a program that declares an input
(`grant_plan::plan_against_with`). The same rule makes `wc report.txt` mean `wc < report.txt`. Only
the planner knows it, because it needs the manifest. Reading the input off `Line` instead would be
the shell classifying tokens the planner has already classified. So `run_pipeline` takes the head's
input file from `plans[0]`. A head with an empty input slot answers a `receive` with `NoSuchSlot`
rather than blocking, so missing it counts an empty stream instead of hanging.

## The chain check, before any file opens

The shell is the producer when the bytes going into the head come from it: a `<`, an input operand,
or a producing builtin. It is always the consumer, because the tail's sink is its result endpoint or
a file it backs. With one wait point and no select, that works only if some stage reads to the end
before it writes ([one-wait-point](one-wait-point.md)). `grant_plan::check_chain` decides this.

It runs before the files are opened, because `>` truncates, and a line that will not run must not
have emptied a file. The files then open before any process exists. `date > f` empties `f` even if
`date` then says nothing.

## The screen narrowing, decided before anything spawns

§106 (the `terminal_sink_caretaker` narrowing) narrows an unredirected tail whose program both writes
and reads. `terminal_sink_caretaker` takes that tail's primary output instead of the shell's result
endpoint. `check_chain` already used the identical condition, the tail's `Sink` and its
`writes_while_reading`, to let a chain with no absorbing barrier through. `run_pipeline` must agree
with it exactly.

`check_chain` is pure, host-testable logic and cannot retype anything, so it cannot confirm the
screen endpoint mints. `run_pipeline` is the one place that can. A line allowed on the assumption of
narrowing must not fall back to an unnarrowed spawn when the mint fails. The shell would already be
mid-feed, the child's output would route back to the result endpoint, and nobody would read it. That
is the deadlock `check_chain` exists to refuse, so a failed mint is a loud refusal.

A screen-narrowed tail's bytes never reach the shell. They go to `terminal_sink_caretaker`, and the
shell waits for the child's exit on the screen endpoint instead.

## Diagnostics

The count of stages handed the diagnostic endpoint comes from the manifests, not the line. Only
stages whose second stream this shell backs count, the ones a `2>` named a file for. A
`Diagnostics::Printed` stage writes to the terminal's sink adapter, and the shell never sees those
bytes. The diagnostics drain first, because in `date | wc` the shell does not read `date`'s output.
Draining the output first would wait on `wc`, which waits on `date`, which is blocked sending on the
diagnostic endpoint.

## Building one stage

The two delegations, sink and source, are the whole difference between a piped stage and an ordinary
spawn. The progenitor receives an endpoint and puts it where the result endpoint would have gone,
which is why `>` and `|` are one mechanism. The rules for the rest of `spawnproto::Wiring`:

- A stage is directory-granted only for words (milestone 205 (how a foreign program is told what to
  do)). `rm` is the one program with `DirectoryGrantDeclaration::Required`. It writes a byte stream
  and takes no input, so `rm x | wc` puts it at the head. Delivering its grant here would duplicate
  `dir_grant`'s refusals, so it is met only in `spawn` ([dir-capability](../dir-capability.md)).
- A stage is always a program the image names. §219 (how the shell names an installed program to
  the spawner) option D runs an image only on a plain line.
- A stage never promises devices. `graphical_terminal`'s manifest refuses every operator that builds
  a stage: `Words` output refuses `>` and a pipe's left, and `InputDeclaration::Forbidden` its right.
  So `graphics` and `keyboard` are false by construction.
- The screen endpoint goes over as `READ | GRANT`. The progenitor installs it as the child's fault
  target, and the shell needs `RECEIVE` and `REAP` on its own copy, as `job_undertaker`'s deaths
  endpoint does. The progenitor narrows its copy no further than `READ` when it inserts the child's
  (`supervision_protocol::build_child_space`). Less than `READ` would leave it nothing to hand on.

A stage whose output was substituted owes the shell no answer, so the progenitor acks instead.
Without the ack a failed spawn is invisible, and the pipeline waits on a producer that does not
exist. A screen-narrowed stage has the same shape, since its completion signal is the fault endpoint.
A fault word can arrive in the ack's place (milestone 235 (a command that faults hangs the prompt)).
It always belongs to an earlier stage of the same line that died between its own ack and this one.

## The file end

`FILE_CHUNK` is 256 bytes on purpose. The shell runs on the four stack pages the progenitor maps it,
and the shared page is sixteen times that. A larger buffer buys fewer filesystem round trips and
costs the one resource the program is short of. `FileOut` still buffers to the chunk, because the
filesystem contract's unit is a page and the sink contract's is sixteen bytes.

A refused write keeps its errno and is reported once, after the stream ends. A producer mid-listing
has nothing to do with the failure. A short write is its own field rather than an errno: the request
did not fail, it was answered with a smaller count, and calling it `EIO` would invent a fact.

`>>` uses `FSTAT` rather than a seek flag. `FileOut` carries its own running offset and every write
names an absolute position, so append is an initial value and not a mode the filesystem holds
([the-file-end](the-file-end.md)). A grant carries a position rather than a token, resolved once at
plan time. Opening walks from the root of the grant's tree (milestone 154 (a process that holds two
directory capabilities)), so a `cd` between planning and running cannot change what a redirection
means.

## The shell's slots

A shell is told its clock slot in `x2` rather than probing for it, because nothing in this system
reports what a process holds. Probing the wrong slot would find some other object and map a page that
is not a clock.

The run-unvouched capability (§219 gate D2) is probed once, at `_start`. At that point the shell has
allocated nothing, so the slot holds exactly what its builder placed there. Probed later, a frame or
a region this shell retyped could have landed in it. It is probed rather than told because `x0` to
`x2` are spoken for (role, directory rights, clock slot). A named slot is how this tree already places
an authority a program looks for (`grant_plan::NETWORK_SLOT`). Holding it changes one thing: an image
request claims it and sends one message on it, so a digest miss runs with only what the line granted
instead of being refused. It carries no `GRANT`, so the shell cannot hand it on.

A per-file grant is a directory capability narrowed to one name (milestone 31 (a capability shell:
designation is authorization), `components/src/fs_file_caretaker.rs`). A redirection is a file opened
through one. Both need a directory to narrow, so a shell granted none gets the refusal, and it is a
true statement rather than a placeholder. The same binary runs in both wirings with no branch on which.
