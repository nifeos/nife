---
status: BUILT
raised: 2026-09-04
built: 2026-09-04
---
# 380. DECISIONS §144's ceiling names two terms and milestone 188 changed both

`design/decisions/0144-fastpath-footprint-ceiling.md` already carries this correction, so nothing in
this file is unfinished work. It states the ceiling as *"an absolute ceiling of 16 KiB, per
architecture, on `script/fastpath-footprint`'s `total`"*, gives both reasons below, concludes that
*"the ceiling's subject is `max(ipc_send_recv, ipc_call_reply) + syscall_entry`"*, and prints the
same three-row table, 52%, 47% and 60% of 16 KiB. The body below is the case that was made.

## In brief

§144 sets an absolute ceiling of 16 KiB per architecture "on the sum of `ipc_fastpath` and
`syscall_entry`", and derives the number from `notes/target-hardware.md`'s 32 KB L1i requirement. It
records the totals it was measured against as aarch64 9,156, x86_64 8,404, riscv64 7,174.

Milestone 188 phases 1 to 3 (pull request #732) moved both terms, on the same day:

- `ipc_fastpath` measured the SEND/RECV shape, which essentially no service in this tree runs.
  The gate now reports `ipc_send_recv` and `ipc_call_reply` separately and keeps `ipc_fastpath` as
  the **worse of the two**, since one round trip is one shape or the other.
- **aarch64's `syscall_entry` counted all sixteen exception vector entries** where an `svc` fetches
  one, so 1,892 of its 3,304 bytes were never fetched.
- Phase 3 then took 6 to 10% off both closures by outlining cold arms.

## What the sentence should say

**The ceiling's subject is `max(ipc_send_recv, ipc_call_reply) + syscall_entry`**, which is what
`script/fastpath-footprint` now prints as `total`. That is the same claim §144 already makes,
evaluated on numbers that are true.

| | §144, as recorded | after milestone 188 | fraction of 16 KiB |
|---|---|---|---|
| aarch64 | 9,156 | 8,536 | 52% |
| riscv64 | 7,174 | 7,764 | 47% |
| x86_64 | 8,404 | 9,759 | **60%** |

x86_64 is the one to watch. Its total rose, because counting the shape services run adds more
than any other correction removes there, and it is now 60% of the ceiling against the 51% §144
recorded. §144's own reasoning was that the ceiling "sits about 75% above the largest and fires
rarely"; it now sits 68% above the largest. Neither this proposal nor the milestone asks calef to
move the number.

## Why it is worth writing down rather than leaving

§144's `BUGS` already says the measurement is "an upper bound and a loose one" and that "the bytes an
IPC actually touches are fewer and nobody knows by how much". Milestone 188 narrowed that by a
measured amount on one architecture and widened what is counted on all three. A decision whose
recorded headroom is 9 points optimistic on x86_64 is the kind of stale record this tree keeps
finding by accident.

## Follow-on

- **Recorded.** x86_64 is the one to watch and the decision says so: its total rose to 60% of the
  ceiling against the 51% §144 recorded, because counting the shape services actually run adds more
  there than any other correction removes. Recorded in
  `design/decisions/0144-fastpath-footprint-ceiling.md` beside the table, which is where a reader
  meets the number.
- **Recorded.** §144's own `BUGS` still says the measurement is an upper bound and a loose one, and
  that the bytes an IPC actually touches are fewer by an amount nobody knows. Milestone 188 narrowed
  that on one architecture and widened what is counted on all three; it did not close it.
  `design/decisions/0144-fastpath-footprint-ceiling.md`.
- **Refused.** Moving the 16 KiB number. Neither this block nor milestone 188 asked calef to, and
  neither should be read as having done so. The ceiling's derivation from a 32 KB L1i is untouched
  by a correction to what is being summed, and only calef raises it.

## Index row

DECISIONS §144 (the fastpath footprint gate gets a delta and a ceiling, and the ceiling is 16 KiB)
set a 16 KiB ceiling "on the sum of `ipc_fastpath` and `syscall_entry`", and milestone 188 (the IPC
fastpath) changed both terms. The gate now reports two IPC shapes and keeps the worse, and aarch64's
entry figure stopped counting fifteen exception-vector entries an `svc` never fetches. The ceiling's
subject is `max(ipc_send_recv, ipc_call_reply) + syscall_entry`, which the script prints as `total`.
The headroom §144 recorded was 9 points optimistic on x86_64, which is now 60% of the ceiling and
not 51%.
