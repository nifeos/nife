# Globbing, and the property that the expansion you see is the grant

Milestone 47's globbing lane. Built 2026-07-31. [glob.md](glob.md) is the matcher, a total function
on two byte strings with no filesystem in it; this note is the layer that turns a match into an
authority, and the demonstration that hangs on it.

The code is `crates/grant_plan/src/expand.rs` (the expander and the name set, host-tested),
`crates/filesystem_contract`'s `nameset` module (the wire encoding),
`components/src/fs_nameset_caretaker.rs` (the caretaker), `components/src/swish/` (`echo`, and the
grant path), `components/src/rm.rs` (the namespace mode), and `kernel/src/user/fs_service/set_grant.rs`'s
`start_granted_set`, and `system_tests/src/user/glob_grant_tests.rs`.

Read [dir-capability.md](dir-capability.md) first for the rights ladder and `fs_subtree_caretaker`,
and [rm.md](rm.md) for why `rm` is a program with a directory grant.

## What a match grants, which was the whole question

`rm *.txt` matching five hundred files has to convey exactly those five hundred and nothing else.
The roadmap's four candidates and its verdict, which this lane implements rather than revisits:

| answer | verdict |
|---|---|
| five hundred file capabilities | honest, and it exhausts capability slots |
| the directory plus a name list | cheap, and it **over-grants catastrophically** |
| make `rm` a builtin | dodges the question, and costs `rm` as a program |
| **a directory capability attenuated to a name set** | the principled one |

The finding that makes it tractable is that this is a small change: `fs_file_caretaker` already
serves *a namespace of exactly one name*, so globbing generalizes the namespace and nothing else.
Same caretaker shape, same `filesystem_contract` protocol above and below, nothing new in the
kernel.

That generalization is in the type system rather than only in the prose. `grant_plan::DirectoryGrant` used
to carry `name: &[u8]`; it now carries `names: NameSet`, and a literal operand is the set of one.

## The property worth the lane

The expansion you see is the grant. `echo *.txt` prints literally the authority that `rm *.txt`
would transfer, because the matched set *is* the namespace the caretaker will serve.

Unix cannot make that claim. Unix's `rm` gets its authority from the uid it inherits; the glob only tells it which of its
existing powers to use. So `echo *.txt` on Unix prints a list of names that happens to be what `rm
*.txt` would delete, which is a coincidence of good behavior by `rm`, not a fact about what it was
handed. Here it is the same object: the shell expands once, and the names it printed are the names
the caretaker is built from.

It only means anything if both go through one expander, which is why `grant_plan::expand::Expander`
exists and why both `echo` and the grant planner drive it. The guest test then checks the pairing
from the other end: a shell rooted in the fixture's `globset` runs `echo gl-*.txt` and plans
`rm gl-*.txt`, and reports whether the names agree. They share the expander, not the
plumbing (`echo` goes text → words → expand → print; the grant goes parse → positionals → expand →
`plan` → the grant's names → render). So a planner that narrowed, reordered or added to a set shows
up as a disagreement. Falsified deliberately: pointing the grant path at a wider pattern turns the
report from `0x3f` into `0x3d`.

## The shell expands before it plans, and that changed `plan_against`

The shell expands first, as Unix does. The consequence is structural: `grant_plan::plan` must see the expanded set rather than the pattern,
since the endowment is the set.

`plan_against` used to fill its slots by splitting a slice of tokens. It now fills them by index,
and takes an `Expansion` keyed to that index. The alternative was to let the planner work out which
slot an expansion belonged to, which would have been the parser classifying tokens again, one layer
down and less visibly. Milestone 47 (navigation and naming) deliberately stopped doing that when it
removed `file:`.

Two guards fall out, and both exist so authority cannot move silently:

- A pattern with no expansion behind it is refused (`Refusal::Unexpanded`), never granted as a
  literal name.
- A token with no magic never consults the expansion at all, so a name that was typed always
  designates itself and no caller can substitute a set for it.

The first guard matters more than it looks. The obvious argument
is that bash's pass-the-pattern-through is harmless here, because a name containing `*` would be
refused downstream. It would not be: neither `grant_plan::file_name_fits` nor the
FS server's `check_component` rejects `*` (they refuse the empty name, `.`, `..`, `/`, `\`, `:` and
NUL, and nothing else). Checked rather than remembered, and there is a host test pinning it. What
that leaves is a worse cost: passing the pattern through would build a grant whose namespace is a
name nobody has, useless today and live the moment anything creates a file called `*.rs`. A grant
should not be able to acquire a referent after it is written.

`Endowment` also stopped borrowing from the command line, because a name a pattern produced comes
out of a directory listing. A planned grant carries values, as `FileGrant::directory` already did by hand,
so nothing afterwards can change what it means.

## Expansion costs `ENUMERATE`, and that is the whole bill

Expanding a pattern is listing a directory, so it costs the authority to list a directory: the rung
[dir-capability.md](dir-capability.md) already separates out. The shell's globbing witness is
granted `ENUMERATE | DESCEND | READ` and no `REMOVE` at all, which is the point of `echo` being the
half that demonstrates this: showing the authority costs none of it.

## `fs_nameset_caretaker`, and why it is a third caretaker

It is a third caretaker rather than a generalization of `fs_file_caretaker` or a mode on
`fs_subtree_caretaker`, which performs no rights checks at all while a name filter is a check on
every name-taking verb. The cost is about thirty lines of duplicated handle table; the three shapes,
and the frame a set needs at start, are weighed in
[glob-grant/third-caretaker.md](glob-grant/third-caretaker.md).

Milestone 61 removed the *other* duplication, which was the dangerous one. Which verbs got asked "is
this name in the set" was a list of match arms in this program. So a name-taking verb added
to the contract would have arrived unfiltered and a set capability would quietly have reached a name
the pattern never matched. It is now `filesystem_contract::verb`'s `takes_name()`, one row per verb,
in a host-testable crate: the filter covers a new verb from the moment its row exists. The
attenuation is not shared: `fs_subtree_caretaker` still consults no policy, the property a mode
would have destroyed.

The distinction that milestone made load-bearing is `Operand::Name` versus `Operand::Payload`. The
four extended-attribute verbs carry a name in the shared page and it is not a name in the
directory, so they pass the filter without being compared against the set. Filtering them would
have refused a program its own file's attributes because `user.com.apple.metadata` is not one of
the names the pattern matched, which is a category error the table now forecloses.

### One rule, and having only one is the design

> A name that is not in the set does not exist here.

Reading it, writing it, creating it, removing it and renaming onto it are all `ENOENT`, because in
this scope there is no such name and nothing consulted a permission. That is `fs_file_caretaker`'s
sentence (DECISIONS §27) over a set instead of over one name, and it is why there is no per-verb
policy here to get wrong.

The filter applies at the granted directory and nowhere else. A handle minted below it, by
descending into a matched directory (which needs a `-r` grant's `DESCEND`), is unfiltered. That is
right rather than a gap: the pattern selected top-level names, and what is under a directory it
selected was never a question the pattern asked.

`RENAME`'s destination is the check that would have been easy to miss. Renaming a matched name
onto an unmatched one destroys a name the capability was never granted, which is an escape even
though nothing was opened. So both names must be in the set, and the consequence is declared rather
than worked around. A set is a *fixed* namespace, so a set capability cannot move a name out of it,
and `mv *.txt` is not something this shape can express.

### `READDIR` is answered from the set

At the granted directory the caretaker does not ask the server at all: the set is the namespace,
so there is nothing to filter out of a listing that is not already absent from this one. That avoids
a filtering caretaker's cursor problem (client and server indexes diverge once an entry is dropped)
and costs no round trip.

It is deliberately not gated on the grant's `ENUMERATE`, and that is not a widening: a listing here
reveals exactly the set, which the command line already printed.

The price is that a set record carries the entry's type, decided at expansion time from what the
directory said. That is the same resolve-at-grant-time rule the rest of a grant follows.

## `rm` is told "everything you can see"

A set grant has no single name to put in the `START` words, so `rm` is started with a grant whose
name is zero bytes long (`filesystem_contract::grant::WHOLE_NAMESPACE`). A name cannot be empty, so
that spelling was free. It means the operand is the namespace, and `rm` learns the names by
enumerating its own capability.

It sweeps in one listing with no rounds, unlike the recursive walk, a fact about the namespace
rather than an optimization. `empty()` must re-read from cursor 0 because
removing a name shifts a real directory's entries, while a set namespace is fixed, so re-reading
would hand the loop the names this run has already taken away.

## The two costs, designed rather than discovered

### A pattern that matched nothing

Refused, at the prompt, with nothing spawned. zsh's default rather than bash's, and here the
model forces it: the expansion is the grant, so an empty expansion is an empty grant and running the
command would be running it with an authority nobody named. The pass-through alternative is argued
above.

`echo` gives the same answer, and it has to. If `echo` printed the pattern where `rm` refuses it,
the two would disagree about what the line designates, which is the one thing this pairing exists to
rule out.

### `ARG_MAX` as a capability limit

You cannot hand a child a hundred thousand names. `nameset::MAX_NAMES` is 8, and exceeding it is
`Refusal::TooManyNames`: a loud refusal at the prompt, never a truncation. A glob that quietly
granted a prefix of what it matched would be the worst outcome this mechanism has, because the
printed preview and the actual transfer would disagree and only the printed one is checkable.

Eight is a measurement: sixteen ran the shell off its stack while planning one grant, so the bound
came down rather than the stack cap going up ([glob-grant/xargs.md](glob-grant/xargs.md) has the two
failures).

## `xargs`: batching at the bound (milestone 109)

`xargs` is a third prefix word in the shell, beside `caps` and `time`. Its operand is a whole
command line, and it re-dispatches that line once per batch of at most eight of what its pattern
matched. `xargs caps rm` shows each batch's grant holding exactly that batch. The name is
provisional.

Batching bounds the authority in flight, so it would be wanted even with an unlimited carrier. It is
a prefix word and not a program, because a batching program would have to hold the directory and so
over-grant every name in it. It is opt-in, so `caps rm *.txt` never prints a single grant that a
silent sweep would make a lie.

A batch is the eight smallest matches strictly greater than the previous batch's last name, so the
resume point survives the command removing what it was handed. The sweep stops at the first batch
that does not run and names the boundary, so what happened is always a prefix of the match. It is a
policy on `Nav::expand`, the shell's single expander, so every command path inherits it and the
refusals survive it. Transcripts and each decision's argument are in
[glob-grant/xargs.md](glob-grant/xargs.md).

## What the tests prove, and from where

Host tests prove the expander, the bound, the refusals, the set encoding and the batching.
`script/swish-check` pins the resume rule at a real prompt. In the guest `rm` is made the attacker
and gets `ENOENT` for a name outside the set, and the host then reads the image and finds exactly
the matched names gone. Each claim and why it is not vacuous is in [glob-grant/tests.md](glob-grant/tests.md).

## BUGS

Known limitations, next to the feature rather than only in a tracker.

- The set filter and the FS server read the name twice, out of one page, and only the first read is
  checked (milestone 43 (a second security audit)'s audit, notes/shared-page-audit.md findings 1 and
  2). The caretaker copies the name off the shared page, compares it against the set, and forwards a
  request carrying only the length; the FS server then does its own read of that same page. The
  caretaker now writes the checked bytes back before forwarding, which makes the server resolve what
  the filter approved. That narrows the window rather than closing it. The FS service memoises one
  frame for every client a boot wires, so any other holder can still land between the caretaker's
  store and the server's load. Nothing can reach it on `main` today, because in the interactive boot
  the shell is the only holder and confined programs hold no budget with which to start a second
  thread. It becomes live once the progenitor builds a *set* caretaker for the prompt, which
  [a-set-grant-at-the-prompt.md](a-set-grant-at-the-prompt.md) proposes. A frame per client channel
  is the fix and is proposed as its own milestone.

- The bound is eight names, and an unbatched line over it is still refused. A directory with nine
  matching files cannot be handed to one invocation; the answer is a refusal, not a partial grant.
  `xargs` sweeps it instead (milestone 109 (`xargs`), above), and that is opt-in on purpose. Lifting
  the number means giving the shell an allocator or the grant a different carrier, not editing the
  constant.
- `xargs` has limits of its own, listed with it in [glob-grant/xargs.md](glob-grant/xargs.md). No
  sweep can run a command that takes a directory grant yet, because no set caretaker is built at
  the prompt. The stop rule sees only failures the shell prints, a sweep lists the directory once
  per batch, and a name created mid-sweep joins it only above the watermark.
- A literal operand's type bit is `false` because nothing enumerated it. `rm` with a literal name
  over two argument words is served through `fs_nameset_caretaker` (milestone 946 (a granted file name can be longer than sixteen bytes)), so its
  listing calls a directory of that name a file. `rm` reads only the names and asks the server.
- A set capability cannot move a name out of its set, so `mv *.txt elsewhere/` is not expressible
  (see `RENAME` above).
- Only the first pattern on a line is expanded. No manifest declares two name slots, so a second
  operand of any kind is already an unplaceable token and a refusal; the day one does, this needs a
  second expansion rather than a loop over the same one.
- No `**`, no qualifiers, permanently and by decision respectively. [glob.md](glob.md) carries both
  arguments: `**` is a traversal feature (descending means holding a capability), and a qualifier
  needs type, mtime and size per candidate, which turns one enumeration into N `FSTAT`s and needs a
  read right beyond enumerate. Neither is a scheduling excuse; both are authority questions.
- At a real prompt a pattern may designate one name, and a set is refused. Corrected 2026-09-26: two
  entries here said the prompt holds no directory and `spawn` refuses every directory grant. Since
  milestone 31 (a capability shell) phase 3 (2026-08-17) the boot shell holds the file service and
  the progenitor builds a `fs_subtree_caretaker` for one name one level down. Two or more names are
  refused in `dir_grant` (`components/src/swish/running.rs`), because `spawnproto` cannot carry a
  set. Only the kernel test wires a set caretaker. How a set should travel is PROPOSED in
  [a-set-grant-at-the-prompt.md](a-set-grant-at-the-prompt.md).
- The set is not consulted below the granted directory, including for attributes. A handle minted by
  descending into a matched directory is unfiltered, argued above for the naming verbs and the same
  answer for the four attribute verbs milestone 61 (the caretakers) forwarded.

## EXAMPLES

At a prompt with a directory capability, the pairing that is the whole point:

```sh
$ echo gl-*.txt
  gl-one.txt gl-two.txt
$ caps rm gl-*.txt
  rm would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 2  endpoint  dir      /  (the directory holding gl-one.txt gl-two.txt)
           ...and nothing under it: no -r, so it cannot even look
  reading the command is reading its whole authority.
$ echo gl-*.rs
  no name here matches that pattern, so there is nothing to grant
```

The same pairing in code, expanding once and granting what was shown, and the guest test wiring a set grant
to attack it, are in [glob-grant/tests.md](glob-grant/tests.md#the-two-code-paths-as-examples).

Run it:

```sh
script/test                  # both ISAs, plus the post-run host check on the image
script/swish-check           # both ISAs: the sweep, at a real prompt, through the real init
cargo test -p grant_plan          # the expander, the bound, the empty match, and the batches
cargo test -p swish          # the sweep's account and its wording
cargo test -p fs_proto       # the set encoding, and both fixtures pinned against the matcher
```

## See also

- [glob.md](glob.md): the matcher, and the four scope decisions this lane inherits.
- [dir-capability.md](dir-capability.md): the rights ladder, `fs_subtree_caretaker`, and why the
  endpoint is the boundary.
- [rm.md](rm.md): `rm` as a program, and why `-r` widens the grant.
- [grant-expression.md](grant-expression.md): the command line as a grant expression, and
  `fs_file_caretaker`.
- Milestone 47's "Globbing, which decides how every multi-file operation grants" in
  `design/roadmap/0047-navigation-and-naming/touch-and-globbing.md`.
