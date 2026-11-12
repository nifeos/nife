---
status: NOT-STARTED
raised: 2026-09-20
promoted_from: does-one-nife-crate-parse-under-verus
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 529. Does one nife crate parse under Verus at all, and can a verified crate build for a bare-metal target?

*(Number provisional until the merge queue lands it.)* Promoted from the proposal `does-one-nife-crate-parse-under-verus`, filed 2026-09-20, on calef's instruction of 2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own, unedited except for this paragraph. The argument is its author's and promotion is not the moment to improve it. Raised by the `maintainer/verus-versus-kani` lane while writing
`notes/verus.md`, which priced Verus against Kani for `design/fatal-risks/README.md` risk 2 (the proofs
prove trivia, and the real bugs live where Kani cannot reach) and could not answer this.

It needs no decision, because it decides nothing. It is a measurement, and its whole
point is to be cheap enough that nobody has to approve it. Adopting Verus *would* be a DECISION
(AGENTS.md puts a dependency and a methodology in calef's hands), and this proposal is deliberately
not that.

## What it is

`notes/verus.md` established, by running both tools, that Verus stops at the same hardware boundary
Kani does. It rejects `asm!` (*"The verifier does not yet support the following Rust feature:
inline-asm expressions"*) and rejects the integer-to-pointer cast that is the MMIO idiom (*"Verus
does not support this cast: `usize` to `*const u32`"*). What it can do that Kani cannot is reason
without an unwind bound. That is the wall every hard case in `notes/verification.md` and
`notes/user-proofs.md` actually hit.

That makes the interesting question a plumbing question rather than a proof question. The note's
`BUGS` says plainly that it is unanswered: **not one line of this tree was put through Verus.** Two
things are unknown, and either could be fatal to the idea on its own:

1. Does nife's Rust parse under Verus's subset? The guide's feature table lists `transmute`,
   user-defined `Drop`, hardware intrinsics and the standard `Mutex`/`RwLock` as unsupported. It
   lists `as` casts, `dyn`, `const` generics, iterators, closures with mutable captures and multi-crate
   projects as only partially supported. This tree leans on most of that list.
2. Can a verified crate build for `aarch64-unknown-none-softfloat`? Verus pins Rust 1.98.1
   stable and this tree pins a nightly with nightly features. The Verus project documents neither a
   custom target JSON nor bare metal. The only evidence either way is the Atmosphere authors
   asserting it about their own x86_64 kernel.

## The smallest honest version

One crate, `verify` only, no proof written. Pick a `crates/` member that is already `no_std`,
already has Kani harnesses. It should have the fewest of the unsupported features above (`capability` and
`timetable` are the obvious candidates; the choice is part of the work). Run `cargo verus verify` on
it with an empty `verus!{}` and report exactly one thing: what it says.

That is a day at most and it is falsifiable in the useful direction. If a `no_std` pure-logic crate
with no `Drop` and no `dyn` does not parse, the whole question is closed for this tree. The note
gets a new BUGS line saying so. If it parses, the next question (what one real property costs) is
worth a lane and is not worth one before this.

**What it must not turn into**, and this is the reason the scope is written this way. A lane that
sets out to *prove something in Verus* will spend its time learning Verus and produce a proof whose
cost tells you nothing, because the first proof anyone writes in a new verifier is the expensive one.
The number this project needs is not "can a skilled Verus user prove this", it is "does the door
open". `notes/verus.md` already established that the first hour is cheap (a `run_end_virtual_address`-shaped
property, `no_std`, from a cold start knowing no Verus, verified in 0.83 seconds) and that this says
nothing about a subsystem.

## What it costs and what it leaves behind

~450 MB of Verus and one extra rustup toolchain (`1.98.1-aarch64-apple-darwin`), both outside the
tree; `notes/verus.md`'s EXAMPLES has the commands. No dependency is added to the workspace, and
none should be: DECISIONS §46 (thin primitives or whole subsystems; we write everything in between)
applies with full force to a verifier in the shipping graph. Nothing here proposes putting one
there.

The deliverable is two or three paragraphs appended to `notes/verus.md`, not a new note.

## Why not now

It is off the customer path, which AGENTS.md says is vacant, and the tie there breaks toward
`design/fatal-risks/README.md`. This is on that list only indirectly. Risk 2's red half is about reach, and
`notes/verus.md`'s finding is that **Verus does not extend the reach**, so this experiment cannot turn
risk 2 green. What it can do is close the one open question in a note that otherwise has to say "we
did not try". That is worth a day and is not worth more.

## Index row

`notes/verus.md` established, by running both tools, that Verus stops at the same hardware boundary
Kani does. It rejects `asm!` (*"The verifier does not yet support the following Rust feature:
inline-asm expressions"*) and rejects the...
