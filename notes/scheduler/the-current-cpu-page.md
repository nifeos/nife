# The current-CPU page and its tests

An appendix to [notes/scheduler.md](../scheduler.md). A userspace thread learns which core it is on
by loading from a page, with no syscall. [The switch](switch.md) says where the scheduler writes it.
This page holds what the kernel tests in `system_tests/src/user/current_cpu_tests.rs` prove about
it, and why each assertion has its shape.

*Name: provisional, for the stem `the-current-cpu-page`, after the page it describes. calef names
files.*

## The ruling it proves

calef ruled on 2026-09-21 (UTC) that a thread observing itself reads a page. No section in
`design/decisions/` records that ruling, so the tests name it rather than cite it. The end-to-end
test follows the whole chain. `crates/current_cpu_contract` lays the page out, `AddressSpace::new`
maps it read-only, `schedule()` writes the core at switch-in, and `user_mode_runtime::current_cpu`
reads it.

## Membership, not a number

The test asserts that each read names a core in the online set. A test that pinned the answer to 0
would pass on QEMU `virt` forever and say nothing, because the scheduler may put the thread
anywhere. Membership catches a failure this tree has already paid for. The VisionFive 2's online set
is `{1, 2, 3}`, and treating a count as an index once put `init` into a parked core's inbox
(`crates/cpu_set`). A userspace consumer that indexes an array by this value inherits that hazard,
so the value must be a real id. The same test carries back the id bound userspace sizes its arrays
by, the one number the two binaries must agree on, which a host test cannot compare.

## Unknown is not zero

A thread cannot run an instruction before it is switched in, so from inside a thread the page has
always been written. The unset state is visible only from the kernel's side of the frame, before
anything has run in the space, and the second test reads it there. Zero would be a plausible answer
and a wrong one. That is why the page carries a magic and a sentinel rather than relying on a zeroed
frame. calef ruled the same day, about the counter frequency, that a wrong number is worse than no
number.

## The frame comes back

The page is the one frame an address space owns that its region does not pay for.
`memory_region::destroy` does not cover it, so `Drop` frees it by hand, and that is where a leak
would hide. The third test asks the allocator about that frame's own bit after the drop. A
neighbour freeing frames cannot move that bit, and a leak still fails it. Bracketing the machine's
free total would instead assert that nothing else allocated or freed anything during the window,
which this test cannot promise. [The `free_page_frames` sweep](../load-sensitive-assertions/free-page-frames-sweep.md)
generalizes this fix. `memory_region::usage` cannot scope the check, because the frame belongs to no
region.
