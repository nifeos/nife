# `2>`: the fork as it stood open

*An appendix to [`notes/pipes.md`](../pipes.md), which is the page to read. This file holds the
analysis written while `2>` was undecided, which explains what the built thing is not. It was moved
here verbatim from the main page on 2026-09-25 (UTC), under [§212 (a prose
budget)](../../design/decisions/0212-a-prose-budget-for-every-document.md). The directory
`notes/pipes/` and this file's stem are provisional names, minted that day by the lane that split
the file. Naming is calef's.*

The records this file cites by number:

- §51 (the sink protocol)
- milestone 50 (pipes and redirection)


## The fork as it stood open, kept for the reasoning

*Everything from here to the end of this section is the analysis that was written while `2>` was
still undecided. It is kept because the reasoning is the reusable part and because it explains what
the built thing is not. "Today" means before 2026-08-03. The section that closes it is "Why this is
calef's call", and he called it: option (c), the manifest declaration.*

### There is no second stream today, and that is a fact rather than an omission

A nife program holds one output endpoint, in slot 0, placed there by its spawner. Its
diagnostics travel on it, in-band with everything else:

```text
  date: the time is unknown: this process holds no clock capability
```

is `date` writing sink messages on the same endpoint it would have written a timestamp to
(`components/src/date.rs`'s `line`). `rm`'s header says the same thing in its own words: "slot 1: a
report endpoint, `WRITE`. Diagnostics and `-v` lines as framed text". One channel, two kinds of
thing on it, and no way to tell them apart at the far end.

So `date > when.txt` on a machine with no clock writes the complaint into the file, which is
exactly the loss `2>` exists to prevent on Unix.

### But the half that hurts most on Unix is already separated here

The thing a person usually reaches for `2>` to save is the shell's own refusals, and those never
enter a redirection here. `wc < nosuch.txt` is `Say::Failed` printed by the prompt;
`least_authority_demo 9 > out.txt` is `Refusal::NotAByteStream` printed by the prompt; a spawn that
fails is `spawnproto::SPAWN_FAILED` printed by the prompt. All of it goes to the terminal, always,
because the shell is a different process from the thing being redirected and its output was never in
the substituted slot.

That is not a small residue. It is most of what fd 2 carries in a Unix shell session, and it is
separated here by process boundary rather than by convention. That is the stronger separation:
there is no `2>&1` that could merge them back by accident.

What remains is a program's own diagnostics, which today are indistinguishable from its output.

### Why fd 2 exists on Unix, and why that reason does not transfer

Unix needs a *numbered convention* because a process cannot ask its parent for a channel. Every
process gets three descriptors by inheritance, so what fd 2 is has to be agreed in advance by
everybody, forever. Nothing here is ambient. A program holds an endpoint because the progenitor put
one in a slot. The progenitor put it there because the shell's plan said to, and the plan came from
a manifest that already declares what kind of output the program has (`OutputDeclaration`). The mechanism
for "this program has a second thing to say" is therefore a declaration, not a number.

### The two shapes it could take, and what each costs

A second endpoint in a second slot. The direct translation. It costs a `spawnproto` bit (there are
29 free in that word), a delegation position, a progenitor branch, a slot in every child, a manifest
declaration, and an edit to every program that has anything to say. It also doubles §51's claim: a
writer holding two endpoints must be able to tell them apart, which it does by slot number rather
than by asking, so indifference survives *technically*. What does not survive is the sentence "a
program's output is an endpoint", which becomes "a program's outputs are endpoints, and which is
which is a convention" and is Unix's fd numbering with a capability underneath.

The concrete blocker is smaller and more annoying: slot 1 already means two things. It is the input
source or the `--mem` untyped, whichever the request carried, and that is unambiguous only because
no manifest declares both ([this note's BUGS](../pipes.md#bugs) has carried the entry since the
milestone landed). A third stream makes an ordered slot convention untenable and forces a numbered
one first.

An opcode on the one endpoint. `byte_sink_contract` puts the operation in the top byte of the
request word, so `OPERATION_BYTES = 0` and `OPERATION_EOF = 1` leave 254 spellings free. A third, "these bytes are
a diagnostic", would carry the distinction on the wire the writer already holds: no second
capability, no second slot, no spawnproto change, no progenitor change, and §51 intact word for
word. The reader then decides, so `2> name` would name where the shell sends the diag messages it is
already receiving, and `date > out.txt` would print its complaint to the terminal and write nothing
to the file.

Its cost is real and it is in the middle of a pipeline. `a | b`: `a`'s diagnostics arrive at `b`,
which is a `wc` that would count them, and the answer has to be a rule (`wc` drops what it cannot
read? every reader forwards diags upstream?). Unix's answer is that fd 2 bypasses the pipe entirely,
and that is exactly the property one endpoint cannot express. Attaching a rule to it is a protocol
design task, not a wiring one.

### Why this is calef's call

Both shapes are defensible and they commit to different things. The first says a program can have
several output capabilities and the model should name them; the second says a program has one
output capability and the *contract* on it should be richer. That choice constrains everything
downstream: a logging service, a supervisor collecting a child's complaints, and whatever milestone
40's documentation service does with a component's diagnostics.

And there is a real third answer: do nothing. The grounds are that in-band diagnostics on one stream
is what a program with one thing to say should do. And the separation the shell already gives (its
refusals never enter a redirection) is the part that was worth having. That is the current state and
it is not obviously wrong.

Inventing the convention before a program has two things to say would be inventing it rather than
discovering it, which is the same argument milestone 50 made about `InputDeclaration` and got right by
waiting.
