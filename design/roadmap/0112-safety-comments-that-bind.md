---
status: BUILT
raised: 2026-08-04
built: 2026-08-05
---
# 112. The SAFETY comments that bind nobody

Built 2026-08-05 (pull request #120, merge `4904cce0`). Raised 2026-08-04 by milestone
82's lane, which set out to burn down `unsafe fn` violations, found zero of them, and reported this
instead. Recorded in `notes/unsafe-obligations.md`'s BUGS. It is a change to the kernel's soundness
surface, which is why 82 deliberately did not take it on a lint milestone. The status read
`IN-PROGRESS since 2026-08-04, a developer holds it on milestone/112-safety-comments` for twelve days
after that merge; found 2026-08-17 by the status-accuracy sweep. §76's defect class.

Three of the four converted, and the fourth was decided rather than skipped, which is what the
deleted gate line asked for when it said the answer may not be four conversions. `write_satp` is now
`unsafe fn` (in `kernel/src/arch/riscv64/mmu.rs`), and its `# Safety` section says out loud that it
is unsafe because aarch64's `set_ttbr0` is. That closes the ISA asymmetry in the direction of the
stricter ISA. `switch_user_root` is `unsafe fn` on both ISAs
(in `kernel/src/arch/aarch64/mmu.rs` and `kernel/src/arch/riscv64/mmu.rs`). `stack::paint` is
`unsafe fn` (in `kernel/src/stack.rs`), and its sibling `high_water` went with it. `virtio::pread`
stayed a safe fn on purpose, with the argument written where a reader meets it
(`pread` in `kernel/src/virtio.rs`, "Why these two stay safe fns"). It is private, all twenty call sites
are one `impl Transport` block, and the compiler closes the caller set. So the obligation really is
discharged by the module rather than onto nobody.

The call sites carry the cost the block predicted. `schedule`'s `switch_user_root` call (in
`sched/switch.rs`) is the context
switch, and its SAFETY comment argues liveness from `on_cpu` and `Running` after `SCHED` is released
rather than restating the signature; the one in `adopt_address_space`
(`sched/thread_control_block.rs`) carries its own separate argument.

One gate did arrive, adjacent to the headline property rather than on it: `script/lint` now checks
that every `unsafe fn` states its contract in a `# Safety` section. It checks that a contract is
*written*, never that it is true, and it found one violation on its first run (`fs_server`'s
`file_page`, fixed in the same pull request). The decision table is in
notes/unsafe-obligations/caller-obligations.md, "The comments that bound nobody, decided (milestone
112)".

The finding. Four safe functions carry a `// SAFETY:` comment that discharges an obligation
onto "the caller", and their signatures impose that obligation on nobody. Any safe code may call
them without meeting it, and both `unsafe` lints are satisfied, because there is no `unsafe fn` and
no undocumented block for either to fire on.

| Site | The comment's claim |
|---|---|
| `pread` in `kernel/src/virtio.rs` | "the caller passes addresses inside a device-mapped BAR or mmio window" |
| `paint` in `kernel/src/stack.rs` | "the caller hands us a mapped, unused stack region" (`#[cfg(test)]`, so test builds only) |
| `switch_user_root` in `kernel/src/arch/aarch64/mmu.rs` | "the caller passes either a live `AddressSpace`'s composed value or ..." |
| `write_satp` in `kernel/src/arch/riscv64/mmu.rs` | "the caller guarantees `satp` names a well-formed Sv39 root" |

The last one is an ISA asymmetry, and it is the sharpest argument in the set. aarch64's
equivalent, `set_ttbr0`, is an `unsafe fn`. The same register write, the write that installs a
user address space, is a contract on one architecture and an ordinary call on the other. Rule 5 says
a capability ships on every supported architecture or a scope note records the gap; here the *rule
about the code* differs by ISA, which is the same defect one level up.

**Why 82 did not just fix it.** Turning these four into `unsafe fn`s puts an `unsafe` block and a
real SAFETY comment at every call site, including the context switch. That is the hottest path in
the kernel and the place where an added obligation is most likely to be discharged by ritual rather
than by thought. It is a change to what the kernel promises about itself, so it earns a review of its
own rather than a ride on a lint milestone.

**Not every "caller" in a SAFETY comment is this**, and the milestone has to tell them apart or it
will churn a dozen sites for nothing. `sched.rs`'s `ipc_call` and `user_rt`'s `cap_delete` mean the
calling *thread* and the calling *process*; `interrupts::enable` says outright that the operation is
sound and only the timing is the caller's problem. The pattern to look for is a safe fn that would
be unsound if its own sentence were false.

The related finding, which is the reason to read the whole set rather than the four. Of the 33
`unsafe fn`s in the tree, **eleven contain no unsafe operation at all**. Their unsafety is a
contract about *meaning*, not a memory operation a compiler can point at. `set_ttbr0` is an `unsafe
fn` because writing `TTBR0_EL1` is the most consequential thing in the kernel. `aarch64-cpu` hands
that write over as a safe call. `Clock::new` takes a virtual address the caller promises is a mapped
clock page. `assume_no_stale_entry` is named for its contract. For those eleven the lint composition
buys nothing. The rustdoc `# Safety` section is the only enforcement there is, for a third
of the tree's unsafe functions. That is worth knowing before deciding how many of the four to
convert. Conversion moves a site *into* that same category rather than out of it. An `unsafe fn`
with a contract nothing checks is better than a safe fn with a contract nothing checks,
and it is not a proof.

## Scope note

**Four sites, and the answer may not be four conversions.** `stack::paint` is `#[cfg(test)]` and its
caller is one loop in `smp.rs`; `write_satp` has the parity argument and probably converts;
`switch_user_root` and `pread` are the ones where the call-site cost is real and the decision is a
judgement. Decide per site with an argument, the way milestone 78 decides per assertion, rather than
producing a rule.

**No new gate is proposed here.** Neither lint can read a comment, and a lint that spots "SAFETY on a
safe fn" would fire on the legitimate uses above. If the survey finds the distinction mechanical
after all, that is a finding for `DECISIONS` §61's ledger (a lint is adopted on evidence from this
tree), not an assumption to start from.

## Follow-on

- **Refused.** Converting `virtio::pread` to an `unsafe fn`, the fourth of the four sites. The
  argument is written where a reader meets it, at `pread` in `kernel/src/virtio.rs` under "Why these two
  stay safe fns". It is private, all twenty call sites sit in one `impl Transport` block, and the
  compiler closes the caller set. So the obligation is discharged by the module rather than onto
  nobody. That is the case the other three could not make.
- **Refused.** A lint for "a SAFETY comment on a safe fn". Neither unsafe lint can read a comment,
  and a check for this shape would fire on the legitimate uses the block separates out, where
  "caller" means the calling thread or process rather than a soundness obligation. If the
  distinction ever turns out to be mechanical, `design/decisions/0061-lints-on-evidence.md` is the
  ledger that adopts a lint on evidence from this tree.
  This refusal is milestone 453 (design/roadmap/0453-a-lint-for-a-safety-comment-on-a-safe-fn.md),
  which carries it with the condition that would change it.
- **Recorded.** `notes/unsafe-obligations.md` carries the related finding, which is the wider one.
  Eleven of the tree's 33 `unsafe fn`s contain no unsafe operation at all. So their unsafety is a
  contract about meaning and the rustdoc `# Safety` section is the only enforcement there is for a
  third of them. Converting a site moves it into that category rather than out of it.

## Index row

Four safe fns discharge an obligation onto "the caller" that their signatures impose on no one, so
both unsafe lints are satisfied and any safe code may call them. `riscv64::write_satp` is also an
ISA asymmetry: aarch64's `set_ttbr0` is an `unsafe fn` for the same register write. Fixing them
adds an unsafe block at every call site including the context switch, so it wants its own review
