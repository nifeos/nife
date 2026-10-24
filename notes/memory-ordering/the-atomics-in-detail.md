# The ordered atomics in detail

*Name: provisional.*

Four adjudications behind the protocol table in [`notes/memory-ordering.md`](../memory-ordering.md).
Two sites are sound for a reason nobody had written down. One publication order has no reader that
depends on it yet. And one question was worth asking of the `Relaxed` sites.

## `HWID`: an acquire with no release

`HWID` in `kernel/src/smp.rs` is read with `Acquire` and written with `Relaxed`. That is an acquire
whose release does not exist, which is this milestone's bug class in mirror image.

It is sound. Nothing is published behind `HWID`. The publication flag for the whole roster is
`ROSTER`, and a caller reaches a valid slot index only by having read `ROSTER` with an acquire
first. The acquire on `HWID` is decorative.

The tell is that the same array is read with `Relaxed` thirty lines further down. So the file
already disagrees with itself about whether that ordering means anything. It was left as it is.
This milestone changes no ordering that is not shown wrong, and an unnecessary `Acquire` removed is
still an ordering change with no argument behind it.

## The compositor's `publish`: a wrong comment

`components/src/compositor.rs`'s `publish` had a comment that was wrong. This is the one place the
inventory corrected the record rather than the code.

It writes six control-page fields with plain `write_volatile` and then writes `MAGIC` last. The
comment said "the store order is what makes the check mean anything". It is not.
`write_volatile` guarantees that the access happens and guarantees no ordering at all. On aarch64
the interconnect may make `MAGIC` visible before the fields above it.

What makes the check sound is that `publish` runs once, before the serve loop. No client's `HELLO`
can be answered until every page is written. Both client programs say exactly that at their end,
and this end did not. That is the same one-sided shape as a fence: the argument existed and lived
somewhere the reader was not. The comment now carries it, and says what would have to change if
`publish` were ever called again while clients run.

## The window list, which no reader has yet

`publish` writes `wlist::COUNT`, then `FOCUSED`, then the per-window `RECORDS`. If `COUNT` were ever
used as the publication flag for the records it counts, that order is backwards.

It is not used that way today, because nothing in the tree reads `RECORDS` at all.
`fixtures/src/window.rs` reads `COUNT` and `FOCUSED` and reports them, and the kernel reads
`FOCUSED`. This is recorded rather than fixed. The first reader of `RECORDS` is the change that
makes it matter, and that reader should meet this paragraph.

## Relaxed, and the one question worth asking of it

The 243 `Relaxed` sites were not adjudicated individually; the brief scoped this milestone to the
sites that carry ordering. One sub-question was worth asking anyway. A relaxed compare-exchange
used as a claim is the classic form of this bug.

There are fifteen compare-exchange sites outside test code. At twelve of them the `Relaxed` is the
failure ordering, which is conventional and right. The three that are relaxed on success publish
nothing behind the value:

- `kernel/src/sched.rs`'s thread-budget reservation.
- `kernel/src/pci.rs`'s BAR cursor.
- The interrupt-routing lottery in `kernel/src/arch/*/irq.rs`, where the loser of the race reads
  the winner's answer out of the compare-exchange's own failure return.

In all three the value is the whole protocol.
