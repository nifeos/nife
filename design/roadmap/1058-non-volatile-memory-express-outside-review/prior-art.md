# Prior art for the non_volatile_memory_express crate

An appendix to [the milestone](../1058-non-volatile-memory-express-outside-review.md).

## Drivers and libraries

Recent downloads are the last 90 days at the time of the review. Where a row says "not read", the
reviewer looked at the front page of the project and not at its code.

| project | what it is | recent / all-time downloads | what a user coming from it misses here |
|---|---|---|---|
| Linux `drivers/nvme/host` | the in-kernel driver: a tag set per queue, PRP lists and SGLs, one queue pair per CPU, MSI-X, reset and timeout handling, shutdown, Identify Controller | not applicable | everything in the second column: this crate builds one command and walks one ring pair, and leaves tracking, interrupts, time and the admin sequence to the caller |
| SPDK `lib/nvme` (C) | the polled user-space driver, the reference for behavior | not applicable | the whole admin command set, queue pairs at scale, a request abstraction |
| Redox `nvmed` (Rust) | a driver daemon, not a library; its GitHub mirror is archived | not applicable | a driver to run, where this crate is a library to build one with |
| `nvme-driver` 0.8.2 (rcore-os, in the `tgoskits` monorepo) | a Rust driver crate with one reverse dependency on crates.io; not read | 858 / 5,133 | a driver that owns its DMA and issues commands; it advertises no proofs |
| `nvme` 0.0.0 | "A simple NVMe driver", one release, dormant | 6 / 4,857 | nothing a user depends on today |
| `vroom` (GitHub, not on crates.io) | a user-space Rust driver aiming at SPDK speed, from a thesis, "not production-ready"; not read | not applicable | a full data path with huge pages; no proofs |
| `libnvme` 0.9.0 | bindings to the `libnvme` of Linux, host only | 648 / 788 | a host administration library, not a `no_std` driver |

## Where this crate differs

It is pure logic with no I/O, no allocator, no `unsafe` and no dependency, so it compiles for any
target and a model checker reaches all of it. Ten Kani harnesses prove the ring arithmetic, the
phase discipline, the doorbell map, the PRP pair, the Identify parse, the two register decoders and
the two checks on values the controller hands back. Each harness ships with the one-line patch that
turns it red. The crate checks what it reads from the controller instead of trusting it: a
submission queue head the controller cannot have reached is an error, not an accepted value.

What it costs: a user writes the volatile shell, the DMA allocation, the admin bring-up sequence,
the command tracking and every wait. A transfer longer than two pages has no PRP list to carry it
(P2-1).

## A concrete example

The falsification patch for `record_head` removes the ring-distance check and leaves only the
"inside the ring" check, which is all a trusting driver does. Under it, a controller that reports
a head past the tail frees slots that hold commands it has not read. The harness
`record_head_accepts_only_a_head_the_controller_can_have_reached` then fails at
`assert!(reachable)`, and the review replayed that failure.
