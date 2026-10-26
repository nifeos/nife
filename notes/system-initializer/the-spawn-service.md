# The spawn service: requests, grants and the tidy-up

The appendix of [`notes/system-initializer.md`](../system-initializer.md) for the spawn service.
The child's table layouts are in [`the-child-layouts.md`](the-child-layouts.md).

## Who may ask, and for what

A request on an installer endpoint comes from a package manager (milestone 809 (the package client
becomes a program), §270 (a package manager holds an installer endpoint, not the spawn endpoint)).
`serve_installer` serves install, remove and rollback as it would the owner's console's, under the
holder's name. It refuses anything else with nothing read past the first message, so a holder can
neither spawn nor vouch. The answer goes to the holder's own reply endpoint, because the shell is
reading `result_ep` while that program runs.

On the spawn endpoint, an edit to the installed-programs table is asked first, because under that
bit no other bit of word 2 means anything. Only the owner's console sends it (`vouch`).

Everything else is a spawn, in one of two shapes (`grant_plan::spawnproto`):

- A normal job. `Service::spawn_job` builds the child in a region it splits off the job pool, with
  a supervision endpoint in its fault slot.
- A supervised, interruptible job. The shell leads with a job untyped and a shared job frame.
  `Service::spawn_interruptible` builds the child entirely from that untyped, so the shell's region
  owns it and can `DESTROY` it (§24 (interrupting the foreground process)). It maps the job frame
  and endows nothing else. `SPAWN_OK` is the shell's go-ahead, and the job's `ThreadControlBlock` is
  the one message after it (§309 (a supervised job's `ThreadControlBlock` goes to the shell that
  asked for it)).

Only the normal shape is supervised by the progenitor. An interruptible job's region belongs to the
shell, which tears it down on the second `^C` and after a clean finish. A supervision endpoint here
would put a second party in the teardown path of memory that is not the progenitor's. It would race
the shell's `DESTROY`.

## The request, in protocol order

The shell sends a request's parts in a fixed order, and `Service::read_request` takes them in that
order. The progenitor takes every part a request announced, whether or not this request turns out
to need it. Otherwise the endpoint would be left holding words the next command reads as its own.

1. The directory grant's two data messages.
2. The image's frames, then the argv frame, then the name set's frame.
3. The delegated capabilities, in `receive_delegated`'s order: the job untyped and frame, sink,
   source, diagnostics, the screen-narrowed tail's completion endpoint, the `--mem` untyped and the
   machine statistics page.
4. Last, one `SEND` on the run-unvouched endpoint. It proves only that the caller holds `WRITE`
   there, which is all it has to.

[`notes/grant-expression/spawn-protocol.md`](../grant-expression/spawn-protocol.md) has the
exchange from the shell's side. Only a `std` program reads the argv. Any other program has its
frame taken and hears nothing.

Under the image bit word 0 is a length, not a program (§219 (how the shell names an installed program to the spawner) option D). No row of the program table
is consulted: the program is whatever the bytes are.

## Images

Once the request has been read to its end, `Service::carve_image` carves it twice from the image
pool (milestone 685 (a job is finished when its memory is back)). It splits the child's region first
and the staging region second. So the staging region is the pool's top carve when
`Service::spawn_job` destroys it after the build, and its pages go back. A region returns its pages
to its parent only when it is the most recent carve.

An image that hears words is a `std` program and gets a `std` region (milestone 205 (how a foreign
program is told what to do)). The note that says which is inside frames not yet copied. So the argv
bit sizes the region, and `endowed_image` checks that the note agrees. A directory grant rides an
image only for bytes that hear words, and their region also holds the caretaker
(`grant_plan::STD_REGION_PAGES`). The region is as large as the image (milestone 595 (the shell runs a `std` program);
`IMAGE_POOL_PAGES`, `grant_plan::image_region_pages`).

A vouched image is endowed with the manifest its own ELF note carries (milestone 597 (a program
carries its manifest in an ELF note)), or `grant_plan::NO_NOTE_MANIFEST` without one. The digest
that vouched covers the note, so installing the package installed its manifest. An unvouched image
is built only for a caller that presented the run-unvouched capability. It then gets
`grant_plan::UNVOUCHED_MANIFEST` whatever its note asks, because an unvouched note grants nothing
(§219). Any other miss is refused with its own word, and so are bytes whose manifest forbids the
request. An image is endowed only from what `endowed_image` decided, never from a program row, so no
row's authority reaches it.

A named program with a verb table is narrowed by the line's verb (milestone 809, option V2): `jig
list` is endowed with the catalog alone. `endowed_image` refuses installed bytes that carry a verb
table, because endowing them from the manifest alone would hand every line every verb's grants.
`Service::resolve` reads the verb from the shell's page before the region is split, because an
installer holder's region is sized and pooled from its manifest. `copy_args` checks it again against
the child's own copy. A page changed between the two reads refuses the spawn rather than endowing for
one verb and running another.

## `graphical_terminal`

Graphics is launched from the prompt, and a `graphical_terminal` spawn is the launch (milestone 632
(graphics on demand)). The request must carry the program and the graphics bit together. It must
never carry the keyboard bit, which no capability follows. `Service::launch_graphical_terminal` also
refuses a request that delegated any capability, because the progenitor builds a session from its
own copies and hands it none. Anything else is refused rather than half-endowed. A well-formed request
on a boot with no display is answered `SPAWN_NO_DISPLAY`, so the shell can say so.

The display devices are the progenitor's own (milestone 715 (the spawn service holds the display
grants)). The GPU's four capabilities and the keyboard's three stay in the boot endowment's slots.
Each session's drivers get narrowed copies, as `boot_terminal` is lent to the UART arm. The session's
children are born from one region, and the session program is born supervised on `deaths`. So its
one reap by `job_undertaker` ends the drivers with it.

A session that does not come up is a refused spawn and not a trap, except for the line discipline
the crate's BUGS names. The shell says "could not spawn", the prompt continues, and the builder
reclaims the region itself. A boot that cannot come up is a dead machine, but a launch that cannot
is a command that failed.

## What a manifest declares that no command line can

Some authorities are read from the program's own manifest and never from the request: a clock, a
process domain, the inert-configuration page, randomness and the network. So are the machine and
share views, the reboot object, a sync-only file capability and console input. A person does not
designate any of them, so no bit on the wire carries them. The program's declaration decides, which
keeps its dependence on each visible in what it holds. A boot without the service leaves a declaring
child an empty slot, which is what its second stream reports. `grant_plan::image_can_carry` keeps
`reboot` off every image, so only the built-in `reboot` declares it.

## A normal job's region and grants

`Service::spawn_job` splits a normal job's region before it builds anything. The progenitor builds a
directory grant's caretaker from the same region as the program it serves (§92 (a caretaker is
supervised by the client it serves)). That is the mechanism of §40 (a supervisor's death is its
subtree's death): a child's resources come from its supervisor's region. One reclaim ends both, and
the caretaker cannot outlive the grant it carries.

A `std` program's region is larger, because it is also the heap; see
`grant_plan::STD_REGION_PAGES` for why one region rather than two. It covers a caretaker too. An
installer holder is built from the image pool. Its region carries a package's staging run beside its
heap, and the progenitor's copy of the package is carved beside it.

A directory grant takes the job's own window (milestone 599 (a frame per filesystem client
channel)). The window is a page and the file server's endpoint badged with it. The caretaker calls
with the endpoint, and the caretaker and the job both map the page. When the server enforces grants
itself, it binds the badge to the grant's directory before the job holds anything (milestone 606 (a
directory walk costs what it does on Linux), rulings D and T1). Then a directory grant needs no
caretaker, and any failure on the way falls back to one. A name set still gets
`fs_nameset_caretaker` over the bound channel, because the set is what narrows the directory
(`Service::wire_directory`). A grant in the second mounted tree takes a window from that tree's pool
and always gets a caretaker, since that server enforces no subtree.

The window is held until `job_undertaker` reports the job reaped (milestone 685). `label` is the
name on the job's supervision capability, and the reaped message carries it back. A pool whose every
window is held waits for the reaps that are due, then refuses.

A directory grant that produced no capability does not build the program. Told to act on something
and holding nothing is the one outcome this model must never trade away. The same rule refuses a
`reboot` that declared a sync-only capability on a boot with a writable filesystem and got none: it
must not start unable to sync. With no filesystem there is nothing to sync, and the slot is left
empty.

The machine's index store (milestone 801 (packages over the internet), ruling I2) goes to a `std`
job whose verb holds it. The request must come from the owner's console, and the line must name no
directory, since the `std` layout has one directory slot. The store is a window bound to
`activation/indexes` with the verb's rights. Only a server that enforces bindings itself can make
one, because the path is longer than a caretaker's name. Not granted is not refused: `jig` says what
it lacks. `jig list -u` gets `activation/` itself, read only. The image's index sources page goes
with the store alone.

A package manager's installer endpoint is a copy of the spawn endpoint badged with the job's label.
Its reply endpoint is made from the job's region, so the reap frees it. Both are placed only for a
request on the unbadged spawn endpoint, which the owner's console alone holds. They are placed only in
the `std` layout, where `grant_plan` keeps every declaring manifest. The image's catalog is copied
into a page of the child's own, and a catalog larger than a page is refused rather than cut. The
reply endpoint's `WRITE` is kept, filed by label, until the child's reap; that is how an answer finds
it.

Name resolution (milestone 892 (a std program at the prompt holds the network and a resolver)) is a
badged resolver endpoint per zone. It is given to a `std` job that holds the network on this line
and is reaped by `job_undertaker`, whose reap forgets the grants. A program in the table gets the one
zone the table names. An installed image gets each zone its `grant` lines give and its note asks for,
in the table's order. The badges are granted before the child exists, since the resolver deletes a
page attached to a badge it does not know yet.

Console input, for a `std` job that declares it, is the boot terminal badged with
`line_editor::proto::STDIN_BADGE`. The terminal serves that badge the typed line and nothing else. A
screen-narrowed job gets neither a resolver nor console input, because its keystrokes are its
session's. A job that should hold any of these and cannot is refused.

The job's supervision capability is `deaths` badged with `label` (§148 (resolves by asking the
kernel)). The kernel hands the badge back beside the death, and `job_undertaker` forwards it in its
reaped message. A job with no label is not built. The progenitor could not recognize it at its reap,
and its window would be held for the rest of the boot.

A screen-narrowed child is supervised by the shell's endpoint instead (§106 (an unredirected tail
stage's output goes to the screen)). Its memory still comes from the job pool. `REAP` returns it there
whoever supervises, because a reclaimed region returns to its builder (§26 (the fault endpoint)).

## After the build

After a build, the progenitor deletes every capability that was only the means of building it.
`Service::serve` deletes its copies of the delegated capabilities, and `Service::finish_job` the
rest. They go whether or not the build worked, so a failed spawn does not cost the progenitor's
table a slot for the rest of the boot.

Nothing in the system holds a capability to a live job's memory. Since §32 (a supervisor may collect
a corpse without being able to build one) the reap is a method on the supervision endpoint.

A job that never started is never reaped, so `Service::finish_job` frees its window, forgets its
resolver grant and reclaims its region at once. A started job under `job_undertaker` whose answer
goes to the shell's result endpoint is one a later short pool may wait for. The shell reads that
answer to its end before sending another request (`ReapsDue`).

A redirected or screen-narrowed child owes the shell no answer of its own. The shell would then have
nothing to read and no way to learn of a failure, so the progenitor sends one `SPAWN_OK` or failure
word for it. An unredirected child's own message is the shell's single read, and a failure is the
sentinel. A child that was never built cannot end its own second stream. The shell drains that stream
to `OPERATION_EOF` before it reads anything else, so the progenitor sends the end for it.

## BUGS

- [`notes/grant-expression/spawn-protocol.md`](../grant-expression/spawn-protocol.md)'s "The
  exchange, in order" omits the image frames, the argv frame and the name set's frame, which
  `Service::read_request` takes after the directory grant's messages.

*Name: provisional. calef names files.*
