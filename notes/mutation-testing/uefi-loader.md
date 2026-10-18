# `uefi_loader`: a score measured over a file nothing compiled

This appendix of [notes/mutation-testing.md](../mutation-testing.md) holds the 2026-09-04 finding
that `uefi_loader`'s 15% measured a file nothing compiles, and the unviable mutant left unviable on
purpose.

## 2026-09-04: `uefi_loader`'s 15% was measuring a file nothing compiles

[The 2026-09-03 sample](first-weekly-runs.md) names `uefi_loader` as one of the two crates
carrying the tree's fall from 92.4%, at 3 caught, 17 missed in the round-robin sample. The crate
boots xenon, a machine where a fault has no console and no debugger, so it went first.

### The number was arithmetic, not a finding

It is the `system_initializer` result again. Milestone 244 (the largest crate in the tree is proved
by nothing a mutation can reach) found that crate scoring 0 of 191 because the host suite could not
compile a line of it. This is the same failure one level down, at a target rather than a crate.

`uefi_loader`'s `[[bin]]` carries `required-features = ["uefi"]`. So `cargo build --workspace` and
`cargo test` never put `src/main.rs` in the build graph. cargo-mutants does not read the build graph.
It edits the file, nothing rebuilds, the tests pass, and every mutant is recorded MISSED. The tell is
**"0s build + 0s test"**, printed in the output nobody was reading.

The whole-crate run on 2026-09-04, before any change:

| | mutants | caught | missed | unviable | killed |
|---|---|---|---|---|---|
| `uefi_loader`, whole crate | 189 | 32 | 156 | 1 | **17.0%** |
| of it, `src/main.rs` (never compiled on the host) | 154 | 0 | 154 | 0 | 0% |
| of it, `src/handoff.rs` + `src/image.rs` (the pure half) | 35 | 32 | 2 | 1 | **94.1%** |

The half the design lifted out to be host-testable was at 94% the whole time. The crate-level 15%
was 154 mutants in a file the tool was alone in reading. The published rate had `uefi_loader`
standing for "a subsystem nobody tests" when it stood for a measurement bug.

### The residue is real and is not fixed by excluding it

`src/main.rs` is 790 lines that call firmware and leave long mode. The only thing that proves it is
`cargo xtask uefi-boot` under OVMF, on `script/test`'s own leg. `load`, `say_conflict` and
`find_screen` carry logic (66, 28 and 9 mutants) that a host test could reach if lifted the way
`handoff` and `image` were. Whether that is worth doing is
`design/roadmap/0381-the-uefi-loaders-firmware-half-is-proved-by-one-boot.md`. Excluding the file
makes the number honest, not the file proved.

### The two real survivors, and the hole beside them

- `physical_span`'s `first & !(page_size - 1)`, twice (`- with +`, `- with /`). Every test started
  its lowest segment on a page boundary, where that mask is the identity. Both mutants clear bits
  the inputs never had set, and a segment starting mid-page separates them. The property is
  load-bearing: the firmware is asked for this range with one `AllocatePages(AllocateAddress)`,
  which takes a page number. A span beginning above the segment's first byte asks for memory that
  starts after the bytes about to be written into it. Killed by
  `a_segment_starting_mid_page_pulls_the_span_down_to_its_page`, verified by applying both
  mutations and watching that named test fail.
- `parse`'s one mutant is unviable, and that hid the fact that nothing called `parse`. See the next
  section.

After: 35 mutants, 34 caught, 0 missed, 1 unviable, **100% of viable**. No equivalents claimed and
nothing deferred.

(Corrected 2026-09-24: the 2026-09-21 census, run 35589550926, measured `uefi_loader` at 356 viable,
48.9% killed, 182 missed. It had been 34 viable at 100% on 2026-09-19.)

`script/lint` now derives from `cargo metadata` that every target with `required-features` is
excluded in `.cargo/mutants.toml`. That is the milestone 244 gate's shape, applied to the question
its dependency-graph derivation cannot see. Exactly one target matched on 2026-09-04; the gate
exists for the second one.

### The unviable mutant, and the case for leaving it unviable

`uefi_loader::image::parse`'s only mutant is `Ok(Default::default())`. `elf::Elf` has no `Default`,
so cargo-mutants has said nothing at all about that function since it was written. This is exactly
the shape of milestone 250 (an unviable mutant is a hole in the measurement that reads as a pass).
It came with the classic symptom: nothing in the tree called `parse` either, so there was no test
for the tool to have failed.

Milestone 246 (measured boot's refusal path is tested by nothing, and one mutant turns it off)
derived `Default` on `Verdict` and turned the hole into a kill. That is the wrong move here.
`Verdict` is a data struct whose default is both the fail-safe value and the dangerous wrong answer.
`Elf` is a validated token. Its own doc says *"an `Ok(Elf)` has nothing left to check; every later
accessor and `segments` iteration step trusts this pass completely."*

A `Default` impl would make an unvalidated `Elf` constructible by every consumer in the tree. That
is rung one of AGENTS.md's ladder run backwards, to buy one mutant on a two-line `map_err` wrapper.
250's own BUGS section asks whether a default is a value the code could plausibly be wrong with.
Here it is, and the objection is the cost rather than the meaning.

What was done instead is what the mutant would have asked for. Two tests now call `parse`.
`an_accepted_image_arrives_with_its_segments_and_its_entry` asserts what an `Ok(Default::default())`
would violate: the accepted image comes back carrying its segments and its entry. A wrapper that
returned an empty `Elf` now fails on the host rather than booting a machine into a kernel with no
segments. The mutant stays unviable, a recorded hole with a test standing where it would have stood.
