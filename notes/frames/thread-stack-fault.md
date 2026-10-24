# A thread-stack fault that milestone 108 surfaced and did not cause

*Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `stack-frame-fault`.*

An appendix to [notes/frames.md](../frames.md). It predates the `Frame` -> `PageFrame` rename.
Every "frame" in it is a CPU call frame, the compiler's stack-size accounting for one function
activation, never the kernel object the parent note describes. It is left with that word as it read
before the rename. Rewriting "frame" to "page frame" here would manufacture the opposite of the
rename's own purpose: a false claim that a stack frame is a physical page.

## The fault

The suite overflowed a kernel thread stack intermittently on the branch of milestone 108 (the drivers move onto frame
capabilities), and the cause was not that milestone. It was fixed on `main` before the
milestone merged. It is kept because the next person to meet a fault on this path should meet the
whole story. "The milestone that surfaced it was not the milestone that caused it" is the part that
would otherwise be lost.

One run in five faulted (2026-08-13; four green runs on the branch, one red). The kernel binary was
byte-identical between a run that faulted and a run that passed. The two commits differ only in
`.github/dependabot.yml`, `.github/workflows/toolchain-bump.yml` and `script/ci-qemu`, with nothing
under `kernel/`, `crates/`, `user/` or `redoxfs_server/`. So it is depth-dependent rather than
deterministic, and re-running until green would hide it.

```
ESR_EL1  0x96000047   EC 0x25, data abort taken without a change in EL (so: kernel mode)
                      WnR 1, a write
                      DFSC 0x07, translation fault at level 3
FAR_EL1  0xffff0010001b3000
ELR_EL1  0xffff00004012fa34
x8       0xffff0010001b7a90
```

`FAR` is exactly the guard page of kernel thread stack slot 87. `thread::STACK_AREA` is
`KERNEL_VA_BASE | 0x10_0000_0000` and the per-thread stride is five pages (`STACK_PAGES` = 4 plus
one guard). So `FAR - STACK_AREA` is `0x1b3000` = 87 × `0x5000` with a remainder of zero. `x8` is
`0x4a90` into the same slot, which is that thread's own stack, 1392 bytes below its top. So the guard
page did its job: a 16 KiB kernel stack ran out and the write below it was caught rather than
quietly landing on the neighbor.

It faulted while the supervision and reap tests were running. The console interleaves, so which
test owns it is not established, and this note should not pretend otherwise.

## The stack did not run out, and the register said so

That last claim about the stack running out is wrong (2026-08-16). A stack 1392 bytes below its top
has used 8% of itself, not all of it, and the two claims sat in the same paragraph. This exact `FAR`
came back twice more, byte for byte: after #157 shrank `reap_region_objects`, and after milestone
124 rebuilt the spawn path. No depth-driven overflow could do that. And `script/stack-depth-check`
now says the deepest chain a thread stack can carry is 13792 bytes, against the 20480 this address
would need. See notes/stack/guard-page-faults-2026-08-16.md. The rest of this entry stands: the
fault is real, it is not milestone 108's, and the binary was byte-identical between a red run and a
green one.

The answer arrived on 2026-08-17, and it vindicates the register and not the reasoning beside it.
The recurring address proved nothing. A fault that reaches the exception vector's own frame store
walks `sp` down and stores upward in aligned steps. So its terminal store lands on the guard base
every time, regardless of what `sp` was doing. What `x8` was telling you is exactly right, though,
and it is the whole diagnosis in one register. The thread was shallow on slot 87's stack because
the stack had been unmapped under it. A supervised corpse is published `Dead` while still executing
on its own kernel stack. An out-of-band region reap frees that stack before the corpse reaches
`switch_to`. See notes/stack/kernel-stack-freed-under-its-owner.md, and the block of milestone 124
(a thread is born where it lives).

## A correction worth keeping

The wrong reading was reasonable and cost an hour. The first pass decoded `FAR` through
`phys_to_virt` (which is `pa | KERNEL_VA_BASE`). It read the result as physical `0x1b3000` with a
stray bit 36, and concluded the pointer was corrupted. Bit 36 is not corruption: it is `STACK_AREA`,
placed 64 GiB up precisely so that a stack address can never collide with the virtual name of a
physical one. `thread.rs` says so in the comment above the constant. The lesson is that a high-half
address is not automatically a physmap address. Masking off `KERNEL_VA_BASE` is not a decode unless
you have first established which region you are in.

## What it turned out to be, and why milestone 108 was the wrong suspect

The milestone was held rather than merged on four green runs out of five. The investigation went
looking for what made its branch's kernel path deeper, and it was not the branch. Per-function
frames were measured with `-Z emit-stack-sizes`, comparing the milestone's test binary against
`main`'s. The largest single frame growth in the whole milestone is 128 bytes. Its biggest new frame
is one more `spawn_on` instantiation, the same size as the eight already there.

The cause was `sched::reap_region_objects` on `main`. Its frame was 6816 bytes, of which 4096 was
one `[u64; MAX_ENDPOINTS]` scratch array, against a measured thread-stack headroom of 4712 bytes.
That one frame wanted 2104 bytes more than all the headroom there was. So any chain reaching the
measured peak and then entering a reap could not fit. The milestone added one more spawned program
to a margin that was already short, which is why it faulted there first. Fixed on `main`
(notes/stack-high-water.md), and `script/stack-frame-check` now fails the build on a frame that
size.

The general lesson is worth more than the bug. The milestone a fault appears in is not necessarily
the milestone that caused it. On a shared resource as invisible as stack depth, the last change to
arrive gets blamed for a margin that many changes spent.
