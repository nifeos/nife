# The file behind a `>` is this shell

*An appendix to [`notes/pipes.md`](../pipes.md), which is the page to read. This file holds why the
file end of a redirection cannot be a separate process, what that costs, and why it made `>>` one
bit. The lane that split the file moved it here verbatim from the main page on 2026-09-25 (UTC),
under [§212 (a prose budget)](../../design/decisions/0212-a-prose-budget-for-every-document.md). The
directory `notes/pipes/` and this file's stem are provisional names, minted that day by the lane
that split the file. Naming is calef's.*

## The file behind a `>` is this shell, and that was not the plan

The plan was the obvious one. `sink.rs`'s file role is an adapter that holds an FS session and
serves the sink contract, and `sink_tests` proves it against a real RedoxFS image. So the shell asks
the progenitor to build one per redirection and hands the child the endpoint. That does not work,
and the reason is worth more than the feature.

`filesystem_contract` shares one page between the FS server and its clients (`fs_service`'s
`FILE_VA_CLIENT`; the server maps the same frame at `FILE_PAGE`). A client stages bytes or a name in
that page and *then* calls, so its use of the page straddles the call boundary. Two client
processes doing that at once race, and nothing in the contract orders them: there is no lock, and
a rendezvous on the FS endpoint cannot span a put-then-call pair.

That is survivable for `date > out.txt`, where the shell touches no file while the adapter writes.
It is not survivable for `ls > out.txt`, which is exactly a line where the shell must read the
filesystem while the adapter writes the redirection. `ls` reads a page of directory entries, hands
a name to the writer, and comes back for the next round. Interleave an adapter's `put` with that and
the listing and the file are both corrupt, silently.

The note that recorded this hazard first is `fs_service::wait_for_caretaker`, which found the
startup half (a client that already exists writes over the name a caretaker staged). This is the
steady-state half, and it has no ordering fix, because there is no moment when both parties are
done.

So the shell backs both ends itself. It already holds the directory capability; it is the one
process that can write the file without opening a second session.

```text
  date > out.txt      date's slot 0 holds the shell's result endpoint, exactly as an
                      unredirected `date`'s does. The shell drains it into a file
                      instead of onto the terminal.

  wc < out.txt        the shell mints an endpoint, gives `wc` READ on it, opens the
                      file and streams it over the sink contract. Same code path a
                      builtin producer already took.

  ls > out.txt        no process is spawned at all. The shell is the producer and
                      the shell is the sink.
```

This costs the milestone nothing, and that is the test of whether it is the right shape. What a
redirected program holds is unchanged: one endpoint, `WRITE`, no way to ask what is behind it. There
is no new message, no change to `grant_plan::spawnproto`, and no change in the progenitor.
`Sink::File` and `Source::File` still exist in the plan, because the manifest check needs them
(`least_authority_demo 9 > out.txt` is still `NotAByteStream`), and the wiring simply does not need
a capability for them.

It is also the smaller claim, honestly stated. `> report.txt` still grants strictly less than Unix's
fd 1, but the sentence is now "the program holds an endpoint and the shell holds the file", not "an
adapter process holds the file". The adapter shape is still real and still proven; it is the right
answer when the client is not the shell, which is what `sink_tests` measures.

### What it costs, named where you meet it

- The shell is single-threaded, so it is inside the drain for the whole of a redirected command.
  That is what an unredirected command already did.
- Every byte crosses the shell's address space twice. There is no benchmark for this.
- A `>` cannot outlive the line, because the thing writing the file is the prompt.

### And what it pays for: `>>` is one bit, in one process

`>>` is the first thing built on that shape, and it is the test of it. Because the shell backs the
file, append is a decision the shell makes when it opens one, and everything else is untouched:

```text
  >   CREATE the name; if it exists, OPEN and TRUNCATE it        offset starts at 0
  >>  CREATE the name; if it exists, OPEN and FSTAT it           offset starts at the size
```

That is the whole diff on the wiring side. `FileOut` already carried an absolute running offset,
because the FS contract's `WRITE` names a position rather than advancing a cursor, so "append" is an
initial value rather than a mode the filesystem has to hold. There is no `O_APPEND`, and there
is nothing for one to mean: a sink has no seek, so every writer appends already and the only
question a `>` ever answered was what happens to the bytes that were there first.

What the child holds does not move, and `grant_plan` asserts that rather than asserting it in prose:
`append_and_truncate_plan_the_same_endowment` plans `date > f` and `date >> f` and compares the two
endowments whole, with the one differing field made equal. They designate the same `FileGrant`, and
the mode rides beside `Sink::File` rather than inside the grant, because the grant is the
authority and the open mode is not part of it.

`>>` is also the reason the truncate is worth stating out loud. `>` empties the file before the
command runs, which is what makes `ls > out.txt` report one more name than the `ls` before it did. A
`>` that had quietly appended to whatever the last run left behind would have been `>>` wearing
`>`'s spelling.

Two rules fall out and neither needed code. `>>` inherits every rule `>` has, because it is the same
operator: the tail of the pipeline only, one per stage, one name, and a name that is not a pattern.
And there is no here-document: the grammar refuses `<<`, because it reads the second `<` as the
operator it is and then finds no name after it. The message is about the missing name where the
mistake was a missing feature, which is a wording gap and is in [this note's
BUGS](../pipes.md#bugs).
