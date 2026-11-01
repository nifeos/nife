# The memory grant: why untyped had to become delegable, and the proof it is real

*Name: provisional.*

An appendix to [notes/grant-expression.md](../grant-expression.md), moved there on 2026-10-11
(UTC) under §212 (a prose budget). It records the kernel fix `--mem N` needed and the program
that spends what it was granted.

## The region had to become delegable first

Making `--mem N` real, and not parsed-and-ignored, needed a kernel fix, recorded as an amendment
to DECISIONS §16 (object revocation). `MemoryRegion::SPLIT` minted its child budget with `WRITE` alone, so it could be spent
but never delegated (`SEND_CAPABILITY` and `CAPABILITY_INSERT` both gate on `GRANT`). A memory region was the one object
type no process could hand on, which quietly foreclosed the whole feature.

The fix is rights inheritance, not a blanket upgrade, and the distinction matters. Minting the
`SPLIT` child full rights unconditionally would be an escalation, since `SPLIT` gates only on
`WRITE`. A process holding a spend-only untyped could split itself a `GRANT`-bearing child and
manufacture the right it was denied. Instead, a `SPLIT` child inherits the invoking capability's
rights and no more, and the root untyped the progenitor holds at boot is the delegable one
(`READ|WRITE|GRANT`). Rights narrow monotonically from that root down: root -> the progenitor split (inherits
`GRANT`) -> shell (narrowed to `WRITE|GRANT` at `CAPABILITY_INSERT`) -> shell split (inherits) -> spawned
child (narrowed to `WRITE`, spend-only). `GRANT` never appears where it was not present above.

## The memory_grant_depleter proves the grant is real

`memory_grant_depleter` is a program whose whole job is to spend the memory it was granted: it maps pages out of
its slot-1 untyped until the budget is exhausted, then reports the count. The number it prints is
the authority the command handed it. `memory_grant_depleter --mem 16` reports 15 pages mapped on both
ISAs: the sixteenth paid for the page table that reaches the others (the kernel allocates nothing on
a process's behalf, DECISIONS §10). Grant more and it maps more; grant nothing and it holds no
untyped at slot 1 at all, so its first `MAP` returns `NoSuchSlot` and it maps zero. There is no
ambient pool behind it. This is the demonstration that `--mem` moves real memory, not a parsed
number.

## A job may not leave a child region in its budget when it exits

A program that holds a `--mem N` grant may `SPLIT` it and build in the pieces. It must give every
piece back before it exits, by `DESTROY` on each sub-region it split. This is a contract for the
program's author (ruled 2026-11-01 UTC, milestone 933). Nothing enforces it at exit: the kernel
cannot refuse a thread's exit.

When the job exits, the shell gives the grant back with one `DESTROY`. The kernel refuses that for
good while a child region lives (`HasChildren`, DECISIONS §16), and the job's table is gone, so
nobody can name the child. The grant's pages stay spent until reboot. The shell reports it: it
reads `USAGE`'s `CHILDREN` record after the refusal and, when a child is live, prints one sentence
naming the job and the pages, without waiting out the retry bound. A refusal for a process still
running in the grant is retried as before.

The rule can be relaxed later by a recursive teardown, which breaks no program written to it;
tightening it later would break programs. No program the shell runs splits its grant today.
