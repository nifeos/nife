# What one frame costs, statically (2026-08-13)

An appendix to [notes/stack-high-water.md](../stack-high-water.md), which summarizes it. It is the
other half of the instrument: the watermark measures depth at run time, and this measures each
function's frame at build time.

A watermark says how deep the suite went. It cannot say which function is expensive, and that is
the question you have when a stack overflows. The fix is either "raise the limit" or "shrink the
offender", and the watermark does not distinguish them.

The compiler will tell you, and it needs no emulator. That matters because it works from a laptop
or a container with no QEMU:

```sh
RUSTFLAGS="-Z emit-stack-sizes" \
  cargo test -p kernel --target aarch64-unknown-none-softfloat --no-run
# then read the .stack_sizes section of the reported artifact:
#   llvm-objcopy --dump-section .stack_sizes=ss.bin <artifact> /dev/null
#   llvm-nm -C --defined-only -S <artifact>        # to name the addresses
```

Each entry is a function address and its frame size in bytes. Measure the test build, not
`cargo build`. Half the kernel's spawn paths and every test body are `cfg(test)`, so the plain
binary is missing exactly what you are chasing. That mistake cost an hour on 2026-08-13, and the
tell was `llvm-nm | grep <a test-only symbol>` returning nothing.

## What it found, and why the parent's 71% row was a warning nobody read

The deepest frame in the kernel was `sched::reap_region_objects` at 6816 bytes, of which 6144 was
three scratch arrays sized to their table maxima:

| local | size |
|---|---|
| `doomed: [u64; MAX_THREADS]` | 1024 |
| `doomed_eps: [u64; MAX_ENDPOINTS]` | **4096** |
| `waiters: [u64; MAX_THREADS]` | 1024 |

All three are gone, and the last two were removed by the same pressure that found them. `doomed_eps` went first, replaced by the rescan-for-one-at-a-time loop. Its comment in
`sched::reap_region_objects` is the general statement of the rule. The other two survived because
1024 bytes each looked affordable. That is the tell this table exists to make visible: a `[T; MAX]`
local is a stack allocation wearing the clothes of a bound. It grows when the bound does, and nobody
is watching that line.

Raising `sched::MAX_THREADS` to 256 took the frame to 4,624 bytes, over `script/stack-frame-check`'s
4,096-byte guard-page ceiling, and the gate caught it before any run did. `doomed` became the same
rescan loop. `waiters` was removed outright, by waking each thread inside
`Rendezvous::drain_waiters`'s callback rather than listing them first. The entry is already off its
queue when the callback runs, which is what makes the wake legal there. The frame carries no
table-sized local at all now.

Now put that next to the parent's own thread-stack row. The measured high-water was 11672 of 16384
bytes, 71%, leaving 4712 bytes. The reap frame wanted 6816, which is 2104 bytes more than the entire
remaining headroom. Any chain that reached the measured peak and then entered a reap could not fit,
and would land on the guard page.

That is what happened. Milestone 108's branch faulted one CI run in five with `FAR_EL1` exactly on
the guard page of thread stack slot 87, and the tests running were the supervision and reap ones.
The branch was held on suspicion of having introduced it. The static measurement says otherwise:
comparing its test binary against `main`'s, function by function, shows the largest single frame
growth in the whole milestone is 128 bytes. It added one more spawned program to a margin that was
already 2104 bytes short.

The 71% row had been sitting in the parent since milestone 84 (stack high-water). A percentage reads
as comfortable, and 4712 bytes of headroom reads as comfortable, right up against a single frame
that needs more than all of it. The lesson is that a high-water percentage and a frame inventory
answer different questions, and neither is safe alone.

## The fix, and the shape worth copying

`doomed_eps` existed because `remove` mutates the table and you cannot remove while iterating it, so
the names were collected first. Rescanning for one at a time removes the array entirely. The frame
went 6816 to 2560 bytes, and it now fits inside the measured headroom with 2152 bytes to spare. The
cost is O(live endpoints) per removal on a teardown path with a 512-slot table, which is not where
this kernel's time goes.

The general shape: a `[T; MAX]` local sized to a table maximum is a stack allocation wearing the
clothes of a bound. `MAX_ENDPOINTS` is 512 because that is a sensible ceiling on live endpoints, and
nothing about that number was ever a claim about how much stack a function may use. The two got tied
together by the convenient shape, and the connection was invisible until something measured it.
