# The system tests' progenitor-shaped spawn

`spawn_progenitor_shaped` in `system_tests/src/user/test_programs.rs` starts a program the way the
kernel starts the progenitor. Milestone 794 (one progenitor-shaped spawn for the system tests) built
it. This note keeps why its space is hand-built, which spawn sites do not use it, and why the bind
it makes is the spawner's.

## Why the shape is the progenitor's

A harness that spawns this way proves the program's own choices rather than a privileged shortcut.
The endowment is the one `spawn_elf_child_builder` gives: the initial ramdisk mapped read-only, the
budget's region capability in slot 0, a report endpoint in slot 1, and nothing privileged.

Every site that uses it shares the space budget's `log_pages_for(initial_ramdisk_pages)` term, so a
change to that accounting is one edit rather than one per site.

## Why the space is hand-built

`crate::user::run` cannot serve these callers. Its extra mapped pages arrive as a slice, and the
archive is a few thousand pages the suite has no allocator to build a slice from.

## The sites that do not use it

- The kernel's own `spawn_elf_child_builder` is not folded in. It also measures the program and
  routes the boot interrupts, which no test harness wants.
- `timetable_tests::spawn_timetable_with` stays whole. Its slots are its own four-slot contract
  (output, budget, child report, deaths). Its archive is a narrowed copy it builds, not the initial
  ramdisk, and its stack, extra mappings and start arguments are its own. It shares
  `segment_pages` and `lay_out_program` and nothing else. A helper parameterized to cover the rest
  would be `spawn_timetable_with` under this one's name.

## The bound notification

`bound_notification` is bound between configure and start because that is the only order without a
race. A program that reached a wait before its notification was bound would park outside the
bind-delivers-what-was-counted rule. The bind is the spawner's to make, because a running thread
holds no capability to its own TCB. `net_stack`'s wiring makes the same bind for its retransmit
wait.
