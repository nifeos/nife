---
status: DECIDED
raised: 2026-09-26
decided: 2026-09-26
ratified_by: calef
---

# 226. `pidwait` takes tids and composes with `pgrep`, because a program does one thing

calef, 2026-09-26 (UTC), on milestone 126 (the `procps` package: who else is running)'s `pidwait`
fork: *"D."* And the principle behind it, in his words: *"One thing I like about unix is that a
program does one and only one thing."* *(Section number provisional until the merge queue lands
it.)*

## The question

How `pidwait` ships. Upstream `pidwait PATTERN` finds processes by pattern and waits until they
exit. Section 4 of [`notes/process-view/remaining-procps-programs.md`](../../notes/process-view/remaining-procps-programs.md)
found that a pattern-taking `pidwait` holds exactly `pgrep`'s authority, so milestone 281 (`watch`
holds exactly what `ps` holds)'s rule said it was not a program, and recommended a wait mode on
`pgrep`.

## The decision

Option D. `pidwait` takes tids, not a pattern, and waits until each has exited. It composes with
`pgrep` the way FreeBSD's `pwait(1)` does:

```
pidwait $(pgrep foo)
```

It is a separate program. Milestone 281's rule is satisfied because the authority differs: `pgrep`
enumerates a domain, and `pidwait` only observes the exit of tids it was named.

calef's principle is the reason, and it is expected to shape the remaining `procps` rows: a
program does one and only one thing. Finding processes is one thing and waiting for them is
another.

## Refused

- A, `pgrep --wait`. Disorienting to users: a program named for finding that also blocks.
- B, one binary with two names, dispatching on `argv[0]` as procps-ng does. A program should do one
  thing, and a name that changes what a binary does is two programs sharing a file.
- C, a pattern-taking `pidwait` holding `pgrep`'s exact authority. Milestone 281's rule refuses it:
  two programs holding the same authority are one program.

## Prior art

Each was offered from memory and then read against a primary source on 2026-09-26.

- procps-ng builds `pgrep`, `pkill` and `pidwait` from one source. Confirmed: `src/pgrep.c` compares
  `program_invocation_short_name` against `"pidwait"` and `"pkill"` to pick a mode. This is option
  B, shipped.
- The rename from `pwait`. Corrected. procps-ng's `NEWS` records `pwait` as new in 3.3.17 and
  "Rename pwait to pidwait" in 4.0.0. The cause was Debian bug #982391, "pwait is already shipped by
  extrace": a file conflict with another Debian package's `/usr/bin/pwait`. It was not FreeBSD's
  `pwait`, though that one exists too.
- FreeBSD `pwait(1)`: "pwait [-t duration] [-opv] pid ...", which "will wait until each of the
  given processes has terminated". Its HISTORY says it first appeared in SunOS 5.8. Confirmed.
- illumos `pwait(1)`: "/usr/bin/pwait [-v] pid...", "Wait for all of the specified processes to
  terminate." Confirmed.
- GNU `tail --pid`: tail "will exit shortly after all the identified processes no longer exist".
  Confirmed. Waiting on named pids is common enough that a file viewer carries it.
- procps-ng's own `pidwait(1)` synopsis is `pidwait [option ...] pattern`, which is option C.

So D is the Solaris and FreeBSD shape, and C is the Linux one.

## Open for the building lane

These are not part of the ruling. The tree was checked for each on 2026-09-26.

- `$( … )`. The shell (`crates/swish`) has pipes, where "the pipe is an endpoint", and no command
  substitution. So either the shell grows `$( … )` or `pidwait` reads tids on its input, as
  `pgrep foo | pidwait`. That is a design question for the lane, and the second needs nothing new.
- Tid reuse, the race between `pgrep` printing a tid and `pidwait` waiting on it. `crates/abi`
  describes tids as generational names that go stale, so a reused slot should not alias. Whether the
  tid `pgrep` prints carries the generation is the lane's to check.
- How `pidwait` observes an exit. Nothing does this today without more authority than the ruling
  gives it. `rendezvous::RECV` needs `READ` and would take the death message from the supervisor.
  Polling `SURVEY` needs `ENUMERATE`, which is `pgrep`'s authority and would undo the reason D is a
  separate program. So the lane likely owes a narrower way to wait on a named tid, which is a
  system-call-surface question and comes back here. A real sleep also waits on milestone 106 (a wait
  that ends on either the interrupt or the deadline).
- `pgrep` cannot take a pattern at the prompt until milestone 47 (navigation and naming) gives it an
  operand, so `pidwait $(pgrep foo)` cannot be typed end to end before then.

## How reversible it is

Nothing is built, so it is cheap to revisit. The expensive part is the wait primitive above, which
is a wire fact and is decided separately when a lane proposes it.
