# The system-test modules: what each proves, and why it is shaped that way

*Name: provisional (milestone 929 (comments in the Linux style across first-party Rust), lane
`comments/929-system_tests-01`). calef names notes.*

`system_tests/src/user.rs` declares every module of the whole-system suite, and each declaration's
doc comment says in a few lines what the module proves. This note holds the reasoning those doc
comments link to: why a test is shaped the way it is, and what it would miss if it were shaped any
other way. The appendices in [`system-test-modules/`](system-test-modules/README.md) do the same
for single test files whose doc comments carried more design than a doc comment should.

How the image is built and why the suite left the kernel crate is in
[notes/system-tests-and-the-kernel-crate.md](system-tests-and-the-kernel-crate.md).

## The thread leak police

`thread_leak_police` asserts that no test left a runnable thread behind. A one-shot driver that
spins forever instead of exiting stays runnable for the rest of the boot. Enough of them on one
core starve a later heavy test past the 60 second hang watchdog, which is how the RedoxFS mount once
overran the watchdog under the net boot. The probe first yields, which lets a just-finished thread
be reaped at the next context switch. Then it asserts that only the idle threads and the probe
itself are runnable. A leak fails on the turf of the test that leaked, with the offending thread in
the dump, rather than as a watchdog trip several tests later.

Tests run in link order, which is alphabetical by module path. So the module's position in
`user.rs` buys nothing, and its name is what makes it run late: it sorts after `tests`, the module
whose whole subject is user threads. The name is the tree's own word for it, from
[notes/riscv-parity-scope.md](riscv-parity-scope.md).

`custom_test_frameworks` has no "run me last" attribute, and nothing enforces the order. A module
whose name sorts after `thread_leak_police` can leak unpoliced, and the symptom would again be a
starvation watchdog somewhere unrelated. The doc comment's `BUGS` names the modules that do.

## Capabilities between processes

The delegation fixtures exist because a capability minted only by the kernel at spawn makes the
kernel a central authority-granting oracle. That is the ambient-authority shape §10 (process
model: capability-based, microkernel) argued against, relocated. A capability system's defining move is that a
process passes authority it holds to another, narrowing it on the way, and only if it was trusted
to (`GRANT`). The granter and receiver are the smallest scenario that exercises all three.

The `PageFrame` producer and consumer apply delegation to memory. The two processes compose the
sharing themselves; the kernel neither copies nor pre-arranges anything.

## Supervision, reaping and process views

`reap_tests` drives every reap through `syscall::invoke`, never through `sched` directly. A direct
call would prove the helper rather than the boundary. The supervisor's table holds endpoint
capabilities and nothing else, because that is what a supervisor's authority is. A test that only
showed the corpse gone would pass a reap that quietly handed the pages to the reaper, which is why
the accounting check exists.

`authority_tests`' restart test is all decisions in an unprivileged process that holds no memory;
the kernel's whole part is one message per death. The progenitor deleted its untyped before the
crash, and the companion test confirms it can no longer use it, so the evidence that it did not
build the replacement is authority, not scheduling order. The same test is where the real tree
proves restart by label (milestone 105 (the two forks milestone 22 named and left)).
`sub_server_supervisor` has two children on one endpoint,
identical programs told nothing but an attempt number; one crashes, one finishes, and it restarts
by label alone. That check shares the test because every `run_tree` leaves a parked tree of about
1,100 frames for the rest of the suite, and a third tree would put the suite over
`SUITE_PAGE_FRAME_BUDGET` (measured in CI, 2026-10-05 UTC). Dropping the label fails inside
`run_tree` at stage 23, not at an assertion, so the kernel claim's falsification lives in
`supervision_tests::a_supervisor_tells_two_dead_children_apart_by_label`.

`child_thread_tests`' reclaim test never probes the TCB region with `reclaim_region` while its
child is live. A refused reclaim is not passive: it arms the kill on every live thread in the
region, so the owner's retry can tear a runaway down. A killed thread becomes a corpse at its next
preemption, so a probe there would reap the child before its `SEND` and hang the test's receive.
A call-free delay in front of the child's `SEND` makes that hang certain on both ISAs, which is
how the constraint was confirmed. Its process-composition test sets one bit per verb the kernel
allowed a two-capability process, and `CHILD_ANSWERED` is the one no other test asserts from
userspace.

`survey_tests` drives the walk with `ps::collect`, the loop `components/src/ps.rs` runs. A bug in the
cursor protocol therefore cannot hide in the gap between the kernel's half and the program's. The
children are real supervised children out of a real region, so the domain is one the kernel built
rather than one a helper described. In the negative control, a viewer of a domain it was not
granted is refused rather than shown an empty list, and an empty domain answers rather than
refusing. Neither claim means anything without the other, so one test asserts both.
`pgrep`'s selector arrives in a register, which is why the prompt cannot spell one. Upstream
`pgrep` prints nothing for a refusal, an empty domain and a selector that matched nothing alike;
here the three answers are distinct.

## The shell, end to end

The scripted shells are the real shell binary, so the builtins under test are the builtins at the
prompt. `pipeline_service` serves the terminal's `OPERATION_WRITE` and collects every byte the
shell prints, so a pipeline that ran but printed the wrong thing fails. Its second thread speaks the
progenitor's spawn protocol because the shell cannot tell the difference, and neither should the
test.

`rm_program_tests` takes its option bits from `grant_plan::rmopt`, so the numbers come from the
manifest the prompt checks against rather than from a second copy of an ordering. The claim is not
that a loop can delete a tree; it is that the walk stops where the capabilities stop.

`redirection_tests` asserts only the shape "same producer, two destinations, the same bytes". A `>`
that dropped every second byte would still produce a file, and a `wc` that agreed with it would
still print three numbers, so any weaker assertion would pass a broken redirection.

`language_tests` asserts about the same shell run as `redirection_tests`. Each scripted shell is a
live process whose frames nothing reclaims, and one more of them put `time_tests` over the frame
pool intermittently. The redirection tests are about where bytes go; the language tests are about
what a word is and what a status means.

## Small programs given one manifest field

`printenv_tests`' unassembled page is the zeroed frame the allocator hands out, the same shape as
`date_tests`' unpublished clock. Its no-capability case is the property `date`'s clock probe also
proves: a program answers without touching memory it does not hold.

`uuid_tests` proves only the empty-slot direction in the guest. `Spawn::grants` fills a capability
table from zero and cannot place a capability at the slot a manifest names, so the endowed
direction is proven at the real prompt, through the real `crates/system_initializer`, by
`script/swish-check`.

## Somebody else's C

Fatal risk 1 is "only software written for nife runs on nife". `ripgrep` (milestone 121 (`ripgrep`
on nife: enumeration as a capability, and what the walk costs)) answered it in Rust, through `std`.
`c_program_tests` answers it in C, through a C library neither program has heard of:

- SQLite 3.50.4's `speedtest1`: 266,378 lines of C written for POSIX (the amalgamation and the
  benchmark), which basalt builds from both files pinned by SHA-256 (milestone 897 (SQLite is
  packaged in basalt)).
- ioping 1.3: one file, a storage-latency tool, the one milestone 834 (ioping on nife and Linux)
  runs on silicon, built by basalt the same way (milestone 906 (ioping is packaged in basalt)).

`cargo xtask basalt-fetch` takes each package by the digest basalt's recipe records at the commit
`xtask/basalt-pin` names. Neither source is touched. What differs from a Linux build is on the
command line: the target flags, nife's headers, and for SQLite two compile-time options it
documents. CI's `rg-threads` rows run the fetch, which is why only they require the programs.

`c_thread_tests` runs hackbench (milestone 907 (hackbench is packaged in basalt)) in thread mode.
Its threads pass 100-byte messages, after every thread has said it is ready through one pipe and
been started through another with `poll`. Basalt's `recipes/hackbench.toml` says what its build
changes and why. The threads are those of milestone 812 (`std::thread::spawn` runs real threads in
one address space), joined to one process, so the process's end is checked to reap every one. The
group sizes keep each run inside a process's 16 threads (`std`'s limit, in milestone 812's `BUGS`);
hackbench's own default, 10 groups of 40, is 400.

## Transports and devices

`block_driver_tests`' PCIe test proves the transport on two interrupt controllers, the GIC and the
local APIC, where `riscv_virtio_tests` proves it on the PLIC board. On `x86_64` it is the whole of
milestone 215 (a PCI function's interrupt reaches nothing on `x86_64`)'s claim, not one more
transport.
