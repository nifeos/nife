# Decisions 1 to 4: the case behind each ruling

An appendix to [the milestone](../1101-memory-regions-outside-review.md). The rulings themselves are
in the milestone's "Held decision" and "Decisions for the architect".

## Decision 1: Whether `memory_regions` leaves nife, and in what shape

Ruled 2026-11-19 (UTC): option A.

Background. `memory_regions` is the kernel's bookkeeping for untyped memory: each region has a page
budget, a watermark that only moves down when nothing is live above it, per-kind object ceilings
and a claim that lets exactly one caller destroy a region. Its one consumer is the kernel, which
reaches it by path. Its vocabulary is the kernel's: `ObjectKind` lists seven nife kernel object
kinds and the table counts nothing else. The standing rule lets a crate leave only when it is
publishable or used by more than one repository.

- A. Hold the move until the crate has a second consumer or its object kinds are the caller's to
  define, whichever comes first.
- B. Leave now with `Counted` generic over the caller's kind set (a `Kinds` trait the kernel
  implements with its seven), so the published interface names no nife kernel object.
- C. Leave now as it stands, with nife's seven kinds in the published interface.

Door: one-way for B and C. Creating a repository and publishing are outward steps, and the
crate's shape on arrival is what a second consumer builds on. A changes nothing outside the tree.

Prior art. seL4 fixes its object types in the kernel and ships them to user space as constants in
`libsel4`, not as a library another kernel reuses. Linux's `page_counter` is a generic
hierarchical counter that lives inside one tree. `slotmap` and `generational_table` are generic over
the value they hold (all recalled, not re-read). nife differs in counting kernel objects against a
budget tree in a library at all. Concrete example: a hypervisor accounting guest pages would call
`retype_object_page(name, ObjectKind::Rendezvous)` for a thing that is not a rendezvous, or fork
the crate; under B it calls `retype_object_page(name, MyKind::Vcpu)`.

## Decision 2: Whether region names cross the crate's boundary as `u64` or as a typed name

Background. The dependency's whole point, in its own words, is that a name carries its table's entry
type so no caller can hand a name from one table to another. A crate whose public methods take and
return a bare `u64` undoes that at its own boundary. `Region` was private, so the typed name could
not appear in a signature, and the crate needed a public opaque marker. A method signature is
one-way once published.

Recommendation: typed names before the first release. `pub struct RegionName(EntryName<Region>)`
with `to_bits` and `from_bits` for the syscall register, or a public zero-sized `RegionTag` and
`pub type RegionName = EntryName<RegionTag>`. The kernel converts once at its syscall boundary, as
it already converts bytes to pages there. The `u64` form is the convenient one for the kernel's
current code and nothing else.

Prior art. `slotmap` keys are a type per map (`new_key_type!`), `thunderdome::Index` and
`generational_table::EntryName<T>` carry `to_bits` and `from_bits` for exactly this boundary
(unverified detail).

Example. `table.unpin(claim.pages())` compiles today, though a name belongs where the page count
sits. With a typed name it does not.

## Decision 3: Whether "zero pages means one" is the crate's contract or the kernel's

Background. The kernel's `RETYPE` count rule says a word of zero asks for one page, because the
syscall ignored the argument before the count existed. That ruling stands and this decision does not
reopen it. A crate that carries it makes `retype_run(name, 0)` hand out a page while `split(parent, 0)`
refuses. A stranger reading `retype_run(name, 0)`
expects nothing to happen, and a published function's behavior on zero is one-way.

Recommendation: the crate refuses zero everywhere, as `split` does, and the kernel maps the syscall
word's zero to one before it calls, in the kernel's own `retype_run`. The kernel's syscall behaves as
ruled.

Prior art. seL4's `seL4_Untyped_Retype` refuses a zero object count with an invalid-argument error.
POSIX `read` with a zero count returns zero and moves nothing (both unverified in wording).

Example. `for n in 0..3 { table.retype_run(name, n); }` retypes four pages today (one for the zero)
and three under the recommendation.

## Decision 4: Whether a child's share of its root's ceilings is the crate's constant

Background. `CHILD_SHARE_OF_ROOT` is 16, private, and every split region of every tree gets one
sixteenth of its root's ceilings, with integer division. A root ceiling under sixteen gives every
child zero (item 10). The kernel chose sixteen for the progenitor's tree. A published crate makes
that every user's policy, and a published policy number is one-way.

Recommendation: the share becomes a field of the root's `Ceilings` (or a parameter of
`insert_root_with_ceilings`), with the kernel passing 16. Zero stays zero (the stricter answer;
relaxing later is compatible). The crate keeps the invariant that no child's ceiling exceeds its
parent's.

Prior art. Linux's `pids.max` and `memory.max` belong to each group, and whoever holds the parent
sets them, not by a fixed fraction. Zircon job policies belong to each job (the wording of neither
description has a check).

Example. A root with a thread ceiling of 15 can give one child 8 and another 7 under the
recommendation; today both children get 0.
