# The unsafe census: the measurement (milestone 134)

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/unsafe-obligations.md](../unsafe-obligations.md), which summarizes it. The
ceilings each number is held to are in [ceilings.md](ceilings.md).

The rest of the parent is about whether an obligation is *written*. This page is about how much
unsafe there is and which way it should go, which calef raised on 2026-08-18 in one question:
*"How much unsafe code is there in a code base? Is that something we should be monitoring and
driving in a particular direction over time?"*

He approved folding it into milestone 134 (the register of measures) rather than building it standalone, on the reasoning that
a standalone census produces another one-time number nobody re-takes. So every number here is
derived by `script/lint` on every build; none of them is typed. The register that holds them all,
with the test for what belongs in it, is notes/register-of-measures.md.

## The measurement, and the thing it found

Measured over the Rust that runs on nife, which is every tracked `.rs` file except `vendor/`,
`patches/`, and the host-side tooling in `bench/host/`, `xtask/`, `tools/`, `fuzz/` and `helpers/`.
Each exclusion's reason is in `script/lint` beside the derivation; `patches/` is a real hole rather
than a boundary and the register's BUGS says so.

| | 2026-07-15 | 2026-07-28 | 2026-08-04 | 2026-08-14 | 2026-08-18 | 2026-08-23 | 2026-09-01 |
|---|---|---|---|---|---|---|---|
| `unsafe {}` outside `kernel/src/arch/` | 171 | 426 | 728 | 763 | 747 | 777 | 698 |
| code lines outside it | 7,508 | 19,223 | 58,351 | 64,452 | 80,359 | 85,530 | 88,596 |
| **blocks per 10,000 lines** | 227.8 | 221.6 | 124.8 | 118.4 | 93.0 | **90.8** | **78.8** |
| `unsafe {}` inside `kernel/src/arch/` | 34 | 102 | 128 | 134 | 139 | 141 | 248 |
| `unsafe impl Send`/`Sync` | 7 | 12 | 15 | 15 | 17 | 20 | 23 |

The 2026-09-01 column is the first one taken after a round worked the kernel: milestone 139 (drive
the unsafe count down), round 8, in [ceilings.md](ceilings.md). Two things in it are worth reading together rather than separately. The
outside-`arch/` count is the lowest since 2026-08-04 while the tree is at its largest, which is the
density's whole point. And `arch/` nearly doubled between the last two columns (141 to 248) without
anything drifting. The `x86_64` port of milestone 161 (the x86_64 kernel port) is a third architecture's worth of assembly,
system registers and MMU code. That is exactly the population this measurement excludes on purpose.

The 2026-08-23 column mixes two different things and the density is what separates them. The
raw count outside `arch/` rose by 30 (747 to 777) between 2026-08-18 and this lane starting,
because five days of unrelated tree growth (other milestones) added unsafe at roughly the tree's
own rate. Against that growth, milestone 139 alone removed 22 net blocks (24 hand-rolled
volatile-access blocks in seven programs, collapsed into two generic methods in one new crate
module): the count outside `arch/` immediately before this lane's reduction was 799, not 777. The
density is the number that tells the two apart. It was already at 93.4 (799 blocks over 85,476
lines) when that lane started, essentially unchanged from 2026-08-18's 93.0 despite five days of
unrelated growth. The reduction alone took it to 90.8. [ceilings.md](ceilings.md) has the cluster.

(`script/lint` prints the density as an integer, truncated: 92 rather than 93.0. Truncated on
purpose, so a ceiling can never fail a tree that sits exactly on it.)

The absolute count more than quadrupled and the density more than halved, falling at every
sample. Both facts are true and only the second one is about this kernel's soundness: the first is
a system being built. That is the whole reason the gate holds a ratio rather than a count.

Nothing was measuring either. The clearest evidence is a single commit two days before this was
written: `d5a969a2`, "user_rt: one trap instruction, not forty-eight". (The crate is
`user_mode_runtime` since 2026-09-13; a commit subject keeps the spelling it was written under.) It
took the count from 863 to 769 in one change, 10.9% of all non-arch unsafe. It did so by lifting a
panic handler that 48 binaries had each inlined with two `unsafe` blocks and two SAFETY comments.
Its commit message argues from §61 (a lint is adopted on evidence from this tree, not on its description) that a SAFETY comment is an assertion and not a formality, and
it is exactly right. What it could not say, because no instrument existed, is that the tree had been
asserting that particular invariant 96 times and now asserts it once.

## `// SAFETY:` parity is deliberately not a gate

The obvious next check is that every `unsafe {}` block has a `// SAFETY:` comment, compared by
count. It should not be built, and measuring it is what settles that, in two ways that both
point the same direction.

`clippy::undocumented_unsafe_blocks` already enforces exactly this, per block rather than in
aggregate, as a hard error through `-D warnings` across all fourteen configurations this script
builds. A count check cannot be stronger than that; it can only disagree with it.

And it disagrees badly, in a way that gets worse the harder you try. A regex anchoring `SAFETY:`
to the head of the comment block above each `unsafe {}` reports 65 undocumented blocks in code
the gate compiles clean. Loosening it to accept the comment mid-line, which is how most of this
tree writes it (`// ... the frame was retyped with GRANT. SAFETY: svc.`), still reports 38. The
ones read are all false positives: a `#[cfg]` attribute sits between the comment and the block, or
the comment covers a closure whose body holds the block, or it covers the first of two blocks on
one line. A gate whose failures are documents that are right is the gate somebody deletes, which
notes/counted-claims.md names as the way this convention dies.

One residue is worth knowing rather than gating: `patches/std-nife/overlay/` holds 37 blocks and
15 of them carry no `SAFETY:` comment in any form, because that code is compiled into `std` by
the farm and by no clippy configuration here. That is a coverage hole in the lint policy rather
than a comment shortage, and it is recorded in the register's BUGS.

## What `user/`'s share is actually made of

On 2026-08-18, `user/` held 287 of the tree's unsafe blocks, the largest share of any directory,
which looks wrong for userspace in a capability system. Milestone 175 (split `user/`) divided that
crate into `components/` and `fixtures/` on 2026-09-13; the table below was not re-taken after the split
(noted 2026-10-11). Reading the first token inside each block says what it is:

| shape | blocks | what it is |
|---|---|---|
| `invoke(...)` | 114 | one raw capability invocation, the userspace syscall |
| `read_volatile` / `write_volatile` | 102 | a byte or word through a granted shared page |
| `from_raw_parts` / `from_raw_parts_mut` | 25 | the same page as a slice |
| `core::arch::asm!` | 12 | entry stubs and the trap |
| everything else | ~34 | mixed |

So it is neither raw pointer arithmetic nor a missing abstraction in the usual sense. Two
populations, and both are one wrapper away. The 114 `invoke` sites all call one `unsafe fn` whose
own `# Safety` section says *"the kernel validates the capability and the method before acting; that
is its whole job. The caller is trusting the kernel, not the other way around"*, which describes an
obligation on nobody. It is not simply mismarked: a few methods (`address_space::MAP_INTO` among them) can
perturb the caller's own address space, so *some* obligation is real. But it is a per-method
obligation carried by a single all-methods signature, and 114 blocks assert it identically. The 127
volatile and slice accesses are the same story about granted pages.

Both are the shape `d5a969a2` already fixed once, in one commit, for the panic handler. Neither is
this milestone's work; the handoff in its lane report proposes it.
