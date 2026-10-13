---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: a-region-reap-scans-every-mapping-on-the-machine-per-page
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 658. A region reap scans every mapping on the machine, once per page

*Title and slug are drafts.*
Raised by the lane for milestone 604 (the builder's scratch cursor is bounded), whose guest test
runs in 3 seconds alone and took 104 inside CI's whole aarch64 suite.

No decision owed to start measuring. The fix itself touches the revocation path
of §13 (frame revocation) and should come back with numbers.

## The finding

`kernel::revoke::revoke_region` is what `MemoryRegion::DESTROY` runs before a region's pages go
back. Its unmap pass loops. It takes the registry lock, scans every live address space's mapping log
from the start until it finds a record of a page in the range, and releases the lock. Then it unmaps
that page everywhere (`unmap_everywhere` scans every live space again) and repeats. So one reap costs roughly the
region's mapped pages times every record on the machine, twice over.

Read from the code, not profiled. The evidence that it matters is the timing above: the same forty
reaps of a 668-page region, alone and behind a suite's worth of live spaces.

## Why it is on the customer path

Every job the progenitor spawns is reaped by destroying its region. A `ripgrep`-sized job has about
670 pages mapped twice (in the child and in the progenitor's scratch window), and the boot servers'
spaces, each carrying the initrd or its own image, are all live while it is reaped. The cost of
ending a command grows with how much else is running.

## What a fix could look like

Options, not a recommendation, because this is the revocation path and §13's use-after-free
guarantee rests on it:

- Scan once per reap rather than once per page: collect every in-range record in one pass, then
  unmap them.
- Index records by physical page, so a reap visits only its own pages' records.

Either should be measured with `script/bench` before and after, since `map_el0` already moves with
this log's layout (`LOG_ENTRIES`' doc comment).

## Index row

Reaping a memory region scans every live mapping on the machine once per page, so a 3-second guest test took 104 seconds inside CI's whole suite. Proposed: measure it, then fix the revocation path of DECISIONS §13 (frame revocation) with numbers.
