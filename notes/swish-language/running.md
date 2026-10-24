# How the shell runs a program

*Name: provisional, after `components/src/swish/running.rs`. calef names files.*

[`components/src/swish/running.rs`](../../components/src/swish/running.rs) turns a planned line into
messages on the spawn endpoint. This page carries the design its doc comments link to. The wire
itself is `spawnproto`'s, in `crates/grant_plan`.

## The words grant

A program whose manifest says it hears words gets its directory grant through `words_grant`, not
through `rm`'s `dir_grant`. The caretaker descends into the shell's current directory with the rights
`WordGrant::rights` asks for. The progenitor clamps those rights to the manifest it endows, so an
unvouched program gets read-only access whatever the shell asks. A set of names becomes a
`fs_nameset_caretaker`, with the set encoded on its own page. The whole directory (`.`) becomes a
`fs_subtree_caretaker` with no page. The confined program's own `START` words are zero, because a
`std` program reads none.

The set's filter can be raced on a real boot. The caretaker forwards a checked name through the file
service's one shared frame. This shell writes that frame too while it drains a redirected job, such
as `std_grep needle docs > out.txt`. A write that lands between the caretaker's re-staging and the
server's read opens a name the set never approved. `notes/a-set-grant-at-the-prompt.md` has the case,
and `notes/shared-page-audit.md` finding 1 has the mechanism. The production pool of milestone 599 (a
frame per filesystem client channel) closes it.

## Running bytes

A path at the prompt runs bytes, not a name (§219 (how the shell names an installed program to the
spawner), option D). The shell reads the file through the file service it already holds, into frames split from its own budget. It sends them on the spawn
endpoint under `IMAGE_BIT`, one frame at a time: retyped, filled, delegated narrowed to `READ`, then
deleted. The progenitor hashes its own copy and looks the digest up in the installed-programs table.
A hit runs with the manifest the bytes carry.

A miss runs only if the session holds the run-unvouched capability, §219's gate D2. The request then
claims it, and the shell presents it as the last message. That presentation goes on every image
request the session makes, vouched or not, because the shell cannot know the verdict. The child of a
miss gets what the line granted, plus the clock and configuration pages. Without the capability the
miss is refused with its own word, `swish::UNVOUCHED_SENTENCE`.

The line is bound against the manifest the file carries, or against `grant_plan::NO_NOTE_MANIFEST`
when it carries none. So an argument the note does not declare is refused at the prompt, exactly as
`uptime 3` is. A note that cannot be read, or that asks for what an image cannot be given, is refused
with nothing sent. Whether the note is honored is still the progenitor's call, on its own copy
(`grant_plan::image_manifest`).

## The image window's page tables

The shell writes argvs, name sets and an image's frames through one window. Its page tables come from
a region split off the budget once per shell and never given back. The reason is how a region returns
memory. A staging region's pages go back to the budget on `DESTROY` only if it is the budget's most
recent carve. A page table allocated from the budget while staging would sit above it and strand
every staging page for the life of the shell. So every mapping in the window takes its tables from
the primed region, and none comes from the budget after the first image.

## The manifest note

`read_manifest_note` reads the note the way the progenitor reads its copy. A file whose first page is
not an ELF header with its program headers inside it reads as carrying no note. That is not this
reader's question: the progenitor's `Elf::parse` refuses what is not a program, and says so. The one
real program that could land there is one whose header table starts past its first page. No linker
script in this tree produces one. Its line is then bound against the no-note default, and if the
progenitor finds a note the request does not fit, it answers `SPAWN_REFUSED_BY_MANIFEST`. A note
segment larger than one page is refused for the same fail-closed reason.

## The directory grant

`dir_grant` builds the two `START` triples for a program like `rm`: the caretaker's, then the
program's. The caretaker is told the directory and the `filesystem_contract::dir` rights to ask for.
The program is told the operand and the options that were typed. Nothing between the shell and the
process decodes either triple, so `spawnproto` carries them without linking the filesystem contract.
The order is the wire's. Backwards, it would start `rm` with a directory's name and a caretaker with
a file's.

The rights come from what was typed, which is why `-r` is an option and not a mode. Without it the
capability is `REMOVE` alone: the program may take a name out of this one directory, and cannot
enumerate it, read it or walk into it. With it the capability is `REMOVE_TREE`, which adds exactly the
walk. `caps rm -r logs` prints the difference before anything happens.

The shell holds no `GRANT` on its file-service rendezvous, so it cannot hand a caretaker anything.
It says what the grant is, and the progenitor, which holds the service, builds the caretaker. Five
shapes cannot be delivered, and each is refused with nothing spawned rather than half-granted.

- A directory in a two-grant shell's second tree. The progenitor descends from the root of the first
  tree, so the same words would name a directory of the same name there.
- The root of the shell's namespace. A caretaker's whole attenuation is one `OPENDIR` into the
  granted directory, and the root has no name to descend into. The contract has no verb for "the
  directory I already hold, with fewer rights". So `rm gate.txt` at the top prompt cannot be narrowed,
  and the unnarrowed service would be the whole disk. The answer is a design fork recorded in the
  roadmap: a `NARROW` verb on the contract, or a shell rooted one level down.
- A directory more than one level down. That is a chain of caretakers (§92 (a caretaker is
  supervised by the client it serves)), and the progenitor builds one.
- A set of more than one name. That grant is a `fs_nameset_caretaker`, and the progenitor builds the
  subtree one for this path. `rm *.txt` is planned, previewed by `caps`, and refused at delivery.
- A directory or file name that does not fit in a grant's two argument words
  (`filesystem_contract::grant::fits`), because each name travels packed into two words.

## The plain request

A plain line sets no `diagnostics` bit (§67 (a program's second stream is a declaration)). It has no
`2>`, so there is nothing for the shell to back. A program that declares a second stream gets the
terminal's own sink, endowed from the manifest the way the clock is. The `screen` bit is never set
either (§106 (an unredirected tail stage's output goes to the screen)). A program reaches this path
only when `plan` put nothing in its source slot. Every program that declares `writes_while_reading`
also declares `InputDeclaration::Required`, which sends it down the `Source::File` arm instead.

The `graphics` bit is the whole request for a `graphical_terminal` launch, because the spawn service
holds the display devices and this shell holds none. A plain line is the only line that manifest can
reach this path on: `Words` output refuses `>` and a pipe's left side, and
`InputDeclaration::Forbidden` refuses its right.

## The screen-narrowed tail

The kernel's exit delivery is the completion signal for a tail stage narrowed to the screen.
`receive_fault` blocks until the child is dead and unreaped, which is stronger than "it painted its
last line". A dead thread cannot enqueue another `SEND`, so the child cannot still be mid-write.
What can still be in flight is `terminal_sink_caretaker`'s trailing `CALL` to `line_editor`. That is
the caretaker-hop display race §106 (an unredirected tail stage's output goes to the screen)
accepts: the next `$ ` can interleave with that call's output. It is a display glitch, not a
correctness or confinement failure. Milestone 151 (notification objects) built the primitive the
fix needs, and milestone 40 (documentation as a system service) owns the fix: the shell binds a
notification and waits on the caretaker's drain.

A corpse can be refused once if its region still holds something schedulable, so the reap retries
with a yield between attempts. A reap that never succeeds leaks one job's worth of the progenitor's
pool, and the next "out of memory" spawn failure is the visible symptom.

## The second stream

The diagnostic drain runs before the output drain, and the order is forced. In `date | wc` the shell
is not the reader of `date`'s output; `wc` is. A shell that drained the output first would wait on
`wc`, which waits on `date`, which is blocked sending on the diagnostic rendezvous. Diagnostics first
is the only order in which nobody waits on somebody waiting on them. The cost is a rule the declaring
program keeps: say everything before producing anything. `date` keeps it, because its complaints are
all reasons it has no answer.

The diagnostic rendezvous is retyped straight from the shell's budget and kept for the session. A
pipeline's region is destroyed to turn a dead reader into `Gone`, and a stream this shell always
drains to `OPERATION_EOF` has no dead reader to signal.

## The output ceiling

`MAX_OUTPUT_CHUNKS` is one number for every destination: 4096 chunks of 16 bytes, 64 KiB. It bounds a
bug, a program that never announces end of stream, so that the bug costs a truncated answer and not a
prompt that never returns. A bug is not more tolerable when the bytes go into a file, so a `>` gets
the same ceiling as the screen. The number only has to be larger than anything a working program says
and smaller than forever. A rendered manual page is tens of kilobytes, and hitting the ceiling costs a
few seconds of serial output.
