# The fourth pass's `TcpListener` and the last pass's file times

An appendix to [milestone 64 (enough `std` to run somebody else's
crate)](../0064-std-for-real-crates.md), moved out of that page on 2026-10-12 (UTC) under [§212 (a
prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main page is complete
without it. This holds the two bindings that turned out not to need a decision: `TcpListener`
(2026-08-18) and file times by path (2026-09-19).

## The fourth pass: `TcpListener`, which was a binding and not a decision (2026-08-18)

Status did not move: still PARTIAL, gate still NONE. The ranked list was not exhausted (rank 2's
`std::os::unix` fallthrough, rank 3's threads and rank 19's `modified` all still wanted answers
calef had not been asked for). A flip to BUILT would have been a claim this pass did not earn. What
this pass did earn is the largest piece of *ordinary* work the list had left.

`std::net::TcpListener` was `Unsupported`, and it had stopped being a contract gap at milestone 107:
`LISTEN` and `ACCEPT` have been on the wire, with a listen grant behind them, since that milestone
landed. The PAL is now bound to them, and nothing about it needed a decision. `bind` is
`OP_LISTEN`, `accept` is `OP_ACCEPT` into a second socket id with a frame attached, and a listener
carries no frame at all, because a listener carries no bytes (DECISIONS §25).

The authority is the interesting half, and it is what the gate is built around. A listening port
is a grant `net_stack` was spawned with, so the same `std_exerciser` binary does two different things
on two boots and neither of them is a fallback:

| the stack it was spawned over | what `TcpListener::bind` answers | what the program prints |
|---|---|---|
| `NO_LISTEN_GRANT` | `PermissionDenied` | `listen refused`, then the outbound transcript |
| `listen_grant(7778, 7778)` | granted | `listen ok`, and it serves two host connections |

Both transcripts are pinned byte for byte on both ISAs, so a change that widened the grant check
would turn `listen refused` into `listen ok` and fail in a diff rather than pass with more authority
than it was given. The granted run also asserts the two refusals inside its own grant: a port
outside the range is `PermissionDenied` and the granted port asked for twice is `AddrInUse`.

What this unblocks is milestone 55, and it is not a small step: nothing that serves a share
accepts no connections. A `std` program that can listen is the difference between "a crate compiles"
and "a server runs".

## The last pass: file times, which were a binding for three weeks (2026-09-19)

Ranks 19 (`Metadata::modified`) and 28 (`File::set_times`) were carried here as wire-format
decisions until milestone 47's `touch` lane added `GETMTIME`, `SETMTIME` and `SETMTIME_AT` on
2026-08-24 (DECISIONS §112). From then on they were a PAL binding, and the PAL's own comments went
on saying no verb existed. That is the fifth time this milestone has recorded a refusal outliving
its reason.

What is bound, in `patches/std-nife/overlay/std/src/sys/fs/nife.rs`:

| `std` call | verb | answer |
|---|---|---|
| `fs::metadata(p).modified()`, `DirEntry::metadata().modified()` | `GETMTIME` by name, after the existing `OPEN`/`FSTAT`/`CLOSE` | bound, files and directories |
| `fs::set_times(p, ..)`, `set_times_nofollow` | `SETMTIME_AT`, walking with `dir::WRITE \| dir::SET_MODIFICATION_TIME_AT` | bound, whole seconds |
| `File::metadata().modified()`, `File::set_times`, `File::set_modified` | none: the verbs take a name and a handle has none | `Unsupported`, with a message naming the path form |
| an access time in `FileTimes` | none | `Unsupported`, and the mtime beside it is not written |
| `accessed()`, `created()` | none | `Unsupported` |

The premise checked first, per AGENTS.md's fourth question: open, read, write, stat, list and
remove were already bound (milestones 27, 31, 64's first and second passes, 122) and proven by
`std_exerciser` on all three architectures. The outstanding item was exactly the mtime surface, and
this pass bound only that.

Why the `File` forms refuse rather than remember the name they were opened by: the answer would
describe whatever holds that name now, so after a rename it would report, or stamp, another file,
and succeed. That is a wire question (a handle-taking form, which runs into §112 because a file
handle can never carry `dir::SET_MODIFICATION_TIME_AT`), proposed in
`design/roadmap/0504-an-mtime-for-an-open-file.md` rather than decided here.

Why `set_times` never falls back to `SETMTIME`: the caller's `SystemTime` is an assertion, which
§112 (`touch`'s two behaviors need two rights) put behind `dir::SET_MODIFICATION_TIME_AT`. Writing the server's own
"now" in its place and reporting success would be exactly the silent substitution this PAL refuses
everywhere else.

Proved by `std_exerciser`'s `file_times`, six new transcript lines pinned byte for byte on
aarch64, riscv64 and x86_64. They show the following:

- the host-made `motd` reads a wall-clock second inside the same window `SystemTime::now()` is
  checked against;
- a write moves a made file's time forward;
- a time set by name reads back in whole seconds (the half-second is truncated);
- a directory's time reads back by name and through a listing entry;
- four refusals (handle metadata, handle set, an access time with nothing changed, the granted
  directory itself), plus a `NotFound` for an empty set on a missing name.
