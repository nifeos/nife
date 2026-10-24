# Two seams only the test kernel has

*Name: provisional, minted 2026-10-23 (UTC) by comments/929-kernel-capabilities.*

`kernel/src/delegation_pause.rs` and `kernel/src/retype_fault.rs` let a system test force an
interleaving or a failure that timing alone would make rare. This note carries the design their
module doc comments link to. Both are named provisionally.

## Why a shipping kernel cannot contain either

Each module is declared under `cfg(feature = "system_tests")`, and so is every call into it. In any
other build the module does not exist, so a hook written without that `cfg` fails to compile
rather than shipping, and a hook written with it compiles to nothing. The feature cannot reach a
bootable kernel either: a kernel binary built with `system_tests` fails to link, on purpose
(`lib.rs`, `system_tests_main`). That is the first rung of the ladder in `AGENTS.md`, held up by the
second. There is no separate cargo feature per seam: a second feature would be a second thing to
keep out of shipping builds, and an unarmed hook costs one relaxed load on a path that is never the
IPC round trip.

## Holding a delegation open

A delegation is a syscall that files a copy of a capability its caller holds: `SEND_CAP`,
`ThreadControlBlock::CAP_INSERT` and `PageFrame::SLICE`. Each decodes its arguments, finds the
source capability and files a narrowed copy. A revocation sweep on another core must either see the
copy and delete it, or find the source gone before the copy is made. The seam lets a test ask what
happens when the sweep lands after the syscall has started and before the copy is filed. The same
seam sits in `PageFrame::MAP`, `AddressSpace::MAP_INTO` and `MemoryRegion::MAP`: a mapping is a use
rather than a delegation, but the question is the same, and one seam is one thing to keep in step.

A test calls `disarm`, starts a thread, and the thread calls `arm` on itself. At `here` the thread
reports itself parked and yields until the test calls `release`. Nothing is held at the seam, no lock,
no guard and no interrupt mask, so the test can run a whole revocation sweep while the delegation
waits. That is the interleaving two cores can produce by timing, made certain.

Where the seam sits decides what a green result means. In each syscall the read of the source and
the filing of the copy are one critical section serialized against every sweep, and the seam sits
just before it. So the case it forces is a sweep landing anywhere before the delegation's own lock
is held. `disarm` exists because a `parked` flag left `true` by the previous test was once read as
the next test's thread arriving: that test revoked before its own thread had armed, released
nothing, and its thread parked for good.

## Failing the Nth retype

A system test names a thread and a number N. Each time that thread makes the kernel spend a page of
one of its regions, a count goes up, and the Nth time the kernel answers as it would for a region
with nothing left. Sweeping N from 1 until a run never reaches N fails a program at every
allocation it makes, in order. It is the only way to run the cleanup paths
[untested error paths](untested-error-paths.md) counted and no test reached.

One retype is one call into the three functions of `memory_region` that take pages out of a region:
`split`, `retype_run` (which `retype_page` is) and `retype_object_page`. So `MemoryRegion::SPLIT`,
`RETYPE` and `RETYPE_OBJ` each count once, and a `MAP`, `PageFrame::MAP` or
`AddressSpace::MAP_INTO` counts once per page table it builds. That last kind is why the hook is
here rather than at the syscall: a mapping that fails after building one table runs the kernel's
own unwind, not only the program's. The check comes before the region lock, so a refusal leaves the
region untouched, exactly as an exhausted one is left.

The target is a thread, read from the core's current-thread word, because that is what the kernel
knows at a spending function without taking a lock. A service the test harness builds spends from
the harness's thread, so its construction is never counted; only what the service asks for itself
is. Arming clears the target first, so a spender racing it on another core sees either the old
target with its old count or nothing, never the new target with a stale count.

## BUGS

- One thread, one pause. Arming the delegation seam again replaces the target, and the pause
  disarms itself when it fires, so a second delegation by the same thread runs straight through.
  Every test that uses it delegates once.
- The pause's protocol is the caller's to follow: `disarm` before starting the thread, `arm` from
  inside it. Nothing enforces the order.
- The retype fault counts one thread, not a process. A process with two threads that both spend is
  counted on the named one only; a multi-threaded target needs a process identity at the spending
  function, which the kernel does not keep there.
- Only the Nth retype fails, not the Nth and every one after. A real exhaustion keeps failing. Failing
  once makes a sweep cheap (each N is one run) and lets the test check the program is still healthy
  afterward, but a program whose cleanup itself allocates is tested with that allocation succeeding.
- One retype target at a time. Arming again replaces the earlier target, and nothing in the suite
  runs two sweeps at once.
