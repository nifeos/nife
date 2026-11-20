---
status: BUILT
raised: 2026-07-30
built: 2026-08-02
---
# 42. Supply chain and fuzzing in CI (extends the 2026-07-30 CI audit)

Two of three legs built 2026-07-30; the decisions are DECISIONS §36. Advisories and licences:
`deny.toml` (written rather than defaulted, a reason next to every knob) run over each workspace by
`script/supply-chain`, in CI. First run found no advisories, no yanked crates and no unknown
sources. That is a result rather than a null because it is the first time anyone could say it. It
also found one duplicate (`getrandom`, host-side, under redoxfs, skipped with a reason and an expiry
condition) and three licences beyond MIT/Apache-2.0 that are genuinely needed. Two crates that needed
`publish = false` before a path dependency could be told apart from `version = "*"`.

Vendored integrity: `script/vendor-verify` hashes the published .crate, applies a committed
divergence patch with zero fuzz, and requires byte identity with the tracked tree. It found drift on
its first run, which is the argument for it. vendor/README.md claimed the published redoxfs
package ships no `Cargo.lock` and that ours was a deliberate addition. It ships one, and ours was a
regeneration that had re-resolved 25 dependencies. Nobody had edited the filesystem, and nobody could
have proved that either.

The fuzzing leg landed 2026-08-02 (notes/fuzzing.md). Four cargo-fuzz targets over the parsers
that read bytes from outside the trust boundary (`dtb`, `elf`, `gpt`, and a `nifefs` round trip),
run by `script/fuzz` and by a CI job of its own with a sixty-second-per-target budget. Fuzzing has
no completion condition and so cannot be a step inside a gate anyone waits on.

**Three bugs, and how each was found is the finding.** `dtb::Region::end` overflowed on a hostile
memory map, which the kernel's boot path calls on every RAM region. The fuzzer found that one, in
ten minutes, from a mutated copy of the committed QEMU device tree. `nifefs::write_image` accepted
a name containing a NUL, wrote it, and could never read it back. That is the same silent-collision
family as the truncation bug fixed on 2026-08-01. A round-trip property found that one, in under a minute,
and no totality proof could have, because nothing panicked. And `dtb::node_register_regions` indexed past its
16-entry cell stack on a tree nested 17 deep. **Reading the code found that one, and ten minutes of
fuzzing did not rediscover it**, because deep recursive structure is what a mutational fuzzer is
worst at synthesizing. All three are fixed and pinned by host tests that run in milliseconds.

The question the leg was held open for is answered in the note's first section. Kani is exhaustive
inside a bound and a fuzzer is unbounded and random. The three cases above show the boundary
between them is not always the bound. Sometimes it is that nobody wrote the property down.

In brief. Three things CI does not do. Advisories and licences: no `cargo-audit`/`cargo-deny`, so a
published advisory against a dependency is invisible. Licence obligations go unrecorded, which stops
being cosmetic the moment the distribution of milestone 39 (repository structure for a
loosely-coupled OS) exists. Vendored integrity: `vendor/redoxfs`
is pinned at 0.9.1 with a `patches/` discipline and *nothing verifies the tree equals
upstream-plus-our-patches*. Fuzzing the parse surface: Kani proves `elf`, `dtb` and `nifefs` under
*chosen bounds*. A fuzzer explores byte sequences past those bounds and finds panics rather than
property violations, which is complementary rather than redundant. Several crates are unproved
entirely and take attacker-shaped input. They are the `fs_proto`/`gfx_proto`/`line_editor` decoders
and `grant_plan` (which parses the human's command line). Then `compositor` (clipping arithmetic, where its
own note says off-by-one is the classic bug), and `measured_boot`, the SHA-256 behind the
measured-boot trust root

Why it matters. **the thesis is confining code we did not write, so not knowing when that code has a
published advisory is an odd blind spot**. The flagship claim of milestone 32 (a real filesystem)
("a real filesystem we did
not write") is only as good as our ability to say what we are actually running. Fuzzing is the
honest complement to bounded model checking. Kani answers "is the property true inside these
bounds", a fuzzer answers "does anything crash outside them", and the project currently only asks the first

## Follow-on

- **Refused.** Fuzz targets for the other crates this block lists as taking attacker-shaped input:
  the `fs_proto`/`gfx_proto`/`line_editor` decoders, `grant_plan`, `compositor` and `measured_boot`.
  `design/decisions/0060-fuzzing-the-parsers.md` settles it: the four targets that shipped are the
  tree's actual trust boundary, because everything on that list parses bytes this system wrote
  itself. A found bug becomes a permanent regression test, not a permanent fuzzing job.
- **Recorded.** `notes/fuzzing.md` holds what the fuzzing leg cannot do, including the one that
  contradicts the assumption. Ten minutes did not rediscover the `dtb::node_register_regions` bug, because a
  mutational fuzzer is worst at deep recursive structure. A grammar-based generator is the named
  next step for `dtb_walk`. The same file records that three of the four targets assert nothing
  beyond "it returned", so a parser that answers wrongly without panicking is invisible.
- **Recorded.** `deny.toml` carries the one duplicate the first supply-chain run found, `getrandom`
  0.2 beside 0.4 under vendored redoxfs, skipped with its reason and its expiry condition written
  beside it. It is host-side only and it goes away when the redoxfs pin advances.

## Index row

we confine code we did not write, and the parsers that read what firmware and disks hand us are
where a bound is a lie
