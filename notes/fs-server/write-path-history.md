# What is proven, and the write bug that was never a filesystem bug

An appendix to [notes/fs-server.md](../fs-server.md), which keeps the conclusion. This file holds
the three rounds of investigation into a write that seemed to loop, the branch salvaged from it, and
the two hypotheses that died on the way.

## The write path is proven on device

That is a correction (2026-07-29, milestone 27 (Rust `std` on the native ABI) phase two). This note
used to record an open item: the end-to-end write "loops inside RedoxFS's allocator commit on bare
metal even on a pristine image". It was said to spin on the `prev`-chain walk in
`Transaction::sync_allocator`, issuing no further writes until the watchdog fired. It does not.

Driven through `std::fs` (the milestone-27 PAL, `OpenOptions::write(true)` on the image's `scratch`
file), the write completes on both ISAs. It reads back through the server. It reads back byte for
byte when the host tool reopens the image afterwards with the pinned engine, which is the part a cache
cannot fake. That reopen is in the gate. `redoxfs_check_after_run` compares `scratch` against the
fixture, and `mkredoxfs` rewrites it to a placeholder before every run. So the check passing means
this run's guest write landed on the disk.

## Narrowed a third time

This is fix/redoxfs-repeat-write. "A repeat write to the same block loops" is also not quite the
shape of it. What is now proven, with tests in the tree rather than by reasoning:

- A repeat write inside one run works, on both ISAs. The FS client writes the same block three times
  in one run (`fixtures/src/fs_test_client/proof.rs`). It passes on aarch64 and riscv64 against a
  freshly generated image. The image afterwards carries the pass-3 payload, so the third write really
  reached the disk. This is the reproduction the old gate could not perform. It depends on nothing left over
  from a previous invocation, so it cannot hide behind `mkredoxfs` rewriting the target first.
- The host does not reproduce any of it. Four `redoxfs_server` host tests are all green in
  milliseconds. They are three writes to one block; the same through the EL0 binary's exact
  chunking; record-sized repeat writes (the multi-block and compressed-tail paths); and write, drop
  the mount with no unmount, reopen, and write again. That last one is the shape the device fails at, and on
  the host it passes.
- The transport is faithful. `IpcDisk` has a `VERIFY_WRITES` switch that reads every written block
  straight back and compares. It never fired. So no write is lost or misdirected, and no read returns
  stale bytes; the blk IPC path carries what it is given. It is off by default. Its 4 KiB scratch
  sits on the stack inside a call RedoxFS makes from deep recursion. That is enough to overflow the FS
  server's stack and produce a *different* failure than the one being chased. That cost an hour, and
  it is recorded here so the next reader does not pay it again.

## Resolved: there was never a filesystem bug here

For three rounds this note carried an open item saying a second mount of a *used* image fails its
write. The write never failed. The mechanism is the missing TRUNCATE verb, and it is documented
behavior rather than a defect: a write shorter than the file does not truncate it.

So a test that writes N bytes, and then compares a *whole-file* read against those N bytes, passes
only while the file was not already longer. One boot's FS client left a 64-byte payload in `scratch`.
The next boot's `std::fs` test wrote its 61-byte pattern and asserted the whole file equaled it. It
got 64 bytes back (61 new bytes plus the old three-byte tail), and panicked inside its write block.
That panic, read as "the server refused the write", is the whole bug. It explains every observation,
including why three investigations disagreed. The symptom depended on what the previous boot's client
happened to leave behind, and that changed as the client changed.

`redoxfs_server`'s `a_shorter_write_does_not_truncate_and_that_is_what_broke_across_boots` pins the
semantics with those exact byte counts. So the sharp edge is now a test rather than a trap. If it ever
fails, the contract grew a verb, and that is a deliberate decision.

## Salvaged from `fix/redoxfs-write-loop`, including the part that was wrong (2026-07-31)

That branch was the *second* of the three investigations above, and outlived them on a shelf. So its
contents are folded in here and the branch deleted. A branch is not a place to keep findings; nobody
reads branches.

Its conclusion was wrong, and saying so is the point of recording it. It ended at "every in-process
component is correct; the divergence is the real device I/O path (blk IPC + the shared page + QEMU's
virtio-blk)", with a leading hypothesis of an async-completion race. The round after it disproved
that. `IpcDisk`'s `VERIFY_WRITES` reads every written block straight back and never fired, so the
transport carries what it is given. The actual mechanism was the missing TRUNCATE verb plus a
whole-file comparison across boots. Four host eliminations and a block-access-sequence diff all
pointed confidently at the transport, and the transport was innocent. It is worth keeping as a
caution. An investigation that rules out everything it can reach concludes the fault is in what it
cannot reach. That is an argument from the shape of the tooling rather than from evidence.

One finding survived the salvage, was recorded as open, and was already closed. The salvaged text
said: *the block server cannot use interrupt-driven completion, and nobody knows why. Switching
completion from polling the used ring to waiting on the interrupt hangs on the first read; the
completion interrupt never reaches the block server, even though the shadow avail ring leaves
interrupts enabled. So the driver polls, and that is a workaround for an unexplained fault rather than
a choice.*

None of that is true of this tree, and it stopped being true two days before the salvage.
`fix/irq-delivery` (2026-07-29, commit `dd8f186`, "block server: wait on the completion interrupt, do
not poll the used ring") replaced the poll with a `WAIT` on the `Irq` endpoint. That is what
`crates/virtio`'s `complete_block` does on `main` today. The correction is written up in
[block-server.md](block-server.md). The salvage folded in a branch that predated the fix, and did not
reconcile it against the note it was being folded into. So one file ended up asserting both a defect
and its repair, seventy lines apart. That is a hazard of salvaging: a branch's findings carry the date
they were found, not the date they were filed, and the tree may have moved.

The RISC-V interrupt-delivery tests of milestone 19 (run a real workload) were pointed at this as a
diagnostic, and they put the fault below the kernel rather than in it. The question posed was: if
IRQ-to-message delivery does not work on RISC-V, then polling had been masking something serious on
the ISA whose hardware arrives in three weeks. It works.
`kernel::sched::tests::an_interrupt_becomes_a_message` and
`an_interrupt_that_arrives_before_the_wait_is_not_lost` now run on RISC-V (see notes/interrupts.md).
They prove the PLIC claim / route / mask / notify / complete path, and the pending-signal count that
closes the lost-wakeup window, both directly and with no device driver in the way. Each was proved
capable of failing. Dropping `irq_notify` from `riscv_trap_dispatch`'s external-interrupt arm turns
the first red. Dropping the `pending` increment from `Endpoint::signal` turns the second red while
leaving the first green. Above them, the whole riscv `riscv_virtio_tests` module runs a real
userspace driver whose completions are interrupts, on both the mmio and PCIe transports.

So there is no open kernel-side interrupt-delivery defect to inherit. Milestone 53 (the board's own
peripherals: network and storage on real silicon) should expect the interrupt path to work. If a real
storage driver on real silicon hangs waiting for a completion, the place to look is the device's own
interrupt configuration, not the kernel's routing.

Also ruled out by the write-loop investigation, and worth not repeating: a bounce buffer. The device
DMAs only into a private buffer, and the block server copies to and from the shared page after
completion. So the device never touches the shared page, and the arrangement is correct by
construction for any aliasing. It still looped. Given what the next round found, that is exactly what
it should have done, since there was no aliasing bug to fix.

## Two hypotheses died on the way

Both are worth recording as dead. Neither was the cause, and a disproved guess left standing sends
the next reader down a road already walked.

Heap exhaustion and accumulated mount state: dead, measured. The hypothesis was that a used image
carries a higher header generation, a longer allocator log and more live tree blocks. So the second
mount would drive the FS server past its 8 MiB cap (`HEAP_MAX` in `redoxfs_server.rs`, matched by
`FS_BUDGET_PAGES` in `kernel/src/user.rs`). It does not. `redoxfs_server/src/bin/second_mount.rs`
runs the real engine under the same allocator the FS server uses: `user_mode_heap`, the algorithm
behind `user_mode_runtime::heap::UntypedHeap`, grown incrementally and capped identically. The image
sits in a `static`, so it stays off the heap exactly as a real disk does. At the device's own 8 MiB
cap it completes 30 mount-and-write cycles, every one fine. Heap high-water is flat at 352 KiB, and
the cap never once refuses a growth. That is four percent of the budget, and thirty generations of
accumulation move it nowhere. So raising the budget would have fixed nothing. Any number picked to
make a test pass would have been a coincidence rather than an argument. The dials are deliberate
(`NIFE_HEAP_MIB`, `NIFE_MOUNTS`), so this is re-runnable.

A device-only cause: dead too. Once the heap was ruled out, the remaining reading was that something
existing only on device was at fault. `NIFE_KEEP_REDOXFS=1` makes the second-boot case deliberate:
run the suite, then run it again, and every mount is a mount of an image a previous boot wrote. With
the client's payloads corrected to one length, both ISA legs pass it completely: aarch64 150 tests,
riscv64 95, including `std_fs` and the FS client's three repeat writes. The device was never the
problem either.

## The plumbing that should have caught it in round one

It now exists. The client used to route every failed reply through `check`, which panics. So a
trapped client told the waiting test that something went wrong, and the server's reason died with the
process. A negative reply is now SENT instead. `w0` carries the raw reply word, and `w1` carries
`0xBADD_0000 | stage << 12 | errno`, with a stage tag for which request was refused. The kernel test
prints the word it compared against `SUCCESS`.

The raw word rides alongside the decoded errno on purpose. The wire's negated errnos overlap the
kernel's own `invoke` errors at -1..-8 (the reply-space wart in notes/std/fs.md). So a small value is
ambiguous between "the server returned this errno" and "the IPC itself failed". Carrying both makes
the ambiguity visible, instead of quietly resolving it the wrong way.

The transferable lesson is the one DECISIONS §27 (the filesystem service) already draws about
order-coupled gates, with a second edge. A fixture that one test *mutates* and another *asserts on*
couples them just as tightly as leg order does. The coupling is harder to see, because both tests
look self-contained. The client now restores the fixture pattern as its last write for exactly this
reason.

That gap is closed (milestone 31 (a capability shell) phase 2). `CREATE` and `TRUNCATE` are in the
contract, so `std::fs::write` and `File::create` work. See the parent's write-path section for the
semantics, and §27's amendment for why `TRUNCATE` was a sharp edge and not merely a missing feature.
