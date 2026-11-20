---
status: SUPERSEDED
raised: 2026-09-18
promoted_from: the-acpi-walk-is-reachable-and-unproved
superseded_by: 319, 423
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 431. The ACPI walk is reachable by the prover for the first time, and proved by nothing

Milestones 319 and 423 cover this block. Promoted from the proposal
`the-acpi-walk-is-reachable-and-unproved`, filed 2026-09-18, split out of milestone 304's
`## Follow-on` because the proposal 304 was promoted from had been discharged and the work it
carried had no home. *(Number provisional until the merge queue lands it.)*

A lane can start by reading the seam and proposing where it falls. Nothing has to
build or boot first, and the decision it reaches is a lane's own: where to cut a proof boundary is
reversible, and no wire format, name or system call is on the other side of it.

Premise re-checked 2026-09-19, and half of it had stopped being true the day before it was
filed. This block says the walk's parsing half "already lives in `crates/machine_discovery`, which
carries no harnesses and appears in no `script/verify` row, so it is pure logic that nothing
proves". Milestone 319 (the crate that parses firmware had no proofs) proved it on 2026-09-17. That
crate now carries 21 Kani harnesses across
`acpi.rs`, `framebuffer.rs`, `riscv64.rs` and `x86_64.rs` and has its own row in `script/verify`.
Three of its first fifteen harnesses were false, one of them a panic on the x86 boot path.

What survives is the volatile half, and it already has a block. Milestone 423, filed 2026-09-17
by milestone 319's own lane, states the same remaining work with the post-319 framing. The seam this
block calls most of the job is narrower than it looked. What is left on the kernel side is
one bounded read against the direct map, and the open question is whether that accessor takes a
bound and what it does when a firmware length exceeds it. Two blocks for one accessor would be two
places for the work to rot, so this one is disposed of and 423 carries it.

**SUPERSEDED rather than `NOT-STARTED` or `REMOVED`.** Nothing here was built and nothing was
deleted, and the work was not declined; it was answered, half by 319 and half by 423, which is what
that word is for. The number is spent either way, per milestone 433's own `BUGS`.

In brief. Milestone 304 put `kernel/src/arch/x86_64/` in front of the prover for the first time.
Its strongest target went with it and is still unproved: `x86_64/machine.rs`'s ACPI walk, 836
lines reasoning over firmware-supplied lengths, checksums and counts. It is the shape of input a
prover is for and the shape of input a machine will lie about.

**Why it is not simply a harness**, which is what makes it a lane rather than an afternoon. The walk
is two things wearing one name. Its parsing half already lives in `crates/machine_discovery`, which
carries no harnesses and appears in no `script/verify` row, so it is pure logic that nothing proves.
Its volatile half reads raw pointers into the direct map, which Kani cannot follow. The work is
finding where those two separate, and that seam does not exist yet; proposing it is most of the
job, and writing the harnesses is the smaller half that follows.

What it is worth. `design/fatal-risks/README.md`'s risk 2 is that the proofs prove trivia while the
real bugs live where Kani cannot reach. Firmware-supplied lengths and counts are the least trivial
input this kernel takes, and milestone 87's first light on xenon already turned up one machine
whose firmware map did not describe the MMIO hole. This is the risk-2 frontier with the door newly
open.

## Index row

Milestone 304 put `kernel/src/arch/x86_64/` in front of the prover for the first time and its
strongest target went with it unproved: the ACPI walk, 836 lines reasoning over firmware-supplied
lengths, checksums and counts. It is the shape of input a prover is for and the shape a machine
will lie about. This block was filed on 2026-09-18 on the premise that the walk's parsing half in
`crates/machine_discovery` carried no harnesses and no `script/verify` row. That premise was one
day stale: milestone 319 proved that crate on 2026-09-17 with fifteen harnesses, three of them false
as first written. What remains is the volatile half, one bounded read against the direct map, and
milestone 423 (a checked direct-map reader for the x86 ACPI walk) states it with the narrower framing 319 made possible. Promoted and closed in one
act on 2026-09-19, because a proposal cannot be disposed of in place and two blocks for one accessor
would be two places for the work to rot.
