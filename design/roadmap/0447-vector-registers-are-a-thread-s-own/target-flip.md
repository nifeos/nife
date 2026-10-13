# Milestone 447 appendix: the target flip, as priced when the save was built

An appendix to [milestone 447 (a thread's vector registers are its
own)](../0447-vector-registers-are-a-thread-s-own.md). The block is complete without it. The flip
itself is [milestone 534 (the soft-float targets could now be
flipped)](../0534-the-soft-float-targets-could-now-be-flipped.md); this is 447's pricing of it,
taken on 2026-09-20, so that the flip can be made on evidence.


## What it would take

Four edits and a rebuild:

- Drop `+soft-float` and the `-sse`/`-neon` feature strings from the three files in `targets/`.
- Drop `"rustc-abi": "softfloat"` from the two that carry it.
- Rebuild the `std` farm (`xtask std-src`, which every lane already takes the `nife-dev` toolchain
  link for).
- Rebuild every user program.

Nothing in the kernel changes. It stays `softfloat`, and should. A kernel that emits FP into its
own fastpath would pay the save on every switch, rather than on the switches of threads that asked.

## What it would buy

Less than the framing suggests, and the honest accounting is worth having before the decision
rather than after.

- `aes_force_soft` could go, and it is currently the only force-soft flag in this tree. Milestone
  164 (x86_64 userspace can't build `aes`: no SSE, no scalar fallback) and the brief for this
  milestone both suggest a family of them. There is one, in `.cargo/config.toml`'s
  `[target.x86_64-unknown-none]` block, and `grep` finds no other. Worth saying plainly, because
  the flip's case is weaker than "six flags disappear".
- AES-NI against the bitsliced software backend. Upstream RustCrypto puts the hardware path roughly
  an order of magnitude ahead. 164 refused to measure this and was right to. Nothing on x86_64
  mounts an encrypted RedoxFS volume, so there is no workload, and a synthetic number would be a
  fact leaving the machine with nothing behind it. That refusal still stands after this milestone.
  What changed is that the number is now obtainable rather than blocked.
- The second failure class of milestone 442 (a crypto provider `rustls` can use on all three
  bare-metal targets). Its block records that `sha2` and `polyval` "fail on soft-float x86_64",
  met through `embedded-tls`. It immediately warns that the probe behind that finding "ran against
  stock bare targets on the stable host toolchain, not against nife's own target specifications".
  442 is NOT-STARTED and owes that re-measurement. It should be run before the flip, not after. If
  those crates build against nife's targets on the pinned nightly, this half of the case
  evaporates. If they do not, 442 has the specific list the flip would have to fix.
- The seam of §31 (the foreign-language seam: C holds no capabilities and makes no syscalls)
  widens. A C component compiled by bare-metal clang currently cannot use vector registers at all.
  After the flip it can, and §31's sentence about "a trap or a corruption depending on which of
  those two bit first" stops being true. That is a real gain for the vendored-component rung the
  seam exists to de-risk.

## What it would cost

Every thread that then executes an FP instruction takes one trap. It then pays 512 bytes of save
and restore on every subsequent switch for the rest of its life, because `live` never clears (this
is `crate::fp`'s first `BUGS` entry).

Soft-float userspace pays nothing today because nothing takes the trap. Hard-float userspace means
`memcpy`, `std` formatting and anything LLVM feels like vectorizing will take it. So most threads
become live, and the common case in the block's switch table stops being the common case. Nothing
in this tree measures that, and the measurement does not exist until there is a hard-float
userspace to run.

## What would have to be rebuilt

The `std` farm at minimum, and therefore every user program and every archive. `nife-dev` is one
symlink per user account, so the rebuild is machine-global. It has to be sequenced against other
lanes (AGENTS.md's `std_src` rule, and notes/std.md's 2026-08-18 cross-contamination).

## What is not known, and would decide it

Whether 442's crates actually fail against our targets, and what a hard-float userspace does to
the switch cost when most threads are live. Both are measurements rather than arguments, and
neither needs this decision made first.
