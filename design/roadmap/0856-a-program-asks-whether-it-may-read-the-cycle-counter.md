---
status: NOT-STARTED
raised: 2026-10-07
promoted_from: a-program-asks-whether-it-may-read-the-cycle-counter
milestone_dependencies: 353, 75
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 856. A program asks whether it may read the cycle counter

*(The title and slug are provisional.)*

It follows from the block of milestone 353 (the aarch64 half of 74).

**In brief.** calef ruled decision B of milestone 353 as B4 in two steps on 2026-10-07 (UTC). Step
1 is built: `user_mode_runtime::cycle_reading()` returns a count paired with an
`abi::cycle_counter::CycleMeaning`. Step 2 is this block: a kernel-provided page or method that
lets a program ask, before it reads, whether it may, so an ungranted read can fall back instead of
being killed. calef called it a later syscall-surface fork, so it is written down and not built.

It carries a second fact for the same reason. The ruling asked that a riscv64 reading be flagged
when the kernel's own probe was handed `hpmcounter3` instead of `mcycle` (QEMU's `rva23s64` model
does this). The kernel knows which counter firmware gave it (`arch::riscv64::pmu`'s `CYCLE_CSR`);
a process has no way to learn it, because nothing carries a kernel fact to a process except a page
or a call. Whatever answers "may I read" can answer "which counter is the kernel's" in the same
place, which is why the two are one proposal.

What has to be decided before building, and it is an architect's:

- Page or method. Linux answers with a page: `perf_event_mmap_page` carries `cap_user_rdpmc` and
  `pmc_width`. This tree already maps a read-only timebase page into every x86_64 and riscv64
  process (`crates/counter_frequency_protocol`), which a field could extend on those two, but not on
  aarch64, which maps none. A method is a new number on the syscall surface.
- Whether milestone 75 (who may read the cycle counter, and by what authority) lands first. Today
  no manifest field grants the counter and no method sets the grant, so the only granted thread is
  the kernel test's. Asking "may I" matters once a real program can be granted.

Reuse: `counter_frequency_protocol`'s page and its parent-fills-child propagation through
`supervision_protocol::build_child_space` were considered as the carrier and are the cheapest
route for riscv64 and x86_64; the choice is the architect's because it widens an ambient page.

## Index row

A program asks whether it may read the cycle counter, and learns which counter the kernel's own probe reads, before an ungranted read kills it. It is step 2 of milestone 353 (the aarch64 half of 74)'s decision B, and page versus method is the architect's call.
