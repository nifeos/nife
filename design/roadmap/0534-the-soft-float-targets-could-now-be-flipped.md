---
status: NOT-STARTED
raised: 2026-09-20
promoted_from: the-soft-float-targets-could-now-be-flipped
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 534. The soft-float targets could now be flipped, and two measurements should come first

Promoted from the proposal `the-soft-float-targets-could-now-be-flipped`, filed 2026-09-20, on
calef's instruction of 2026-09-20 to give every proposal on `main` a number. The text below is the
proposal's own, unedited except for this paragraph: the argument is its author's and promotion is
not the moment to improve it. Filed by the lane of milestone 447 (a thread's vector registers are
its own), which built the thing that was blocking the flip and deliberately did not take it.

Both measurements below run on the dev Mac and need nobody. The flip itself is
calef's ruling rather than a lane's, which is what makes this a proposal.

## Why this is a proposal and not a change

Every target in `targets/` is soft-float. Milestone 184 (extend the `std` port to x86_64) calls
that a correctness requirement, because a kernel that saves no FPU or SSE state on a context switch
needs it. Milestone 447 (a thread's vector registers are its own) saves that state on all three
architectures, so the feature strings are a choice.

Taking that choice is not a lane's. Turning `+soft-float` off changes the calling convention for
every userspace binary and every `std` crate built against it, which is an ABI two programs agree
on. `AGENTS.md`'s *move fast on what can be undone* tenet puts that squarely in the irreversible
category. And §22 (Rust `std` on the native ABI, the Hermit way) picked the current
one on purpose when it specified nife's target JSON as "softfloat, and `singlethread = true`".

## What it would take

Four edits and a rebuild.

1. Drop `+soft-float` and the `-sse`/`-neon`/`-mmx` feature strings from the three files in
   `targets/`.
2. Drop `"rustc-abi": "softfloat"` from the two that carry it.
3. Rebuild the `std` farm (`xtask std-src`). `nife-dev` is one `rustup toolchain link` per user
   account, so this is machine-global and has to be sequenced against other lanes; notes/std.md has
   the 2026-08-18 cross-contamination that makes that a rule rather than a courtesy.
4. Rebuild every user program and every archive.

The kernel stays `softfloat`, and should. A kernel that emitted FP into its own fastpath would
pay the register-file save on every switch instead of only on the switches of threads that asked
for the unit, which is the whole of 447's cost argument.

## What it would buy, honestly accounted

- Seven flags disappear: `--cfg aes_force_soft` in `.cargo/config.toml`'s
  `[target.x86_64-unknown-none]` block, and in `cryptography_provider`, from milestone 442 (a
  crypto provider `rustls` can use on all three bare-metal targets), a `force-soft` feature on
  `sha2` and five `--cfg` flags selecting portable implementations.


  The correction is worth more than the count. A lane measures the tree it branched from, and a
  claim of the form "grep finds no other" is a statement about a moment. This one went stale before
  its own pull request merged, which is the hazard every block asserting an absence carries.
- **But not all seven, and this is the sharpest thing 447 can say about the flip.** 447 saves the
  `FXSAVE` area: x87, `MXCSR` and `xmm0`-`xmm15`. It does **not** save AVX, and it cannot, because
  `CR4.OSXSAVE` stays clear and that is precisely what makes every VEX-encoded instruction `#UD`
  rather than a silent corruption (447's `arch/x86_64/fp.rs` carries this as its first `BUGS`
  entry). `cryptography_exerciser/.cargo/config.toml` says `chacha20_force_soft` is there for a
  run-time reason where the others are compile-time ones: `chacha20` "compiles fine and then
  executes an AVX2 instruction in ring 3", and the program "dies with `vector 6 (invalid opcode)`
  before it prints a byte". Flipping the targets does not change that one byte. A crate that picks
  its implementation by asking the CPU rather than by asking the compiler stays a hazard after the
  flip, and `chacha20_force_soft` stays with it until this kernel grows an `xsave` path and sets
  `XCR0`. Anyone pricing the flip should count the flags it retires, not the flags that exist.
- AES-NI instead of a bitsliced software backend, roughly an order of magnitude by upstream
  RustCrypto's own figures. Still unmeasured, and 164's refusal to measure it still stands:
  nothing on x86_64 mounts an encrypted RedoxFS volume, so there is no workload and a synthetic
  number would be a fact leaving the machine with nothing behind it. What 447 changed is that the
  number is obtainable rather than blocked.
- **The seam of §31 (the foreign-language seam: C holds no capabilities and makes no system calls)
  widens.** A C component
  compiled by bare-metal clang currently cannot touch a vector register at all, and §31 says why:
  "the kernel never enables FP/SIMD for EL0, and the context switch" saves nothing, so such a
  register would be "a trap or a corruption depending on which of those two bit first". After the
  flip that sentence stops being true, which is a real gain for the vendored-component rung the
  seam exists to de-risk.

## What it would cost

Every thread that executes an FP instruction takes one trap and then saves and restores the whole
register file on every subsequent switch for the rest of its life, because 447's `live` flag
never clears (its first `BUGS` entry). Today that costs nothing, because no thread in this tree has
ever taken the trap and `crate::fp::ENABLES` is zero on every boot. A hard-float userspace means
`memcpy`, `std` formatting and anything LLVM feels like vectorising will take it, so most threads
become live and 447's common case stops being common. 512 bytes each way per switch, against a
switch that costs about 565 instructions on aarch64 today.

Nothing measures that, and nothing can until there is a hard-float userspace to run.

## The two measurements that should come first, and neither needs the decision made

1. Re-measure 442's soft-float failures against nife's own targets. Its block
   records that `sha2` and `polyval` "fail on soft-float x86_64", met through `embedded-tls`, and in
   the next sentence warns that the probe behind that finding "ran against stock bare targets on the
   stable host toolchain, not against nife's own target specifications and pinned nightly". 442
   has since done exactly that and is BUILT: `script/crypto-probes` measures against
   `targets/*-unknown-nife.json`, and the answer was that those crates do build here, with the
   force-soft flags above. So this item is closed rather than owed, and what it hands the flip is
   the concrete list of seven flags, not an unknown.
2. Price a live thread's switch. Build one user program hard-float against a scratch target
   (not the committed ones), run `script/bench`'s `yield_switch` and `ctx_switch` with it in the
   mix, and report what a switch costs when both threads are `live`. That turns the paragraph above
   from an argument into a number, and it is the number the flip is actually deciding.

## What this proposal deliberately does not do

It does not recommend. The *fork reaches calef with its questions already answered* tenet asks for
options and costs on an irreversible fork and explicitly **not** for a winner, because "a
system-call-surface decision arriving with a recommendation is already most of the way made" and an ABI
is the same shape of thing.

## Index row

Every target in `targets/` is soft-float.
