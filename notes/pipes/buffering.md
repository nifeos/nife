# Buffering: measured, and the answer is to build nothing

*An appendix to [`notes/pipes.md`](../pipes.md), which is the page to read. This file holds the
2026-08-03 throughput measurement against a Unix pipe, what it says, and its caveats. The lane that
split the file moved it here verbatim from the main page on 2026-09-25 (UTC), under [§212 (a prose
budget)](../../design/decisions/0212-a-prose-budget-for-every-document.md). The directory
`notes/pipes/` and this file's stem are provisional names, minted that day by the lane that split
the file. Naming is calef's.*

The records this file cites by number:

- milestone 50 (pipes and redirection)
- §51 (the sink protocol)


## Buffering: measured, and the answer is to build nothing

The roadmap block said it plainly: measure `a | b` throughput against a Unix pipe before deciding
anything. If buffering earns its place, it arrives as a component speaking the sink contract on both
sides. The measurement came on 2026-08-03. It has not earned its place, and the number says
something more useful than "no".

### The measurement

`bench: sink_throughput` (`kernel/src/bench.rs`) is a pipeline with the shell taken out: two EL0
processes, one endpoint, the left one packing sixteen bytes into a sink message and `SEND`ing, the
right one `RECEIVE`ing and self-timing. `bench/host/pipe_throughput.rs` is the same shape over a real
`pipe(2)`, twice, because only one of the two arms is apples to apples.

Apple Silicon, one machine, one sitting. nife under HVF (`cargo xtask bench --real`), so the
nanoseconds are real; the host arms are medians of three.

| | per 16 bytes | throughput |
|---|---|---|
| nife `a \| b` (one endpoint, no buffer) | 1146 ns | 13.3 MiB/s |
| macOS pipe, 16-byte writes | 348 ns | 44 MiB/s |
| macOS pipe, 64 KiB writes | (5.4 µs per 64 KiB) | 11,600 MiB/s |

Two reference points from the same run make the first row legible. `ipc_rtt_el0` is 2785 ns for a
round trip, so a one-way rendezvous is about 1.4 µs. Our pipe is one rendezvous per message and
nothing else: there is no overhead to find. And `relay_rtt` (1187 ns) against `ipc_rtt` (2313 ns)
prices what a hop through a userspace process costs, which is roughly double.

### What the numbers actually say, which is not what the block expected

The lockstep is not the bottleneck. The sixteen-byte message is.

Read the two host rows together. At the same granularity Unix is 3.3x faster than us, which is a
real gap and is the cost of a capability rendezvous against a tuned kernel pipe. The 870x is
somewhere else entirely. Unix's win is that a program writes 64 KiB per syscall, while the
register-only sink contract caps ours at sixteen bytes per message.

That cap is not an oversight and it is not a thing buffering fixes. `notes/sink-protocol.md` records
why the contract had to be register-only: the moment a sink also needs a page mapped at an agreed
address, redirection stops being "put a different capability in one slot" and becomes a three-way
spawn-time negotiation, and milestone 50's whole finding evaporates.

### So a buffering component would make it worse, and that is the decision

Insert a process between the two ends and every message pays a second rendezvous. `relay_rtt` prices
that at roughly double, so an 80 KB pipeline would go from 5.7 ms to something near 11 ms. A buffer
cannot batch its way out of that, because what it forwards is still sixteen bytes per message. The
contract it speaks on both sides is the one that sets the cap.

Buffering buys decoupling, not bandwidth. It wins when a producer has *work to do between writes*
and a consumer has work to do between reads, so the two can overlap instead of alternating. It does
not win when both are only moving bytes, which is what this benchmark and every pipeline in this
system today are.

So nothing is built, and that is the successful outcome the block described. What would move the
number is a larger message, and that is a different decision with §51's indifference on the other
side of it. It is not taken here and it is not needed. `date | wc` and `ls | wc` are hundreds of
bytes, and at 1.15 µs per sixteen that is tens of microseconds for a whole line.

### The honest caveats

- The benchmark does not measure the case buffering is for. Both ends do nothing but move bytes,
  so there is no producer-side work for a buffer to overlap with the consumer's. A pipeline of two
  programs that each compute would show a different shape, and if one is ever built, this is the
  benchmark to extend rather than the conclusion to keep.
- The host arm is macOS, not Linux. `bench/host/run_linux.sh` exists for the cross-OS suite and
  this program belongs in it. The Linux number is not taken here, and Linux's pipe is a different
  implementation with a different fast path.
- The 64 KiB row is nearly memcpy-bound and the producer runs ahead through the whole transfer,
  which is the buffer effect at its most flattering. It is in the table because it is what a Unix
  program actually gets, not because it is a fair comparison.
- `sink_throughput` is in `bench/baseline-*.txt` on both ISAs, under the same 10% tripwire as
  every other row, with a comment on the row that its second column is bytes rather than
  iterations.
