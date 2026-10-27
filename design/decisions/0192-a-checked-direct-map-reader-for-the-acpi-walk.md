---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-27
ratified_by: calef
---

# 192. Does the ACPI walk's direct-map read take a bound, and is the bound per-read or a region it holds?

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 423 (a checked direct-map reader for the x86 ACPI walk)'s
`DECISION` gate naming no section. Filed 2026-09-17 by milestone 319, which proved the parsing half
and so narrowed what the volatile half is actually for. *(Section number provisional until the merge
queue lands it.)*

## What is being decided

Does the accessor take a bound, and what does it do when the bound is exceeded? Two shapes are
live and they differ in more than mechanism:

1. Per-read, where each call carries a length and the accessor refuses one that leaves the
   direct map's extent.
2. A region capability the walk holds, where the walk is handed an extent once and every read is
   checked against it.

And the accessor's name, since it is a new item at a trust boundary.

## Is the premise true

Checked 2026-09-19 in this worktree. Yes, on both halves:

- `kernel/src/arch/x86_64/machine.rs` is 847 lines with no bounded accessor. `read_acpi(hint:
  u64)` at line 405 still takes a firmware-supplied hint, and each table body is still a raw pointer
  plus a length the table itself supplied.
- `crates/machine_discovery` is proved, with its own `script/verify` row, so the narrowing this is
  built on holds.

Milestone 431 restated the pre-319 framing (that the parsing half carries no harnesses and
appears in no `script/verify` row, which stopped being true on 2026-09-17) and is `SUPERSEDED` by
319 and by 423. Its surviving claim is 423's.

## What the question actually is, after 319 shrank it

Milestone 304's lane named this walk as its strongest remaining target and declined to take it,
calling the search for the seam a design question rather than a mechanical one. It was right.

Milestone 319 changed the size of the question rather than answering it. With
`crates/machine_discovery` proved, every decision about what those bytes *mean* now lives in a crate
Kani reaches. What is left on the kernel side is one operation repeated: read N bytes at a physical
address through the direct map and hand them to a parser. The 847 lines are mostly the walk's
sequencing; the part that is a trust boundary is a single accessor.

Today an RSDT whose `length` field says 64 KiB is a 64 KiB read at whatever physical address the
RSDP pointed at. Nothing between the firmware's number and the dereference says how much of the
direct map is legitimately readable.

## What this tree already does in the analogous case

The other two architectures answer it, and reading the code rather than recalling it sharpens what
they answer. `crates/dtb`'s `Dtb::from_ptr` (line 135) reads only the fixed-size header first,
checks the magic before anything else, then takes the blob's self-declared `totalsize` from that
header, and delegates to `from_bytes`, which refuses when `bytes.len() < total || total <
HEADER_LEN`. Every reader below it is proved total (`be32_is_total`, `be64_is_total`, milestone 18).

**So the device-tree precedent is not "take an external bound".** It is: validate a fixed-size
header before trusting anything, then treat the self-declared length as a claim to be checked
against a slice that already exists. That is closer to shape 1 than to shape 2, and it is a real
data point rather than an analogy.

**The reason it does not simply transfer is structural**, and it is why this is a decision: ACPI is
not one blob. It is a linked structure of independent tables, each reached from a physical address
found inside another one, so there is no single slice for a self-declared length to be checked
against. Something has to say what the walk is allowed to reach, and that something is either passed
per-read or held.

## What each shape costs

| | what | cost |
|---|---|---|
| **1, per-read** | every call carries the length; the accessor checks it against the direct map's extent | the bound is visible at every call site, which is also the failure mode: a caller that computes it wrongly is not caught by anything, so the invariant is asserted N times by hand, which §94 and §61 both name as the wrong rung |
| **2, a region the walk holds** | the walk is handed an extent once and reads are checked against it | one place to be wrong, and it reads like the rest of this system, where authority is a thing you hold. Costs a type and a hand-off the boot path does not have today |

Neither is expensive in code. What decides it is which one a prover can say something about and
which one a reader can check, and §30 is the standing example of a proof that says exactly where it
stops.

## Recommendation

Shape 2, held rather than passed, and the argument is this tree's own rather than economy. A bound
that travels with the walk is one place to be wrong. A bound recomputed at each call site is the
same invariant asserted N times by hand. That is the shape §94 (what may live in a userspace
library) refused for 58 panic handlers and §61 (a lint is adopted on evidence) refuses for
`// SAFETY:` comments.

**The recommendation is offered because the mechanism is reversible; the name is not, and no name is
proposed here.** An accessor at a trust boundary is read by every future reader of this file, and
milestone 423's own block deliberately declines to guess at it.

## Would we still choose 2 if both cost the same

Yes. Shape 1 is the smaller change (no new type, no hand-off in the boot path) and is still the one
this section argues against, on where the invariant lives rather than on effort.

## What is out of scope

The parsers. They are proved (milestone 319, with three defects found in the doing), and nothing
here should re-litigate what a table means.

## What was blocked

Nothing was blocked. This is the last unproved reach of the x86 discovery path, and the one place
left where a firmware number reaches a dereference with no proved step between. Milestone 423 is
unblocked by the ruling above.
