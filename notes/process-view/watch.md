# `watch` was built, and then cut (milestone 281)

*Name: provisional. For the program it records.*

An appendix to [notes/process-view.md](../process-view.md), which summarizes it. `watch` is gone as
of 2026-09-13, along with `crates/watch`. The reason is the useful part of this page rather than
the obituary.

## What it was

`watch` re-walked the domain a bounded number of times and prefixed each table with
`CSI 2J`/`CSI H` (erase, then home the cursor), so the terminal showed the latest snapshot in place.
It was granted exactly what `ps` and `pgrep` are, plus a required argument, and that turned out to
be the whole problem. It held the same three slots from the same `grant_plan` constants (`REPORT`,
`DOMAIN_SLOT`, `DIAGNOSTICS_SLOT`). And it was, literally, `ps`'s own loop, down to a `diag_slot()`
fallback whose comment read "`ps`'s own fallback, verbatim".

## The test it leaves behind

That is a reusable test, and it is why this page stays after the program is gone. In a capability
system "should these be one program or two" is not a matter of taste. Two programs are two programs
when they hold different authority. Identical cspaces mean there is no boundary being drawn, and the
split is arbitrary.

The refresh needed no capability of its own either. The interval was a yield-spin over
`monotonic_nanos`, granted to every process unconditionally, rather than a clock grant. So even the
least-authority argument for a separate program was absent. Had the interval needed a clock, the
answer would have gone the other way.

Milestone 281 (`watch` holds exactly what `ps` holds) was minted to fold it into `ps --watch N` on exactly that argument. calef took it one
step further. The flag's entire content was a busy-wait over a table of two columns that barely
changes. So deleting it buys the same simplification and costs nothing anyone was using.

## It was never in `watch`'s family

Upstream `watch` re-runs an arbitrary command, which this one could not. Re-running a named command
needs a program to hold spawn authority. Here that belongs to the shell alone
(`grant_plan::spawnproto`), and is granted to nothing the shell spawns: an interruptible child is
built with no capabilities in its capability table at all. So there was no route from "a program is
running" to "that program can start a second one". That is the same category of gap `top`, `pwdx`
and `w` were blocked on.

So it redrew the one thing it could already reach: the domain it was spawned into, which is `ps`'s
own listing. It was a very thin member of `top`'s family wearing `watch`'s name. That is why the
name was wrong, and why neither it nor `crates/watch` was ever ratified. calef declined to rule on
both while this milestone might retire them; it did.

## What replaced it

Not a flag, but milestone 282 (a thread's CPU time, and the `top` it makes possible), which gave the domain something to rank by: per-thread scheduled CPU
time (DECISIONS §150 (how does a thread's CPU time reach userspace?)). It is read as `abi::survey::record::CPU_TIME`, through the selector §204 (how
userspace asks where a thread runs) put on `SURVEY`. The live view came back as `top`, ranked,
rather than as a refresh flag on a table of two columns.
