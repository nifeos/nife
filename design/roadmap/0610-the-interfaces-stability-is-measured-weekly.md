---
status: BUILT
raised: 2026-09-26
built: 2026-09-26
---
# 610. The interface's stability is measured weekly

calef, 2026-09-27 (UTC): "What I want are good metrics for capturing the stability of our
interface." *(Number provisional until the merge queue lands it.)* The metrics page gains a weekly
series for it, in the shape milestone 415 (sub-tripwire drift accumulates across baseline saves)
set in PR #1375: a helper, a CSV under `notes/project-metrics/`, a chart and a generated line.

## What shipped

- `helpers/interface_stability.py` (name provisional) reads each week's public API of the contract
  and std ABI crates from rustdoc's JSON (`--output-format json`, the pinned nightly, no new
  dependency), over a `git archive` of the week's tree. It writes
  `notes/project-metrics/interface-stability.csv`, back-filled to 2026W29. Five measures:
  - breaking changes and additions to the public API, with the std ABI broken out;
  - syscall surface changes: renumbered or removed, renamed with the number kept, and added;
  - format bumps, derived from `*VERSION*`, `*MAGIC*` and `*REVISION*` constants;
  - amendments, from a ledger at the format;
  - co-change, by the method behind #1389's 76%.
- A per-crate view, `interface-stability-crates.csv`, and a generated appendix listing it and this
  week's breaks by name.
- `script/metrics` carries the cells (it may not build, so it never recomputes them), draws
  `interface-stability.svg` with a four-week trend, and writes the headline line.
- `.github/workflows/metrics.yml` runs the helper before `script/metrics --update`. It already
  commits `notes/project-metrics.md` beside the directory (#1375's fix), so nothing it writes is
  dropped.
- `--selftest`, 40 fixture cases with no git and no cargo, runs in `script/lint`.
- [notes/interface-stability.md](../../notes/interface-stability.md): what the interface is,
  checked against the tree, what each column counts, the first reading, a threshold proposal, and
  the blind spots in its BUGS section.

## Proposals for calef

- PROPOSED: the `# Amendments` ledger convention, a dated list in a format crate's module
  documentation. Provisional; `capability_manifest_note` should carry the first entry when its in-place
  amendment of version 1 merges.

Ruled 2026-10-28 (UTC), on the threshold proposal above: the policy is to accommodate change rather
than wait for a lull. Frozen identifiers and additive growth are review discipline, not a gate:
nobody renumbers or removes a syscall number, nobody changes a format in place (bump or amend), and
growth happens by addition. This series reports violations weekly. The four-week-zero reading (a
crate settled after four weeks with none) is a reported interpretation on the metrics page and
gates nothing. The "at most four contract breaking changes" proposal is dropped: it has no
remaining consumer now that the per-package co-change column and milestone 756 (process-tools,
core-tools and disk-tools are the first code to leave this repository)'s re-aimed condition exist.
Co-change stays reported, not thresholded.

Names provisional: the helper, the note, both CSVs, the appendix, the chart, every
`interface_*` column and the ledger heading.

## Follow-on

- **Recorded.** The threshold for "stable" is calef's call; the proposal and its reasoning are in
  `notes/interface-stability.md`, "What stable would mean", and the PROPOSED entry above.
- **Recorded.** The amendment ledger convention and its first owed entry (`manifest_note`'s version
  1, amended in place on 2026-09-27 on a branch not yet merged) are in `notes/interface-stability.md`,
  "The amendment ledger". Nothing checks that an in-place amendment has an entry; that is in the
  same note's BUGS.
- **Recorded.** Every blind spot of the five measures, including that only the aarch64 view of each
  crate is read (a `cfg(target_arch)` item for riscv64 or x86_64 is not seen), is in the BUGS
  section of `notes/interface-stability.md`.
- **Refused.** `cargo-semver-checks` and `cargo-public-api`. rustdoc's JSON already answers whether
  the public surface moved; the tools answer whether a change needs a major version, and each would
  be a §46 (thin primitives or whole subsystems) dependency tied to the nightly pin. Reasoning in
  `notes/interface-stability.md`.
- **Done.** The per-package co-change column that §235 (the OS is built and updated from
  packages, and its source divides by purpose)'s Fork 3 amendment asks for: a home is
  sized by what changes together. `interface-stability-packages.csv` holds one row per
  `packages/*.package.toml` package and per Cargo package each week, by the division method, and
  `interface_package_*` carries the weekly share to the metrics page. The appendix lists each
  package's partner and the release-track crates; the method and its blind spots are in
  `notes/interface-stability.md`, "Co-change per package". Milestone 857 (crates released on their
  own) carries the eight crates' reading in its appendix. Branch `per-package-co-change`.
- **Recorded.** Ruled 2026-10-28 (UTC): `glob` and `machine_statistics_contract` join the per-crate
  measured set (`helpers/interface_stability.py`'s contracts list; the ruling is on #2851). The CSVs
  take no hand edits; the weekly metrics job regenerates them, so the two crates' rows appear at the
  next weekly run.

## Index row

The interface a program is written against now has a weekly series on the metrics page: the
contract crates, the syscall surface in `abi`, the wire formats and the std ABI. It is back-filled
from git to 2026W29 by reading each week's public API from rustdoc's JSON. It counts breaking changes next to
additions, syscall renumbers apart from renames, format bumps and recorded amendments, and how often
a contract commit also touches another division. Its first reading is 121 breaking changes and 449
additions over the last four weeks, which is what "not yet stable" looks like in numbers. The
stability policy it serves is ruled 2026-10-28: accommodate change rather than wait for a lull, and
this series reports the week's violations of the frozen-identifier review discipline.
