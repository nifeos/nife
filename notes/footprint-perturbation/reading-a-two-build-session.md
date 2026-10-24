# Reading a two-build session: placement, and what each outcome means

*Name: provisional, minted 2026-10-11 (UTC) by lane/ten-longest-notes.*

An appendix to [notes/footprint-perturbation.md](../footprint-perturbation.md). It holds how the
2026-09-04 procedure controlled for placement, and the routing table it was written to fill in. The
2026-09-04 capture matched none of the E3 rows, because all of them assume the only variable is
footprint; see [session-2026-09-04.md](session-2026-09-04.md).

## Controlling for the thing that has already burned this project

Placement decides throughput on radon by up to fifteenfold (milestone 240 (the soak reports what
happened and not where), notes/soak.md). Four soak runs on the same card spanned that range, and the
census explains them by how many cores held an IPC thread and no grinder. A latency number from a
single boot is a draw from that distribution.

A `single_hart` card removes the lottery rather than controlling for it, and that is the strongest
form available. There is one core, so there is no spawn placement to draw, no work stealing, and no
migration; `pick_spawn_target` has one answer. The fifteenfold spread cannot be reproduced on this
card, because the mechanism that produces it is not present.

That is not the same as saying the boots are identical, and the remaining variation is why step 4
repeats them anyway:

- DRAM training and the U74's own cold state differ boot to boot.
- The archive is measured at boot, the heap is laid out fresh, and where E1's 96 thread stacks land
  relative to each other is a property of one boot's allocator history.
- Nothing here is pinned to a hart *number*. OpenSBI's boot hart is not guaranteed to be 0, so a given
  boot runs on whichever hart it woke on. All four U74s are the same core, but they are not the same
  silicon.

So: three boots minimum per condition, interleaved, and report the spread rather than a mean. An E3
effect smaller than the boot-to-boot spread is not an effect, and saying so is the finding.

## What each outcome means, and where it goes

This is the table the session existed to fill in. The phase 4 of milestone 188 (the IPC fastpath) is
a hand-written IPC fastpath, held by calef pending evidence that the footprint costs anything real.
Its phases 1 to 3 measured `ipc_call_reply` at 48% to 103% over the 4 KiB target. So the bytes are
settled, and the cycles are not.

| what the capture shows | what it means | where it routes |
|---|---|---|
| `call_reply` and `ipc_rtt_el0` clearly slower padded, beyond the boot-to-boot spread | Liedtke's claim is live on this machine at this footprint. §95's premise holds, measured rather than argued | milestone 188 phase 4 is justified; the magnitude is the expected payoff of a hand-written fastpath and the number to hold it to |
| padded and un-padded within the spread, as on patagonia | a doubling of footprint costs nothing measurable on a 32 KB L1i either. This is the strong direction of the negative: E3's own design says a null here is worth far more than a null on patagonia | §95's premise is in serious doubt. 188 phase 4 buys a standing verification obligation for an effect two machines cannot find. Route to `design/decisions/0095-*` as evidence for closing it |
| `ipc_rtt` moves and `call_reply` does not (or the reverse) | the effect is real but shape-specific, which is a result about *which* path to optimize rather than whether to | 188 phase 4, with a narrower scope than currently sketched |
| E1 `ipc_scale_*` bends sharply in the low tens | Warton's effect reproduced on the machine the prediction was computed for. §96's performance input is live | `design/decisions/0096-process-kernel-or-event-kernel.md`, read against E2's finding that the customer path runs 4 to 8 threads |
| E1 flat to 96 threads | the process kernel costs nothing on this axis on the smallest cache we target. Stronger than patagonia's 8-11% rise, in the opposite direction | §96 answered no, on data |
| E4 `_ipc96` clearly above `_ipc`, and both above zero | application displacement is real and load-dependent: the Liedtke measurement proper | the register, and 188 phase 4 as supporting rather than deciding evidence |
| E1 or E4 print `skipped` | the card was built wrong. `needs a single hart` means `single_hart` was missing; `QEMU virt detected` means this is not the board | rebuild, do not interpret |
| `MEASURED BOOT REFUSED` | kernel and archive came from different builds | `script/board-image --card` copies them as a set; copy all three files again |

Two outcomes are worth naming as genuinely decisive, and one is not. Rows 1 and 2 both settle 188
phase 4, in opposite directions, and both are worth the session. A result that lands inside the
spread but "looks like" a trend is the outcome to resist. This project has a recorded habit of
reporting overlapping ranges as directional findings, and E4's own 2026-08-23 follow-up says so in its
own words.
