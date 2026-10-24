# The address-space tests, in detail

An appendix to [notes/system-test-modules.md](../system-test-modules.md). The doc comments in
`system_tests/src/user/address_space_tests.rs` say what each test proves. This page holds why each
is shaped the way it is.

## A test that cannot fail for its stated reason

The tagging test installs space A, reads a virtual address, installs space B and reads the same
address again. B must see its own byte. That only proves tagging if the root switch flushes
nothing. A root switch that flushed the TLB would make B read B's byte because everything had just
been thrown away, and the test would pass whether tagging worked or not. A test that cannot fail for
its stated reason is worse than no test.

That is why the test needs the ASID shootdown underneath it on every architecture it runs on. On
aarch64 and RISC-V the context switch flushes nothing, so the assertion means the same thing on both.
x86_64 runs with `CR4.PCIDE` clear, so every `mov cr3` flushes the whole TLB, and the test skips
there rather than pass for the wrong reason.

## The cross-core shootdown

The tagging test proves that two spaces stay apart on one core. The shootdown test proves the other
half of the contract. When `crates/address_space_identifier` says "flush, then the number may tag
someone else", the flush has to have reached every core that could hold an entry wearing that
number. On aarch64 that is one broadcast instruction. On RISC-V it is a distributed protocol: SBI
RFENCE to every other hart, and a wait for each.

The sequence has four steps:

1. A probe thread on another core installs the space and reads `VA`. That pulls the translation
   into that core's TLB, tagged with the space's ASID, and leaves the space installed.
2. This core moves `VA` onto a different frame with no per-address invalidation at all, so the only
   thing that can announce the change anywhere is the ASID sweep.
3. This core calls `flush_asid`.
4. The probe reads `VA` again, on the same core, with the same space still installed.

Step 4 must see the new frame. If the sweep stayed local, the probe sees the old byte, and that byte
is the bug: a core still translating an address space that no longer means what it did.

Two choices keep the measurement clean. The space is never re-installed between the reads, because
a `satp` or `TTBR0` write is a second event that an emulator or a core may treat as a flush. And the
mapping is changed rather than the ASID recycled, because a recycled number would likely get the
dead space's frames and read the right byte by accident. The remap is discharged with a no-op flush
on purpose: the real per-address flush broadcasts too, and would make the test pass whatever
`flush_asid` did.

The probe masks interrupts for its whole body. A preemption would migrate it and split its two
reads across cores, and would swap the space out from under them. Masking does not block the
shootdown: RISC-V's RFENCE arrives as an M-mode software interrupt, which S-mode masking does not
touch, and aarch64's `tlbi` needs no interrupt at all.

`spawn_on` is a hint, so the probe records where it ran and the test asserts only that it was not
this core. It cannot land here, because this thread spins without yielding, so its core never goes
idle and never steals. If the remote core never services the request at all, the probe's
deadline-bounded spin reports a timeout, and the failure text then names the handshake rather than
the shootdown one level down.

## A space installed across a timer tick

`translate_user` walks the root the CPU holds now. A kernel thread has no space of its own, so a
context switch back to it installs the kernel's reserved root. A preemption between
`while_installed`'s install and the body's walk therefore takes the space away under the body. It
needs a tick inside a window a few microseconds wide and another runnable thread on the same core.
A one-core machine where an earlier test leaves a client spinning has that thread. Without
interrupts held off inside `while_installed`, this test fails on every run on all three
architectures.

The test makes both conditions certain. A competitor never yields, so once it shares a core with the
test thread it stays queued there, and a tick in the window switches to it. The body spans five tick
periods before it walks.

The precondition is read inside the window, from this core's run queue, not inferred from where a
competitor once ran. Placement is a hint, and a steal breaks it: §28 (SMP placement) lets an idle
core pull a queued thread from a loaded one. While competitors are spawned in core order, a core
whose own competitor has not arrived yet is idle, and it can steal the one queued on the test's
core. Once every core holds a spinner no core is idle again, so nothing steals one back. The setup
therefore repairs the placement: an empty queue here is answered with a competitor spawned onto this
core. Each steal makes an idle core busy for good, so at most one extra competitor per core is ever
needed.

## Exact frame accounting

The whole-space free test counts frames exactly, because an approximate count would hide a leak.
Three things keep an exact count honest.

The subjects run on the test's own core. A thread's frames are freed by `finish_switch` on whichever
core reaps it, after it leaves the thread table. On another core that free is asynchronous, and
`used()` never reads exact.

The baseline is taken only once `used()` stops moving. A free that lands after the baseline makes
`used()` settle below it, which reads as a negative leak, a count no real leak can produce.

The final check is `<=` against a waited-for value, and the message prints the count the wait
decided on. Another test's teardown may free frames inside the window, which four subject spaces are
not responsible for. A fresh sample after a timeout could print zero or a negative count and send a
reader into an investigation of nothing. The general rule is in
[notes/load-sensitive-assertions.md](../load-sensitive-assertions.md).
