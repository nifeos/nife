---
status: NOT-STARTED
raised: 2026-09-17
promoted_from: a-checked-direct-map-reader-for-the-acpi-walk
milestone_dependencies: none
decision_dependencies: 192
machine_requirements: none
specific_machine: none
needs_person: no
---
# 423. A checked direct-map reader for the x86 ACPI walk

Promoted from the proposal `a-checked-direct-map-reader-for-the-acpi-walk`,
filed 2026-09-17 by milestone 319, which proved the parsing half and so narrowed what the volatile
half is actually for.

The decision is
§192 (does the ACPI walk's direct-map read take a bound, and is the bound per-read or a region it holds?),
written up 2026-09-19 by milestone 435's slice-c lane because this gate named no section. It was ruled
2026-10-27 (UTC): shape 2. The walk holds one region, an extent handed over once at boot, and every
read is checked against it. The accessor is a new item at a trust boundary, so its name ships
provisional and goes through `script/names`.

Premise re-checked 2026-09-19 and still true, and this block subsumes milestone 431 (acpi).
`kernel/src/arch/x86_64/machine.rs` is 847 lines with no bounded accessor: `read_acpi` still takes a
firmware-supplied hint and each table body is still a raw pointer plus a length the table itself
supplied. `crates/machine_discovery` is proved (21 harnesses, its own `script/verify` row), so the
narrowing this block is built on holds.

**Milestone 431 was filed a day later restating the pre-319 framing** (that the parsing half carries
no harnesses and appears in no `script/verify` row, which stopped being true on 2026-09-17), and its
surviving claim is this block's. It is `SUPERSEDED` by this one and by milestone 319.

## In brief

Milestone 304's lane named `kernel/src/arch/x86_64/machine.rs`'s ACPI walk as its strongest
remaining target and declined to take it. 836 lines over firmware-supplied lengths, checksums and
counts. Its volatile half reads raw pointers into the direct map. It called finding that seam a
design question rather than a mechanical one, and was right.

**Milestone 319 changed the size of the question rather than answering it.** With
`crates/machine_discovery` proved, every decision about what those bytes *mean* now lives in a crate
Kani reaches. What is left on the kernel side is one operation repeated. Read N bytes at a physical
address through the direct map and hand them to a parser. The 836 lines are mostly the walk's
sequencing, and the part that is a trust boundary is a single accessor.

## The question

**Does that accessor take a bound, and what does it do when the bound is exceeded?**

Today each read is a raw pointer plus a length the table itself supplied. An RSDT whose `length`
field says 64 KiB is a 64 KiB read at whatever physical address the RSDP pointed at. Nothing
between the firmware's number and the dereference says how much of the direct map is legitimately
readable.

The analogous question is answered on the other two architectures. `dtb::Dtb::from_ptr` takes the
blob's own length, validates the header before anything else is read. Every reader below it is
proved total (`be32_is_total`, `be64_is_total`, milestone 18 (verify the capability core)). The x86
walk has no equivalent,
because ACPI is not one blob. It is a linked structure of independent tables, each reached from a
physical address inside another one.

## What is in scope

- A bounded read against the direct map's own extent, so a firmware length cannot name memory
  outside it.
- Whether the bound is per-read or a region capability the walk holds, which is the part that could
  go either way and is why this is a decision.
- The accessor's name, which is an architect's and which this proposal deliberately does not guess
  at.

## What is out of scope

The parsers. They are proved (milestone 319, fifteen harnesses), and three defects came out of that;
nothing here should re-litigate what a table means.

## What is blocked until it is answered

Nothing is blocked. This is the last unproved reach of the x86 discovery path, and it is the one
place left where a firmware number reaches a dereference with no proved step between.

## Index row

Milestone 304 named `kernel/src/arch/x86_64/machine.rs`'s ACPI walk as its strongest remaining
target and declined it. 836 lines over firmware-supplied lengths, checksums and counts, whose
volatile half reads raw pointers into the direct map. Milestone 319 changed the size of that
question rather than answering it, by proving `crates/machine_discovery`. Every decision about
what the bytes mean now lives where Kani reaches and what is left on the kernel side is one
operation repeated. Read N bytes at a physical address through the direct map and hand them to a
parser. The question is whether that accessor takes a bound and what it does when the bound is
exceeded. An RSDT whose `length` field says 64 KiB is a 64 KiB read at whatever the RSDP
pointed at. The other two architectures answer it: `dtb::Dtb::from_ptr` takes the blob's own length
and validates the header before anything is read. Whether the bound is per-read or a region
capability the walk holds is the part that could go either way, and the accessor's name is an
architect's.
