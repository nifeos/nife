---
status: NOT-STARTED
raised: 2026-09-13
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 279. Documentation search is a shell builtin, so a second shell has none

calef's question: if the search is built into swish, is there no documentation search with a
different shell? There is not.

calef decided the shape when he raised it: loosen the coupling with a service rather
than record it as a limitation. What is still his at build time is the wire format and the protocol
crate's name, like every other thing two programs agree on.

In brief. `apropos` is a builtin in `crates/swish`, and the capability argument for that is
sound. Searching means reading the documentation store, and a *program* holding that capability
widens the contract of every documentation program, which is exactly what `doc`'s two-slot table
exists to demonstrate. The consequence nobody wrote down is that the search is not a component:
replace the shell and it is gone.

## What is actually coupled, measured on `4306c8c1`

Three pieces, and only one of them is portable.

| Piece | Where | Portable? |
|---|---|---|
| The ranking query | `manual::index::search` | Yes. Any consumer can call it |
| The store walk | `components/src/swish.rs:937 fn apropos` | No. Opens `index::STORE_DIR` from the root through the shell's own directory capability and rights, copies the manifest, hands each shard to `search` |
| The presentation | `crates/swish/src/lib.rs:606 write_apropos` | No |

So a second shell reuses the ranking and rewrites the other two, and must itself hold a directory
capability rooted where the store is installed. There is no `apropos` program to fall back on, by
design.

The walk already exists three times. The guest builtin above, `cargo xtask manual <word>` on the
host (`swish.rs`'s own comment: "the same function ... over the same bytes"), and `script/apropos`
pointed at the checkout rather than the image. A fourth consumer is a fourth copy.

## The shape

A server holding the store and answering queries over an endpoint, with `apropos` as a thin client
any shell can spawn. **The tree already has this shape for filesystems**, so this is applying a
precedent rather than inventing one, and the capability argument survives intact: the store
capability lives in exactly one place, and what crosses the endpoint is a term in and a list of
names out. `doc`'s two slots do not change, and neither does the property that finding a page grants
nothing.

The wire contract is a crate, per AGENTS.md's codebase rule 7, and its name is an architect's. A
lane ships a provisional one.

## The refusals

- Record it in `BUGS` on `crates/swish` and stop. Honest and cheap, and it is what this milestone
  would have been if calef had ruled the other way. Refused because the cost it accepts is that
  "shell" permanently means "shell plus documentation search", which is a coupling a newcomer writing
  a second shell discovers by finding the feature missing.
- Make `apropos` a program holding the store capability directly. Refused for the reason the
  builtin exists: it widens a program's contract to include the store, and then the `man`/`apropos`
  capability split that `doc`'s header presents as the demonstration stops being true.
- **Move the store walk into `crates/manual` and leave the builtin.** It would kill the triplication
  and nothing else; the second shell still has no search. A real improvement, and it is this
  milestone's first phase rather than an alternative to it.

## BUGS

- This adds a running service to reach a page, where today the shell reads the store directly.
  That is a process and an endpoint on a path that currently has neither, and nobody has measured
  what it costs at the prompt.
- **The store is opened from the root rather than the cwd** (`swish.rs`'s own note: a `cd` does not
  move the manual). Whatever the service holds has to preserve that, or a search starts answering
  differently depending on where the reader is standing.

## Follow-on

- **None.**

## Index row

If the search is built into swish, there is no documentation search with a different shell. `apropos` is a builtin in `crates/swish` and the capability argument for that is sound: searching
reads the store, and a program holding that capability widens the contract `doc`'s two-slot table
exists to demonstrate. The consequence nobody recorded is that the search is not a component. Only `manual::index::search` is portable; the store walk (`swish.rs:937`) and the presentation are not,
and the walk already exists three times. A server holding the store with `apropos` as a thin
client applies the shape `fs_server` already has and keeps the capability argument intact. Wire
format and the protocol crate's name are an architect's.
