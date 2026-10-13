---
status: NOT-STARTED
raised: 2026-08-23
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 160. Review the public function names across the kernel's dependency crates

calef, 2026-08-23: *"Let's create a milestone to review the pub fns for the kernel crates."* This is
the crate-naming review one level down, below the crate names that milestone 158 (build the eleven
kernel object and identifier renames) and the crate renames settled. There is no fork to decide,
only a large amount of one-at-a-time review work, the same shape milestone 115's naming discipline
already uses.

## What was measured before minting this

Checked directly, not assumed: across the 47 crates the kernel depends on (its 46 direct
dependencies plus `glob`, the one crate one level deeper that is not already a direct dependency
itself), there are 1,753 `fn` items total, of which 681 are `#[test] fn` and 392 are
`pub fn`.

The 681 tests are explicitly out of scope: this tree already has a deliberate, different naming
discipline for them (descriptive-sentence names, e.g.
`the_fault_slot_is_inside_the_cspace`), and nothing in `script/names` or the naming tenet has ever
claimed authority over test names. The 392 `pub fn` are the actual public API surface and are what
this milestone reviews.

## What this milestone extends, and the honest scope question

CLAUDE.md's naming tenet, as written, states its scope narrowly: *"calef names the crates, the
programs, and the shared modules"* and *"the name of a crate, a program, or a shared module is
calef's call, not a lane's and not yours."* Nothing in that text, nor in `script/names`'s
provenance mechanism, currently extends to individual function or method names. This milestone is
itself the record of that extension, at calef's direct request on 2026-08-23, rather than a silent
scope creep discovered later. CLAUDE.md gained one sentence recording it in the same breath this
milestone was minted.

## Sequencing

**Re-measure before starting, not from this milestone's own count.** The four crate-rename lanes
minted the same day (`names/isa-machine-discovery`, `names/kernel-core-primitives`,
`names/display-crates`, `names/protocol-crates`) move functions between crate directories without
changing their signatures; once they land, the 392-count above is stale in *which crate* each
function lives in, even though the total should not move. Wait for those four to merge before
starting the walkthrough, so a function is reviewed under its final crate name.

392 is a lot more than the 24 crate names just reviewed. Doing this one function at a time, the
discipline this session used throughout, will take many passes. Two shapes worth considering when
the work actually starts, left as a judgment call for whoever begins it rather than decided here:

- Prioritize by exposure, not alphabetically: crates nearest the syscall boundary and most
  widely depended on (`abi`, `capability`, `inter_process_communication`, `paging`) are the ones a reader meets first and
  most often, so a naming problem there costs more than one in a narrowly-used crate.
- **Batch mechanically-fine names**, the same way this session batched (or offered to batch, and
  calef chose to walk individually) the nine `_proto` crates: a crate whose `pub fn` set already
  reads as a consistent, well-named API (verbs on nouns, no unexplained abbreviations) can likely be
  confirmed faster than one function at a time, reserving the per-function walkthrough for crates
  where a first pass finds a real problem.

## What this does not decide

Whether every one of the 392 gets calef's individual attention, or whether some crates' fn surfaces
get a faster batch pass, is left to how the work actually goes -- the same "recommend on reversible
forks" principle CLAUDE.md already states, since a function name is markedly more reversible than a
crate name (one `use` site inside the crate itself typically, versus every consumer across the
tree).

## What it unblocks

Nothing else is gated on this. It exists to carry the naming discipline this session applied to
crates one level further down, and to record honestly that doing so required extending a written
rule's stated scope rather than assuming it already covered this.

## Index row

calef asked to extend this session's crate-naming review one level down: 392 `pub fn` across the
kernel's 47 dependency crates (1,753 total functions, 681 of them tests and out of scope). Extends
the naming tenet's stated scope from crates/programs/shared modules to functions, recorded rather
than silently assumed. Wait for the four in-flight crate-rename lanes to land first.
