# The `ps` crate: why its logic has the shape it has

`crates/ps` is the logic of `ps`, lifted out to run on the host; `components/src/ps.rs` is the
syscall and the two output streams. [process-view.md](../process-view.md) has what the view is.
This page holds the reasons the crate's doc comments link.

## Three answers, kept apart

A monitor that reports nothing because it could not look reads exactly like a quiet machine. So
three answers stay distinct to the terminal:

- rows, on the output stream;
- an empty domain, said on diagnostics;
- a refusal, named on diagnostics with no rows.

The empty domain is not an error and must not read like one. The caller held the domain and was
allowed to look, and there was nothing in it. That sentence is what distinguishes an empty answer
from a refused one, which is the distinction Linux's `ps` cannot draw.

`Survey::complaint` is split out of `write_diagnostics` so a second program over the same survey
reuses its sentences. `crates/pgrep` filters this listing and has to describe a refusal in the same
words, or the two programs drift into disagreeing about what one refusal means. The clause carries
no program name and no newline for that reason. The empty domain comes last in its order because
it is the only one of the four that is not a failure.

## Collect first, then speak

§67 (a program's second stream is a declaration) requires every complaint said, and that stream
closed, before a byte of output. An endpoint can be destroyed mid-walk. So `collect` takes the whole
domain into the caller's buffer first, and the caller then emits `write_diagnostics` and
`write_report` in that order.

The walk stops at the first refusal and reports it, rather than skipping the entry and carrying on.
A survey that silently dropped an entry it could not read would be the failure this program exists
to avoid, one row smaller. The reader is a closure rather than a syscall, which makes the empty
domain, the refusal and a domain of a hundred threads all reachable from a host test.

## The buffer, and what it costs

The buffer is the caller's because a `[Row; MAX_ROWS]` local made `collect`'s frame larger than the
4,096-byte guard page under every kernel thread stack (`script/stack-frame-check`).

`MAX_ROWS` is the kernel's whole thread table, so a buffer of that size makes truncation unreachable
rather than handled. A `ps` holding the widest grant this system can express sees every thread on
the machine, so there is no "and N more" the shipped program can print. A compile-time assertion in
`kernel::user::survey_tests` keeps `MAX_ROWS` and `sched::MAX_THREADS` in step.

The buffer is a stack allocation in both shipped programs, so its size was measured.
`-Z emit-stack-sizes` puts `_start` at 4,240 bytes in `ps` and 4,320 in `pgrep`. That is against
the 12 pages (49,152 bytes) `system_initializer::CHILD_STACK_PAGES` maps under every child: under 9%
of the stack each. Nothing else in these programs is deep; `collect` is 368 bytes and the next frame
down is smaller still.

## Two walks joined on the tid

`Survey::join_cpu_time` fills the CPU-time column from a second walk of the same domain. Two walks
joined on the tid, rather than one walk carrying two facts, is the shape DECISIONS §204 (how
userspace asks where a thread runs) chose when it made the record a selector. The cursor and the tid
are the same whichever record is asked for, so only the third word moves. A program that wants one
fact pays nothing for the existence of the others. The cost is one extra syscall per thread, which
§204 priced at about 45 microseconds per second for sixty-four threads at a one-second refresh.

A domain can change between the two walks. A tid in the first walk and not the second is a thread
that died in between: its row keeps `None`, and the listing prints that it does not know rather than
borrowing a number from another row. A tid in the second walk and not the first is ignored, because
there is no row to put it in. That is `readdir`'s bargain, the same one a single walk already takes
between its own calls.

`None` rather than `0` is the point of `Row::cpu_millis` being an `Option`. A zero is a thread that
has never been on a CPU at a tick, which is a different fact from "not asked". When ranking, a row
with no figure sorts last, because putting "not known" at the head would be the strongest possible
claim about the one row that has nothing to say.
