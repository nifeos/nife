# The live candidates against the fork test and the AI-submissions criterion

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md), measuring the
crates it names against the two criteria in
[take, fork, vendor or write](take-fork-vendor-or-write.md). Everything here was read on
2026-10-22 between 21:08 and 21:11 (UTC). Nobody upstream was asked anything.

## Method, and what the numbers mean

- Written AI-contribution policy. Read in each repository at its default branch's head: the README,
  any `CONTRIBUTING`, any `.github/` template, and the owning organization's `.github` repository
  where one exists. A GitHub issue search for AI, LLM, Copilot, ChatGPT and Claude found nothing in
  any of the six. The policy readings behind 857's fork 8 cover none of these rows: rcore-os's
  `tgoskits` is a different repository from `virtio-drivers`. *Missing* means no written policy
  exists in those places. It is not acceptance.
- CLA. A contributor license agreement the repository requires in writing. The license's own
  inbound-equals-outbound clause, as `xhci`'s README states, is not one.
- Last release. The newest non-yanked version on crates.io.
- Commits, 12 months. Commits on the default branch since 2025-10-22T00:00Z, merges included, from
  the GitHub API.
- Crate size. Code lines (blank and comment lines removed, test modules included) under `src/` of
  the published `.crate`, the counting 784's method uses.
- Our changes. Lines added plus lines removed in upstream's existing lines, as a share of the crate
  size. Proofs, fuzz targets and tests we add are not counted, as calef's refinement says. Where we
  carry nothing the share is 0. Where the change is not yet known it says so.

## The table

| Crate (repository) | Written AI-contribution policy | CLA | Last release | Commits, 12 months | Crate size | Our changes | Fork test |
|---|---|---|---|---|---|---|---|
| `elf` 0.8.0 ([cole14/rust-elf](https://github.com/cole14/rust-elf/tree/5702acc510d616aff43bfe37b0d0764d2cba899a)) | missing | none | 2025-05-14 | 4, last 2025-11-29 | 8,581 | not measured; #2583 measures it beside ours | fails 2; 3 open |
| `gpt_disk_types` 0.16.1 ([google/gpt-disk-rs](https://github.com/google/gpt-disk-rs/tree/9a78aeb63bca05b0465e1f70906618e27d0c2880)) | missing | Google's, [CONTRIBUTING.md](https://github.com/google/gpt-disk-rs/blob/9a78aeb63bca05b0465e1f70906618e27d0c2880/CONTRIBUTING.md): "Contributions to this project must be accompanied by a Contributor License Agreement." | 2025-04-01 | 21, last 2026-10-20 | 1,240 | not estimated | fails 2; 3 likely fails |
| `virtio-drivers` 0.13.0 ([rcore-os/virtio-drivers](https://github.com/rcore-os/virtio-drivers/tree/c76e09d7143f3070f81776db108ceaff7d80a8ba)) | missing | none | 2026-03-03 | 108, last 2026-10-20 | 9,458 | 0 of ours; upstream's #232 is carried, 194 lines in `src/` (151 added, 43 removed), 2.1% | passes all three |
| `xhci` 0.9.2 ([rust-osdev/xhci](https://github.com/rust-osdev/xhci/tree/f2254c86d5ba12754259bf0ae483c6c8dc821aad)) | missing | none | 2023-07-19 | 0, last 2024-09-02 | 4,266 | not known; the driver is unbuilt | fails 2 |
| `talc` 5.1.1 ([SFBdragon/talc](https://github.com/SFBdragon/talc/tree/4a547f57b75fdc766cfbe0fdb12c220cb1307be8)) | missing | none | 2026-09-09 | 46, last 2026-09-09 | 3,441 | 11 lines in `src/`, 0.3% (`vendor/talc.divergence.patch`, which is upstream's open #57) | passes all three |
| `fatfs` 0.3.6 ([rafalh/rust-fatfs](https://github.com/rafalh/rust-fatfs/tree/38b3a7950bf0e765e34286d1dfe76c519ee347e2)) | missing | none | 2023-01-17 | 25, last 2026-10-16 | 4,213 | not applicable | not weighed |

The test's conditions are numbered as calef gave them and read with his refinement. 1: our changes
to upstream's lines are small. 2: upstream has commits and releases in the last year. 3: the crate
is not one we release, and proving it does not require restructuring upstream's code.
No verdict turns on that reading: condition 2 alone decides `elf`, `gpt_disk_types` and `xhci`.

## What each row means for 784

- `elf`. No release in the last year, so a fork does not beat ours. Whether Kani needs cole14's
  parser restructured, condition 3, is open until #2583's candidate harnesses finish. Under 857's
  fork 8, contributing our proofs (its item 4) needs a policy that admits disclosed AI work, and
  none is written.
- `gpt_disk_types`. Active in commits but no release in the last year, so a fork does not beat ours.
  Condition 3 likely fails too: our own proofs needed the CRC restructured for the model checker.
  Under fork 8, contributing (857's item 3) needs a written policy, and there is none. It also
  needs a person to sign Google's CLA once.
- `virtio-drivers`. Nothing of ours to carry: `VIRTIO_F_ACCESS_PLATFORM` is upstream's #232, merged
  2026-03-17 and in no release. The crate passes all three conditions, so the test favors a fork of
  0.13.0 carrying #232 over keeping our drivers. calef chose that fork for item (c0), 2026-10-22
  21:58 (UTC), once a probe passed, and it passed
  ([virtio-drivers-probe.md](virtio-drivers-probe.md)). It drops when a release includes #232.
- `xhci`. Dormant, so a fork has nothing to merge. Item 10 vendors the register and TRB definitions
  under rule 6 once milestone 242 (USB host and HID) starts, and the driver logic is ours.
- `talc`. Passes all three, and is carried as a fork in `vendor/talc`: the 5.1.1 package plus
  upstream's own #57. Without a written policy, the take-unmodified risk line counts against it,
  and we have already needed one fix.
- `fatfs`. Commits in the last year, no release since 2023-01. This row records activity and policy
  only. FAT's adopt-or-build question is answered in
  [its own appendix](fat-adopt-or-build.md), and nothing here changes that answer.

## BUGS

- Every figure here ages. Policies especially: one written after 2026-10-22 changes a row, and
  nothing re-reads these files on a schedule.
