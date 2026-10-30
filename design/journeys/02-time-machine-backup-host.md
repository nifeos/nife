---
status: RETIRED
raised: 2026-08-26
retired: 2026-08-30
---
# 2. Run a Time Machine backup host

The customer this journey served stopped using Time Machine. The
family's backups are now **borg over SSH on cordoba**, with Immich for images, both built with the
existing Linux ecosystem while nife was not ready; SMB is no longer in that path either. calef,
2026-08-30: *"Journey 2 is dead so put a fork in it."*

Milestone 54's adapter, milestone 55's Time Machine half and the shares they served left the tree on
2026-08-30. **notes/smb.md is the record of what they did**, and it is the place to start for anyone
reviving this.

**This file is kept rather than deleted, because the steps below are still true statements about
what was built** and because a retired journey is evidence about the project rather than clutter: it
is the worked example of principle 1's own warning, a customer with a real deadline going elsewhere
because the work did not land in time. Milestones 54 and 55 are now `REMOVED`, and 65, 107, 53 and
131 keep their own statuses; all of them are repriced in their own blocks, not here. Nothing below
should be read as current intent.

calef, 2026-08-26: a second journey, running a Time Machine backup host. Unlike journey 1, this
story is not waiting on unwritten code for its first light: milestone 55's own 2026-08-22 scoping
pass says outright, *"there is no fork left to bring him."* Every remaining step below is hardware
contact, not design or implementation.

| step | milestone | decision | what this step needs |
|---|---|---|---|
| 1 | 54 | | the mountable-share core: real Mac's `mount_smbfs` already mounts a share served by nife |
| 2 | 65 | | NTLM secrets: the key `ntlm_response` computes with |
| 3 | 107 | | sockets that accept: what lets a Mac connect at all |
| 4 | 55 | | Time Machine itself: SMB3 subset, mDNS discovery, the `AAPL` create context and TM flag, durability witnessed via real `VIRTIO_BLK_T_FLUSH`, throughput measured. Software-complete; what remains is a real NIC driver on real silicon, multicast proven on a real network segment (slirp cannot carry it), and an actual backup plus power-cut test |
| 5 | 53 | | board peripherals: the JH7110's GMAC driver on the VisionFive 2, the actual blocker for step 4's real NIC. Board on the desk since 2026-08-14; recommended first over aarch64 hardware because it is the one already in hand |

Steps 1 through 3 are built and are the foundation the rest stands on, not steps still in progress.
Steps 4 and 5 are coupled: 55 names the real gap and 53 is where it is closed, in that order because
55's own text frames riscv64-first as a reversible sequencing call, not a fork.

**Adjacent, not on this journey's critical path.** Milestone 131 ("configure a service instead of compiling it, and type its secret at a console") is explicitly named in 55's own text as something
first real Mac contact should happen *before*, against the existing guest share, not wait on.
Milestone 163 ("the JH7110's PCIe root complex: a real driver for the PLDA XpressRICH controller")
is needed for the *full* persistent-storage validation a production host would want, but 55's text
notes a narrower first experiment (mount, small write, clean unmount) only needs step 5, not 163.
Neither is a step above because neither blocks the journey's first real backup.

**DECISIONS §129 (RENAME's `NOREPLACE` flag) does not gate this journey.** Checked directly:
55's own text says "nothing ties `ReplaceIfExists` to a confirmed Time Machine operation today," so
§129's "build it when we have a customer" holds exactly as decided; a real backup running is what
would make it load-bearing, and this journey is what would surface that if it happens.

## What "done" looks like

Every step above at `BUILT`, ending in step 5: a real NIC driver on real silicon, carrying a real
Time Machine backup through to completion and surviving a power-cut test, this project's own
durability standard.
