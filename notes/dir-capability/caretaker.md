# The subtree caretaker: why it checks nothing, the verb table, and one frame

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.*

An appendix to [notes/dir-capability.md](../dir-capability.md), moved there on 2026-10-11 (UTC)
under §212 (a prose budget). It holds the detail of `fs_subtree_caretaker` behind the main page's
structural finding.

## `fs_subtree_caretaker`, and why it checks nothing at all

```text
  FS server ──file IPC──► fs_subtree_caretaker ──narrowed file IPC──► the confined program
  (the image root)                  (one subtree, one rights set)
```

`fs_file_caretaker` has to inspect requests, because a file capability and a directory capability
speak different protocols and it is translating between them. `fs_subtree_caretaker` performs no
rights checks, by design.

At startup it sends exactly one `OPENDIR`, asking for the granted name with the granted rights.
The FS server intersects those with its own and refuses if the intersection came up short. So a
wiring that asked for more than exists dies at the caretaker's first request, instead of coming up
serving a capability nobody meant to hand out. Everything the client can reach afterwards, it
reaches *through the handle that request minted*. There is no branch in the caretaker that could
be wrong about it.

What the process does is translate a namespace. The client numbers its handles in its own space
starting at `fs::ROOT`, which is the granted directory. The caretaker maps each to the FS server's
number and forwards the request otherwise unchanged. A client that guesses a handle is guessing in
a table with a handful of inhabitants, none of which it chose. A number the caretaker never minted
is `EBADF` from one check.

It costs no memory: the granted name and the rights mask ride in the three `START` argument words
(`filesystem_contract::grant`), and one frame is shared by all three processes.

## The verb table, and how it stayed a translation (milestone 61 (the caretakers: one verb table))

Each of the three caretakers had a hand-written `match` over the opcode, and nothing made a
`match` and the contract agree. So the way it failed was that a verb added to `filesystem_contract`
was absent from a caretaker, and the capability silently was not there. That is not hypothetical.
Milestone 57 (partitioning and formatting a real drive, and extended attributes) added the four
extended-attribute verbs, none of the three was taught them, and nothing
failed. Programs behind every kind of grant could not reach their files' attributes.

`filesystem_contract::verb` is a row per verb. It says what the request word's length field counts
(`Operand::None`, `Name`, `Payload`, `Rename`), whether the second word means anything, whether the
reply is a new handle, and which `dir` rights the server will demand. `verb::of(op)` is the whole of
a caretaker's dispatch now. A verb with no row is a compile error, so forgetting fails the build
rather than producing a capability that is quietly missing.

The table shares the dispatch and never the attenuation, which is what keeps this program's one
strong property intact. A table lookup that decides whether to forward the length field or a zero
cannot refuse anything. A name filter or a rights test here would be a branch that could be wrong.
`fs_subtree_caretaker` consults no policy table at all, and there is nothing for it to consult: the
attenuation is still entirely in the handle the server minted.

The three caretakers therefore stay three programs, and the roadmap's refutation of collapsing them
holds. `fs_subtree_caretaker` and `fs_nameset_caretaker` serve identical verb surfaces by opposite
means, one by checking nothing and one by checking every name. `fs_file_caretaker` translates
between two protocols rather than narrowing one.

## One frame, and the startup ordering that argument does not cover

The one-frame argument is `fs_file_caretaker`'s. Every request on both hops is a blocking `CALL`, so
the client is parked inside its own call for the whole time the caretaker is using the page. A
second frame would buy a copy and no isolation.

That holds once the caretaker is serving. It does not hold at startup, and this cost a debugging
round. The caretaker stages the granted name in the shared page and then blocks in a `CALL` to the
FS server. A confined program that already exists writes its own first name over that page, and the
FS server resolves whatever it finds there.

In the case that failed it is not even a race. When the wiring call is the one that wires the FS
service, the FS server is parked inside its readiness `SEND`. So the caretaker's descent cannot be
answered until somebody drains it, and the client has that entire window. The caretaker then died
rather than serve a hole, and its client blocked forever on a call nobody would answer: a userspace
`ebreak` followed by the 60 s lost-wakeup watchdog. It passed on aarch64 and failed on riscv, which
is the shape of a timing bug and was one.

The fix is ordering, not a second page. Draining the readiness sentinels is sequencing, not only an
assertion: each server is parked inside its blocking announcement until somebody receives it, so
nothing it serves can be answered first. `fs_service::start_granted_dir` now drains the service,
waits for the caretaker's own sentinel, and only then spawns the confined program.
`fs_file_caretaker` had the same latent bug and took the same three lines.
