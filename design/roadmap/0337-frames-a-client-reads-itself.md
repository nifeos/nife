---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 337. Nobody has drawn the capability-shaped way past one IPC round trip per file request

Premise re-checked 2026-09-19 and it holds. No `design/decisions/` entry draws this design: the only sections naming a page cache or
`mmap` are §34 (RedoxFS primary) and §86 (whether an NVMe driver can leave the kernel), neither of which is about granting a
client readable frames. Milestone 138's block still describes the ~13 us residual in prose at four
sites, which is what "a frontier described in prose" means.

Drawing the design is a lane's work and can start today: frames are already
capabilities, the residual is already measured, and the alternatives milestone 138 refused are
already written down with their reasons. What the design *concludes* is a change to a contract two
programs agree on, so it lands as a `design/decisions/` entry for calef rather than as code.

**In brief.** After milestone 138's three steps, a file request costs about 13 microseconds of
residual that no cache removes, because the shape of the contract is one IPC round trip per
request. The capability-shaped answer is to grant the client frames it can read directly, which is
what `mmap` over a page cache buys Linux. The primitive exists here already: frames are
capabilities, they are shared rather than moved, and rights narrow at send. Nobody has drawn the
design, so the residual is currently a frontier described in prose.

## Why this matters

It is the last term. Milestone 138 removed the 208 microsecond fixed cost with a metadata cache and
raised the transfer size to 64 KiB, and what remains is not a slow implementation of the current
contract, it is the current contract. No further tuning reaches it. That makes this the difference
between a file service that is fast for a microkernel and one that is fast, which is the
comparison this project exists to make.

It is also the interesting half from a demonstrator's point of view. Linux gets `mmap` performance
by handing a process a mapping into a global page cache, and the confinement questions that raises
are answered by the ambient authority model. Here the same performance would come from a
capability that says exactly which bytes, with exactly which rights, revocable. If that works, it
is a result about capability systems and not only about this file server. If it does not work, the
reason is worth writing down, because it is a claim about the model.

The questions the design has to answer are the ones milestone 138 (close the read gap) refused a data cache over. They
are coherency between a client reading frames and a server writing them, what a client observes when
the file changes underneath it, and what revocation means mid-read. The metadata cache had none of
those because it lives inside one server's address space. This has all of them, which is why it is
a design and not an optimization.

## Constraints this design inherits

Milestone 138 refused these. Replacing RedoxFS was ruled out
on measurement: 94% of the old fixed term was the absence of a cache, not a property of the store,
and any replacement arrives needing the identical cache. A `READV`-shaped scatter list and a
negotiated channel size were both declined because every existing agreement between a client and
this wiring is a compile-time constant both sides carry. A new concept on the wire has to earn
its way past that.

## Index row

After milestone 138's three steps, a file request costs about 13 microseconds of residual that no
cache removes, because the contract is one IPC round trip per request. The capability-shaped answer
is to grant the client frames it can read directly, as `mmap` over a page cache does for Linux. The
primitive exists, since frames are capabilities, shared rather than moved, with rights narrowed at
send. Nobody has drawn the design. Linux buys that speed with ambient authority over a global page
cache. Here a capability would name exactly which bytes with which rights, revocably. The design
inherits the questions 138 refused a data cache over: coherency, what a client sees when the file
changes underneath it, and what revocation means mid-read.
