# Decision 1: the identity, with the line count inside it

An appendix to [the milestone](../1099-machine-statistics-contract-outside-review.md). The ruling in force is on the main page.

Background. Three programs under `components/src/` declare this interface by the pair (`INTERFACE_ID`, `VERSION`) in their `INTERFACES` note. The kernel and the spawn service compile in the fixed address. Every reader compares `MAGIC` and the version word. The kernel and every reader compute the layout from `word::LINE` and `CPU_ID_BOUND`. Once a pin outside nife builds one of those programs, a change to any of these values has to move every pin in one image. Milestone 1017 (the contract crates leave in this order) measured that cost from two reverts. The identity
set here has one member the sibling page crates do not have: the number of cpu lines, which today
is another crate's constant (item 6). A kernel and a reader that agree on every identity value can
still disagree on the layout if that constant moves between their pins.

Recommendation first: freeze the identity as it stands, and make the line count this crate's own.
`MAGIC` is `MACHSTA1`, `VERSION` is 1, `INTERFACE_ID` is 4, and `PAGE_VIRTUAL_ADDRESS` is `0x005F_F000` (a pair page on the address-space map). A line is eight words. There are eight cpu lines after the header line, and the content is 576 bytes. The header holds the magic at word 0, the frame size at 1, the tick rate at 2, the total and free frame counts at 3 and 4 and the version at 5. Words 6 and 7 of every line stay reserved and read zero. A later writer may assign one under the same version only if a reader that does not know it is still right to read zero there. Any other change bumps `VERSION` and re-mints the magic's digit in one change. Every value is already compiled into the kernel, the spawn service and three programs, and no item above needs any of them to change. The line count becomes a constant of this crate with a compile-time check against `current_cpu_contract::CPU_ID_BOUND`. The two cannot drift inside one image, and a change to the bound is a deliberate version decision here.

Prior art. Linux's vDSO data page always matches the kernel that ships it, so its layout never has to be versioned across a boundary. FreeBSD's `vdso_timekeep` carries `tk_ver` and OpenBSD's `timekeep` page a version constant that libc compares. A mismatch means no page rather than a misread one (both unverified in detail). Linux `/proc/stat` grows by appending columns and readers ignore columns they do not know, which is the growth rule recommended for the reserved words. Fuchsia's `zx_info_cpu_stats_t` is a struct per topic whose size the call checks (unverified in
detail). `block_roster_contract`, the contract pilot, froze its identity as a set before it left.

Concrete example. `components/src/free.rs` declares
`(machine_statistics_contract::INTERFACE_ID, machine_statistics_contract::VERSION)`. Suppose a
release of `current_cpu_contract` raised `CPU_ID_BOUND` to 16 and the kernel took it while `free`'s
pin did not. Both still declare `(4, 1)`, the magic and version match, and `free` reads eight of
sixteen lines: its `Mem:` line is right and its cpu totals are silently half. With the line count
frozen here, the kernel's build of this crate fails at the assertion instead, and the change
arrives as `VERSION` 2 with `MACHSTA2`, which `free` refuses until its pin moves.

The alternative, left open, is to carry the line count in a header word (word 6) so a reader
learns it at run time. It costs a layout change now, with the version bump that implies, for a
flexibility no reader in the tree needs; the review does not recommend it.
