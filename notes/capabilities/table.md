# The capability table crate: `crates/capability`

An appendix to [`notes/capabilities.md`](../capabilities.md), which says what a capability is and
why the kernel has no `open()`. This page carries the design of the crate that holds the table, the
rights and the two pure decisions the kernel makes about supervision. The rulings it implements are
§10 (process model: capability-based, microkernel), §12 (Call/Reply IPC: a one-shot reply
capability), §16 (object revocation: reclaim the objects a process built) and §32 (a supervisor may
collect a corpse without being able to build one).

## The table

The table is flat: an array of slots, each holding an object and its rights. seL4 uses a tree of
CNodes with guard bits, which buys sparse capability spaces and costs a great deal of explanation.
A flat array is an fd table with a type tag on each entry, which is what a capability space is. A
sparse version, if one is ever needed, is a change to that one file.

The unforgeability is an array and a bounds check. The table lives in kernel memory and userspace
sees only a slot number, so it cannot fabricate slot 7 for the reason it cannot fabricate fd 7.
There is no cryptography. A brand-new table holds nothing, so a fresh process can name nothing until
somebody hands it something. Under Unix a fresh process inherits every fd its parent had.

An empty slot answers `NoSuchSlot` even when the caller asked for rights. "You may not" and "there
is nothing there" are different answers, and the second leaks less.

The table is a const-generic array, so its size is part of its type and creating one touches no
heap. The kernel picks the size once, in `kernel/src/cap.rs`; tests and proofs pick small ones.

## Rights only narrow

The one operation on the delegation path is intersection. `derive` refuses a request for rights the
source does not hold rather than clamping it, because a loader that quietly grants less than was
asked is a loader nobody can reason about. If delegation could widen authority, anyone could hand
themselves the rights they were denied by passing a capability to themselves.

- `GRANT` is the right to pass a capability on. It makes delegation a decision rather than an
  accident. Unix has no equivalent, which is why a child inherits every fd by default and
  `FD_CLOEXEC` had to be added afterward.
- `ENUMERATE` is the right to learn what exists, the kernel twin of the filesystem's directory
  `ENUMERATE`. Listing is a larger power than reading something you were handed. It exists for
  milestone 126 (the `procps` package): `SURVEY` first needed `READ`, and `READ` on a supervision
  rendezvous is also what `RECEIVE` and `REAP` take. calef ruled on 2026-08-17 that a domain names
  its members and never acts on them, and one bit for three operations cannot express that.
  Only `Rendezvous` consults it. `AddressSpace` would want it for `pmap` and `MemoryRegion` for
  `free`, and both wait for a consumer that can say what it needs.
- `BIND` is the right to add a thread to a process, from milestone 812 (`std::thread::spawn` runs
  real threads in one address space) and §269 (how threads share a process) fork 1. Adding a thread
  is running code inside the process, so it is an authority that can be withheld and narrowed.
  Zircon's `ZX_RIGHT_MANAGE_THREAD` is the same shape.
- `ALL` is every defined bit, and `from_bits` masks against it. A right missing from `ALL` is
  dropped at every delegation, so a new constant must widen the mask.

## Minting outside `derive`

`MemoryRegion::SPLIT` mints a child budget's capability without going through `derive`, so the
proof that `derive` never widens rights does not reach it. `MemoryRegion::RETYPE` and every
`RETYPE_OBJ` arm mint the same way, from milestone 824 (a retype mints no right its budget lacks).
All of them go through `Cap::mint_child`, which takes no rights argument and copies the parent's.
`split_never_widens_rights` proves the child's rights equal the parent's, which implies the subset
relation the rest of the model is stated in.

Equality is the precise claim because of §16's amendment, which has `SPLIT` pass `GRANT` down only
when the parent holds it, so delegability comes from the root's mint. `memory_region_root_cap` mints
once with `Rights::ALL`, `SPLIT` inherits, and `CAP_INSERT` narrows on the way into a child. Rights
down a budget tree never increase, and a spend-only (`GRANT`-less) budget cannot split itself a
`GRANT`-bearing child.

## Reap and survey

`reap_decision` is the whole authorization of `rendezvous::REAP`, as pure logic the kernel calls
(`sched::reap_supervised`) before it reclaims. It takes no `Cap` and returns none, so a supervisor
gains no authority by reaping. The supervision check comes first, so a live stranger and a stale
name give the same answer. Only inside the relationship does liveness change the answer.

`survey_includes` scopes `rendezvous::SURVEY` (`sched::survey_supervised`) with the same predicate.
The threads a supervisor may see are the threads whose deaths arrive on its rendezvous, so the view
cannot drift from the relationship. Liveness is not an input, so `ps` shows a dead child not yet
collected. The proof runs both ways: a stranger is never shown, and every member is, so a missing
entry means the thread is gone rather than hidden.

## Two doors, and the counts they keep

Besides the slots, a table records which are free, how many are occupied and the most that ever
were. Each is a second record of what the array says, worth having only if it cannot drift. So the
fields are private to one module, which changes a slot only through `fill` and `empty`. Every
mutator outside it (`insert`, `insert_at`, `put`, `derive`, `delete`, `delete_matching`) is written
in terms of those two, so a new one cannot forget the bookkeeping: the compiler refuses the field
access. That is rung one of the ladder in `AGENTS.md`.

The free-slot word has bit `i` set when slot `i` is empty. Set-means-free makes `insert`'s first
free slot one `trailing_zeros` with no inversion, and keeps bits at and above `N` clear, so a full
table is a zero word. It replaced a linear scan that LLVM unrolled once per slot, which grew the IPC
fastpath with every slot (`script/fastpath-footprint`). The word is a `u64` (milestone 754 (the
capability table grows to 64 slots)), so a table holds at most 64 slots and a larger one does not
build. A 64-slot table rounds to 2,064 bytes, eight more than the slots, word and counts need.
Growing past 64 is a wider word or a second one, and
`design/roadmap/0778-capability-tables-sized-per-process.md` is the question to answer first.

`delete_matching`, the revocation sweeps' primitive, walks the occupied mask rather than `0..N`. A
sweep runs over every thread's table for every frame a revocation touches, so its cost belongs on
what a table holds, not its capacity. It reads each slot once rather than a `get` and `delete` pair.

`used` is kept by the doors rather than counted on demand, and `peak` is the largest `used` ever
held. `peak` never falls: a mark that dropped when the table drained would report what a boot is
holding, not what it reached. The crate also keeps one global, `PEAK`, packing `peak << 16 |
ceiling` into an `AtomicU32` so the pair is read from one instant. It lets the kernel's idle loop
see that the mark moved without walking 256 thread tables under the scheduler lock. `fetch_max`
suffices because the peak sits in the high half. A `static` cannot be keyed by a const generic, so
`PEAK` does not say which table set it; the kernel has exactly one instantiation, and
`kernel/src/sched.rs` scans for the owning thread once, only when it is about to print.

## The proofs

The Kani harnesses quantify over every input: `derive_never_widens_rights` covers all 2^32 source
and requested rights at once. They run under `cargo kani` (`script/verify`), not `cargo test`.

The table harnesses use a three-slot table and carry `#[kani::unwind(4)]`, which is N + 1. The
builder and the folds loop over `0..3` exactly. The sweep's bit-scan clears one occupied bit per
iteration over a mask held to the N bits that exist. A bit-scan's trip count is data-dependent, so
Kani cannot read it off an induction variable; unbounded, it was still unwinding iteration 1,669 on
a three-slot table when the CI shard hit its cap. If a loop could run a fourth time, the unwinding
assertion fails, so raising N past the bound is a verification failure, not a quietly weaker proof.

`derive_never_widens_rights` states its claim in raw bits, `x & !y == 0`, rather than through
`is_subset_of`. `derive` guards on `is_subset_of`, so a consistently wrong predicate would satisfy a
claim phrased through it; swapping its operands left such a phrasing green. The stays-deleted
harness checks the storage before the accessors, so the claim rests on the array rather than on two
readers of it.

*Name: provisional, minted 2026-10-24 (UTC) by lane comments/929-crates-12, after the thing it
describes. calef names files.*
