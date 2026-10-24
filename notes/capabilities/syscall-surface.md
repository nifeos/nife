# Milestone 7d: the syscall surface, and refusing to be a deputy

*Name: provisional. After the parent's own heading.*

An appendix to [`notes/capabilities.md`](../capabilities.md). It is the record of milestone 7d (user mode: EL0, capabilities, the ELF loader, and IPC), when the decision (§10) became code.

The decision (§10) is now code. Three things landed: an ABI that is one artifact, a capability
table, and a syscall dispatcher whose most important function is the one that says *no*.

## 7d landed three calls; the surface is five today <!--count:syscalls-->

What 7d shipped, and the numbers in this section are 7d's rather than today's:

```text
  exit(code)                          authority over yourself
  yield()                             likewise
  invoke(cap, method, a0, a1, a2)     EVERYTHING ELSE
```

No `open`. No `read`. No `write`. No `fork`. A process acts on the things it was handed, and
`invoke` is how. `exit` and `yield` are plain syscalls rather than methods on a TCB capability,
because *a capability is authority over something else* and you do not need to be granted the
right to stop running.

This is DECISIONS §4 rule 3 ("the syscall surface stays narrow and explicit, a boundary not a
habit") meeting §10, and §10 is what makes three enough. A monolithic kernel needs a syscall per
verb because the verb is where the authority check lives. Here the authority is the capability,
and there is one verb: invoke it.

**One call has been added since, and it did not widen the boundary** (added by the 2026-08-17
documentation sweep, which found this heading claiming three in the present tense). `SYS_CAP_DELETE`
arrived on 2026-07-24 with milestone 19d: a loader retyping hundreds of frames through a 16-slot
capability table has to recycle slots, and forgetting something in your *own* table needs no capability for
the same reason `exit` does not. So the property this section is about survives the count
going up: three of the four calls are authority over yourself, and the fourth is everything
else. `notes/abi.md` is the current contract and has the full table.

## The ABI is a crate, not a convention

`crates/abi` is a `no_std` crate that both the kernel and every user program depend on. The
syscall numbers, the register layout, the method constants, the error codes: one definition. If
it changes, both sides fail to compile. A boundary that can drift silently is not a boundary, and
two files that "agree" by having been edited to match are exactly that.

## A capability is a file descriptor that can point at anything

`crates/capability` is the table, and it is pure logic, so it is host-tested in milliseconds. The
mechanism is deliberately boring: a `Vec<Option<Cap>>`, indexed by a small integer, living in
kernel memory. Userspace sees the integer. You cannot forge slot 7 for the same reason you
cannot forge `fd 7`: the table is not yours to write. No cryptography. A bounds check.

The one operation with teeth is `derive`, and its whole job is that rights only ever narrow:

```rust
if !rights.is_subset_of(src.rights) {
    return Err(Error::CannotWiden);
}
```

If delegation could widen authority the model is theater, because you would derive
yourself a better capability from the one you hold. And `NONE`/`READ`/`WRITE`/`GRANT` includes a
right Unix cannot express: `GRANT`, the right to pass a capability on. Our console capability
has `WRITE` and not `GRANT`, so the program may print and may not lend printing to anyone. Unix's
"the child inherits every fd" is the opposite default, and `FD_CLOEXEC` is the afterthought that
tries to walk it back.

For a long while `derive` and `GRANT` were true in the crate and unreachable from a running
process. Every capability was minted by the kernel and handed out at spawn. That made the kernel a
central authority-granting oracle, the ambient-authority shape §10 warned against, only relocated.
That is fixed. A process now delegates a capability to another process over an IPC endpoint
(`SEND_CAP` / `RECEIVE_CAP`), narrowing the rights on the way, and only if it holds `GRANT`. Authority
composes between processes at runtime instead of being wired by the kernel once. See
[delegation.md](../delegation.md).

## A new process holds nothing

`CapabilityTable::empty()`, and every thread starts with one. That constructor is the decision. A Unix
process inherits its parent's descriptors and can `open()` anything its uid allows. A nife
thread can name nothing until `sched::grant` puts something in its table, and the only thing
`user::exec_elf` grants is a `Console` with `WRITE`.

The demo spawns the byte-identical binary twice: once with the console, once with
`exec_elf_with(image, false)`. The first prints. The second cannot, and not because it is denied
when it tries. Slot 0 holds nothing, `invoke` returns `NoSuchSlot`, and there is no other way to
ask. Same program. Different world.

## The interesting refusal: the confused deputy, in our own kernel

The console `write` takes a pointer and a length, both chosen by the user. So the user hands us
`0xffff_0000_4008_0000` (our own `.text`) and a length, and asks us to print it.

**The kernel can read that address.** It reads it all day. So a `write` that dereferences
the pointer prints the kernel's memory *on the user's behalf, using the kernel's authority*, and
the program that could not read one byte of it receives all of it.

**No capability check catches this.** The console capability is genuine; the program is entitled
to print. The authority that leaks is not the console's. It is the kernel's own. That is exactly
the compiler service and the billing log in [the parent note](../capabilities.md): the deputy is confused about
*whose authority it is acting under*.

And it is real. With the check deleted, the demo prints raw kernel bytes, including the four
bytes `ARMd`, which is the [arm64 Image header magic](../boot-protocol.md) sitting at the start of
our `.text`.

### The refusal is one question, asked of the hardware

Not re-implemented in software and hoped to agree with the silicon. Asked of the silicon:

```text
  AT S1E0R, <addr>     translate this address AS EL0 WOULD, for a read
  ISB
  MRS  x, PAR_EL1      bit 0 (F) set  =>  it FAULTED  =>  EL0 could not have done this
```

One instruction does the stage-1 walk with EL0's permissions and reports the same "no" the
hardware would have given the user program itself. So `syscall::user_slice` asks, for every page
the range touches:

> **Could EL0 have read this itself?** If not, then neither may we, on its behalf.

`PAR_EL1` is a single shared register, so the `at`/`mrs` pair runs with interrupts masked: a
preemption in between could return with another thread's translation result sitting in it. Two
instructions, one masked window, and a bug that would never reproduce if you skipped it.

### The TOCTOU note, which is a comment today and a bug tomorrow

Between "the hardware says EL0 can read this" and "the kernel reads it", the mapping could
change. Nothing can currently change it: an address space belongs to exactly one thread, so there
is no second thread to unmap it, and there is no demand paging. When either arrives, borrowing
the user's bytes becomes unsound, and the fix is to copy them under a lock. The comment is in
`user_slice` so the next person meets it before the bug does.

## What it prints

```text
      hello from EL0, through a capability in slot 0.
      and it refused to read the kernel's memory on my behalf.

      every one of its expectations held, including that one.

      and spawned again with an EMPTY capability table, the very
      same binary cannot print one byte. slot 0 holds nothing,
      and there is no other way to ask.
```

## What is deliberately still kernel-served

Invoking the `Console` capability lands in the kernel, which owns the PL011. That is a
milestone away from what §10 (process model) promised. At milestone 8 (console driver) the console driver leaves the
kernel, `Object::Console` becomes a `Rendezvous` to a userspace console *server*, and the kernel
stops knowing what a UART is. Until that happens we have a capability system with a monolithic
kernel underneath it, which is honest scaffolding and not the destination.
