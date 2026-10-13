# Milestone 447 appendix: the five tests, and why they are kernel threads

An appendix to [milestone 447 (a thread's vector registers are its
own)](../0447-vector-registers-are-a-thread-s-own.md). The block is complete without it: its [proof
summary](../0447-vector-registers-are-a-thread-s-own.md#the-proof-and-it-fails-against-a-kernel-without-this)
names the tests and the falsification. This holds what each test catches.

## The five tests

Five `#[test_case]`s in `kernel/src/fp.rs`, running on all three architectures.

- `two_threads_doing_vector_work_do_not_see_each_others_registers` is the milestone. Two threads
  fill the whole register file with distinct patterns, yield two hundred times each, and check
  their own values after every turn. They are placed with `spawn_on(cpu::id(), ..)` and not with
  `spawn`. §28 (SMP placement: two random choices at spawn) would put them on two cores with two
  register files, where the test would pass without the kernel doing anything. On one core they
  interleave over one file, with this thread (which never touches FP) between them. So the
  scrub-and-disable arm runs between every pair of turns as well.

  Falsified on purpose: with `save` removed from `hand_over` and nothing else changed, it fails
  with "a thread found another thread's values in its own vector registers". Run on aarch64 before
  the other two architectures existed.
- `a_thread_with_no_vector_state_finds_the_registers_scrubbed` drives the fourth row of the
  block's table directly. Two threads both using FP catch a missing *save* loudly. A thread that
  stops using FP while its values sit in a file nobody scrubs fails silently, forever, and is what
  CVE-2018-3665 was. It reads the hardware back through a helper that deliberately skips the
  enable trap, because that trap installs the initial state itself and would prove nothing.
- `the_whole_register_file_survives_a_save_and_a_restore` checks every register by name. A save
  that copies the right bytes to the wrong lane passes the concurrency test whenever both threads
  are preempted at matching offsets; the per-register pattern fails here.
- `the_first_floating_point_instruction_takes_a_trap` is why the RISC-V boot-path bug was found
  rather than shipped ([`riscv.md`](riscv.md) has the account).
- `two_threads_that_never_used_the_unit_leave_it_shut` asserts the case every switch in this
  tree actually takes. If it stopped being true the cost would be a kilobyte of memory traffic per
  switch, and the only symptom would be a slower benchmark.

## Why the tests are kernel threads

The tests are kernel threads, not user programs, and that is forced rather than chosen. Every
target in `targets/` is soft-float, so no userspace binary in this tree can execute an FP
instruction to be tested with. That is why all three first-use handlers serve the kernel's own
exception level as well as userspace. Refusing EL1 would have meant a mechanism whose only possible
test was a different mechanism. The C seam of §31 (the foreign-language seam: C holds no
capabilities and makes no syscalls) is untouched for the same reason. It is the first thing that
changes if the target flip is taken.
