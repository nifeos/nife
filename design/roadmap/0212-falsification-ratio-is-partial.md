---
status: BUILT
raised: 2026-08-31
built: 2026-09-01
---
# 212. `script/falsifications` walks `crates/` only, so the ratio it prints is not the tree's

Built.

What it found. The walk comes from `cargo metadata` now, and the correction is smaller than the
block expected and worth reporting as measured rather than as feared. 141 harnesses in 24 crates
became 145 in 26 packages, and 25 replayable (18%) became 27 (19%). `crates/` held 97% of the
harnesses. The number was still a claim about a scope nobody had stated, which is the defect, and
three things follow from fixing it. A file's module path now comes from the Cargo target it belongs
to rather than from counting path components, because `components/src/printenv.rs` is a `[[bin]]` root and
contributes no module segment where `crates/page_tables/src/sv39.rs` contributes `sv39`. `--sweep` derives
two package-shaped flags rather than listing them: `--bin` for a package of many binaries, and
`--ignore-global-asm` for a package containing `global_asm!`. And a falsification record can exist
for something no sweep can run, which is told apart from rot mechanically rather than by a hardcoded
exemption; see the BUGS below and notes/falsification.md.

Two records were written here rather than deferred, both to answer this block's own "decide what
a kernel record costs before promising one". Milestone 193's two kernel harnesses now carry blocks:
one `unfalsified`, and one `replayable` against milestone 142's MAJOR 4 itself. A kernel *Kani*
falsification costs an ordinary sweep entry, 3.1 seconds; a kernel *test* falsification stays
unsweepable and stays milestone 210's (no kernel test can be run by name).

`script/lint`'s `kani-harnesses` and `harness-crates` counts carried the same defect and are
rescoped in the same change, because leaving them would have put two derived answers to one question
in the tree, 141 and 145.

In brief, and written before the work, in the present tense it was written in. DECISIONS §134
(a harness carries a machine-replayable falsification record) says the `unfalsified` count is
*"the claim's honest denominator"*. It was the wrong denominator.

`script/falsifications` walks `crates/` and nothing else. So:

- Milestone 197's record and patch live under `user/` and are **not counted**.
- Milestone 193's two kernel harnesses, the first proofs ever written over `kernel/src`, **carry no
  `Falsification:` record at all**, and nothing reports their absence.
- Milestone 202's `kernel/falsifications/` directory exists on `main` and nothing sweeps it.

The number is printed as a fraction of the tree and is a fraction of one directory, and the
script does not know the difference. That is worse than an undercount: §134's whole argument is that
a claim about proofs must not rest on a written claim about proofs, and this is a mechanical claim
resting on an unstated scope.

## What it needs

Derive the walk from `cargo metadata`, the way `script/lint`'s verify-table check already does.
A hand-maintained directory list is the same defect one level up, and `script/verify` has recorded
that failure twice: `mdns_proto` and then `jh7110_trng`, both carrying harnesses nothing ran.

Teach `--sweep` that a package can have binaries. It shells `cargo kani -p <crate>`, which for
`user` selects all 68 programs. Milestone 197 solved the same problem for `script/verify` by deriving
a `--bin` list from a grep rather than writing one down, *"because a list would be one name short the
first time somebody adds a harness to a 69th program"*. The sweep wants the same treatment.

Decide what a kernel record costs before promising one. A kernel falsification cannot be swept
until milestone 210 (no kernel test can be run by name) lands, so the kernel rows may have to count
as a known gap rather than as records for now. Saying so explicitly is better than a denominator that
quietly excludes them.

## BUGS

- The ratio was expected to get worse and went up by a point. Of the four harnesses the old walk
  could not see, one was already `replayable` (milestone 197's) and one was made so here, so 25 of
  141 became 27 of 145. The prediction was not wrong about the direction of the argument, only about
  its size, and the honest reading is that `crates/` held 97% of the harnesses all along.
- **It does not fix the kernel *test* sweep**, which is milestone 210's (no kernel test can be run by
  name). Milestone 202's record under `kernel/falsifications/` is now reported by name as a record
  nothing can replay, counted in neither half of the ratio, rather than being invisible. Kernel
  *Kani* harnesses do sweep, and one now does.
- **A patch that declares no `Falsifies` target and has no harness is reported as rot**, which is
  right for a patch that rotted and wrong for a future non-Kani record whose author does not know the
  convention. The message names the convention; nothing teaches it before the failure.
- A harness in a `#[path]` module of a binary would get a wrong patch path. The module path comes
  from the Cargo target, and a `#[path]` module contributes whatever the including file calls it.
  `user/src` holds two such files today and neither carries a harness; `--check` reports the mismatch
  rather than accepting it silently.

## Follow-on

- **Milestone 210.** A kernel *test* falsification stays unsweepable, because no kernel test can be
  run by name, and a sweep that runs the whole suite per record is not a sweep. Milestone 202's
  record under `kernel/falsifications/` is now reported by name as a record nothing can replay,
  counted in neither half of the ratio, rather than being invisible. A kernel *Kani* falsification
  does sweep, at 3.1 seconds, and one now does.
- **Recorded.** `notes/falsification.md` and this block's BUGS carry it. A patch that declares no
  `Falsifies` target and has no harness is reported as rot, which is right for a patch that rotted
  and wrong for a future non-Kani record whose author does not know the convention. The message
  names the convention; nothing teaches it before the failure.
- **Recorded.** `design/roadmap/0212-falsification-ratio-is-partial.md` names the residual in the
  module-path derivation. A harness inside a `#[path]` module of a binary would get a wrong patch
  path, because the path comes from the Cargo target and a `#[path]` module contributes whatever the
  including file calls it. `components/src` holds two such files and neither carries a harness
  today, and
  `--check` reports the mismatch rather than accepting it silently.

## Index row

Found by milestone 197 (user/ and xtask are out of reach of the prover)'s lane. §134 calls the
`unfalsified` count the claim's honest denominator,
and it is currently the wrong one. Milestone 197's record under `user/` is uncounted, milestone
193's two kernel harnesses carry no record at all, and milestone 202's `kernel/falsifications/` is
swept by nothing. The number is printed as a fraction of the tree and is a fraction of one
directory. The walk should come from `cargo metadata`, the way `script/lint`'s verify-table
check already does, since a hand-kept list is the defect that hid `mdns_proto` and `jh7110_trng`.
Built from `cargo metadata`: 141 harnesses in 24 crates became 145 in 26 packages and 25
replayable (18%) became 27 (19%), so `crates/` held 97% of them and the defect was the unstated
scope rather than the size of the omission. Module paths now come from the Cargo target (a `[[bin]]` root contributes no segment), `--sweep` derives `--bin` and `--ignore-global-asm` rather
than listing them, and a record for something no sweep can run is told apart from rot
mechanically. `script/lint`'s `kani-harnesses` and `harness-crates` had the same defect and were
rescoped with it.
