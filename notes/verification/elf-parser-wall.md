# Where BMC hit a wall: the ELF parser

An appendix to [notes/verification.md](../verification.md). It is why whole-parse totality of
`Elf::parse` is deferred, and how decomposing the arithmetic out of the loop recovered most of what
it was for. The harnesses themselves are in [parser-harnesses.md](parser-harnesses.md).

## The goal, and the two things that put it out of reach

The goal for `elf` was the big one: prove `Elf::parse` *total*, so that no byte string, however
hostile, makes it panic. A parser over attacker-controlled input is the textbook case for it, and a
panic there is a crafted binary halting the kernel. It did not work, and the reason is worth keeping.

Two things put it past bounded model checking:

1. A loop Kani bounds too loosely. `parse` has an `O(n^2)` overlap check over up to
   `MAX_PHNUM = 64` program headers. The real bound is far tighter. The header table must fit in the
   file, which at any small input size allows one or two headers. But that bound is *nonlinear*
   (`phoff + phnum * phentsize <= len`). Kani uses the *linear* `phnum <= 64` cap it can see for the
   unwinding assertion, so it insists on unrolling 64 deep. `unwind(65)` did not return in 7+
   minutes.
2. Symbolic slice offsets. `phoff` and each segment's `p_offset` come out of the file. So the reads
   land at *symbolic positions* in a symbolic array. That is expensive for the solver's memory model.
   It did not return even after pinning the header count to a single segment to kill the loop.

## The path forward

So *whole-parse* totality is deferred. But the first path forward turned out to recover most of what
it was for, so the story is worth following to its end rather than stopping at the wall:

- Factor the leaf arithmetic into a pure function, and prove that. Done. The per-segment bounds and
  overflow checks are now `check_segment_bounds`, a loopless function over a header's raw fields and
  the file length. Its harnesses prove three things. It never panics. A passing check yields an
  in-bounds range (`p_offset <= end <= file_len`, which is what makes `segment_at`'s slice safe). And
  a passing check rules out the `vaddr + memsz` overflow. That is the actual panic surface, proved
  for every input, without ever touching the loop. The refactor left the tests unchanged, so it is
  faithful.
- A loop-invariant tool (Verus), if the *loop itself* (the `O(n^2)` overlap check) ever needs
  proving rather than just the arithmetic inside it. Not needed yet.
- Shrink `MAX_PHNUM`. That is changing product code to suit the prover, and it is still the last
  resort.

The lesson, kept: BMC blunted against the loop and the symbolic slice base, and the fix was not a
bigger hammer but a smaller target. Decomposing the risky arithmetic out of the loop moved it from
"the solver never returns" to "verified in under a second". What remains unproved is narrow and
named. It is that the *number* of segments and their mutual overlap are handled without panic
across all 64 possible headers. The by-example tests still cover that.
