# Review packages for the crates on journey 5

*Name: provisional, minted 2026-11-05 (UTC) by lane/journey-5-review-packages. The file name says what the page holds: the evidence an architect's crate review starts from.*

The architect reads every crate by hand before it leaves this tree
([journey 5](../design/journeys/05-crates-released-on-their-own.md), steps 2 to 8 and 10, and the
first contract crates of milestone 1017 (the contract crates leave in this order)).
This note is the index of one package per crate, so a review starts from evidence and not from a
search. Each package is a file in [`notes/crate-reviews/`](crate-reviews/README.md) and has a row
under The packages, below; a new one is a new file and a row, never a section here. Each package
follows the review of `jh7110_entropy` in
[milestone 818 (proven JH7110 TRNG logic, released on its own)](../design/roadmap/0818-the-jh7110-entropy-crate-released-on-its-own.md).
It gives the exact name and version, what the API exposes, what the proofs cover and what the crate
depends on. It also gives the provenance still owed, what the move still needs, and a
recommendation. A package gathers facts. The architect judges the API's vocabulary.

## What was run

Every result below comes from `main` as it stands.

| check | command | scope |
|---|---|---|
| packet | `script/crate-review-packet <crate>` | size, public API, dependencies, harness and patch pairing, dependents |
| names | `script/names <crate>` and `python3 helpers/name_release.py --draft <crate>` | every `Name:` block in the crate |
| standalone | `script/crate-standalone <crate>` | manifest, files, citations of this tree, build and tests at the manifest's `rust-version` (1.88) for the host and the three bare-metal targets, every Kani harness, every falsification patch |

Not run, and so not claimed: the stranger test, coverage, mutation, `cargo-semver-checks` (a first
release has no baseline) and the publish rehearsal. The template's `check`, `proofs` and `weekly`
workflows did not run either, because a repository exists for only eight of these crates:
`file_allocation_table`, `non_volatile_memory_express`, `generational_table`, `e1000e`,
`designware_mobile_storage`, `extensible_host_controller_interface`, `designware_ethernet` and `universal_serial_bus`. The
stranger record lives in the crate repository, in `docs/stranger-test.md`. The `file_allocation_table`
package is the exception: it ran the README's commands in a scratch copy and `cargo-semver-checks`
against itself.

## What is the same for all of them

- Version. Every manifest says `0.1.0`, edition 2024, `rust-version = "1.88"`, license
  `MIT OR Apache-2.0` unless a package says otherwise. The proposed first version is `0.1.0`: the move commit carries the tag `v0.1.0`, and nife depends on it the way it depends on the pilot, by `git` and `rev`
  with `version = "0.1.0"` beside it. No tag exists for any of these crates.
- Publish flag. Every manifest carries `publish = false`, and `crates/publish-allowlist.txt` lists
  no crate. The caller of the template sets `allow-unpublished` until the architect approves a
  publication. Nothing here publishes.
- Dependencies in nife today. Every consumer reaches these crates by a path dependency with a
  `version` key (the packet's dependents tables list the manifests), except `generational_table`,
  reached by `git` and `rev`. Each move converts every consumer's line to that ruled form.
- Unsafe. No crate has an `unsafe` site except `counter_frequency_contract`, whose reader names a
  raw address. Each other crate is pure computation behind a caller's volatile accesses.
- Template status. [Milestone 887 (crate repositories share one CI template)](../design/roadmap/0887-crate-repositories-share-one-ci-template.md)
  stands built: the template's workflows, the allowlist lint and the `crates-io` environment exist and a
  canary repository adopts each release. What a crate gets at its move is that template. The local
  stand-in for the template's `standalone` job is `script/crate-standalone`, whose result is in each
  package. No further check can run before a repository exists.
- README readiness. Every README has the sections a stranger looks for: what the crate does, the
  proofs with their harness names, usage, limitations and license. Every other README that names harnesses tells a newcomer how to install Kani. Three
  crates record a stranger test in their repository, and no other has run one.
- References to nife in shipped text. Crate repositories stand alone and do not mention nife, except on a line marked `// wire literal` (a wire literal keeps its bytes; ruled 2026-11-14 UTC).
  Each package lists the mentions that remain, each a small edit for the move lane.
- Harnesses and patches. In every crate, each Kani harness has exactly one falsification patch
  (the packet pairs them), and `script/crate-standalone` ran each patch and saw its harness fail.
- Units in names. The rule of 2026-11-03 spells every unit out (`_HERTZ`, not `_HZ`). A release
  review re-opens a name ratified before a later rule.

## The packages

One file each, in `notes/crate-reviews/`. The step is journey 5's.

| crate | step | recommendation |
|---|---|---|
| [`file_allocation_table`](crate-reviews/file-allocation-table.md) | 2 | cleared to move; repository exists |
| [`extensible_host_controller_interface`](crate-reviews/extensible-host-controller-interface.md) | 3 | cleared to move; repository exists |
| [`universal_serial_bus`](crate-reviews/universal-serial-bus.md) | 3 | cleared to move; repository exists |
| [`generational_table`](crate-reviews/generational-table.md) | 4 | cleared to move; repository exists |
| [`non_volatile_memory_express`](crate-reviews/non-volatile-memory-express.md) | 5 | cleared to move; repository exists |
| [`grant_plan`](crate-reviews/grant-plan.md) | 5 | ready for review |
| [`grant_contract`](crate-reviews/grant-contract.md) | contract | ready for review |
| [`domain_name_system`](crate-reviews/domain-name-system.md) | 5 | ready for review |
| [`interfaces_note`](crate-reviews/interfaces-note.md) | 3 | cleared to move (2026-11-20 UTC, no tag, no publish) |
| [`static_elf`](crate-reviews/static-elf.md) | kernel | cleared to move (2026-11-20 UTC, no tag, no publish); the public names stay provisional until the architect ratifies them |
| [`e1000e`](crate-reviews/e1000e.md) | 6 | cleared to move; repository exists |
| [`designware_ethernet`](crate-reviews/designware-ethernet.md) | 7 | cleared to move; repository exists |
| [`designware_mobile_storage`](crate-reviews/designware-mobile-storage.md) | 8 | cleared to move; repository exists |
| [`jh7110_clock_and_reset`](crate-reviews/jh7110-clock-and-reset.md) | 10 | cleared to move; the move waits for the template gates |
| [`block_roster_contract`](crate-reviews/block-roster-contract.md) | contract | ready for review |
| [`capability_manifest_note`](crate-reviews/capability-manifest-note.md) | contract | cleared to move (2026-11-20 UTC, no tag, no publish); the move waits for its place in step 6 of milestone 1017 |
| [`counter_frequency_contract`](crate-reviews/counter-frequency-contract.md) | contract | ready for the architect's review |
| [`entropy_contract`](crate-reviews/entropy-contract.md) | contract | cleared to move |
| [`clock_contract`](crate-reviews/clock-contract.md) | contract | overlay names for the architect |
| [`pci_configuration_space`](crate-reviews/pci-configuration-space.md) | 9 | ready for review; the move waits for the names |
| [`page_tables`](crate-reviews/page-tables.md) | milestone 1104 (`paging`: what an outside maintainer would fix before the move) | names for the architect, then clearance |
| [`subtree_scope`](crate-reviews/subtree-scope.md) | step 9 | eight provisional names for the architect; the move waits on P0-3's pin |
| [`network_time_protocol`](crate-reviews/network-time-protocol.md) | 1102 | cleared 2026-11-20; names ratified; move lane next |
| [`intrusive_queue`](crate-reviews/intrusive-queue.md) | 10 | cleared to move (2026-11-20 UTC, no tag, no publish) |
| [`current_cpu_contract`](crate-reviews/current-cpu-contract.md) | contract | cleared to move |
| [`domain_name_resolution_contract`](crate-reviews/domain-name-resolution-contract.md) | 6 | ready for review |
| [`socket_contract`](crate-reviews/socket-contract.md) | contract | identity, scope, names |
| [`machine_statistics_contract`](crate-reviews/machine-statistics-contract.md) | contract | cleared to move (2026-11-20 UTC, no tag, no publish); the move waits for `address_space_layout` and `current_cpu_contract` |
| [`user_mode_runtime`](crate-reviews/user-mode-runtime.md) | 3 | ready for review |
| [`memory_region_heap`](crate-reviews/memory-region-heap.md) | 9 | cleared to move (2026-11-20 UTC, no tag, no publish); the move waits for `abi`, `address_space_layout` and `user_mode_runtime` to get repositories |
| [`environment_contract`](crate-reviews/environment-contract.md) | contract | clearance for the architect (names ratified) |
| [`page_frames`](crate-reviews/page-frames.md) | kernel | cleared to move (2026-11-20 UTC, names ratified, `publish = false`, no tag, no publish) |
| [`inter_process_communication`](crate-reviews/inter-process-communication.md) | - | ready for review; the move waits for `intrusive_queue` (milestone 1097 (`intrusive_queue`: what an outside maintainer would fix before its first release)) |
| [`globally_unique_identifier_partition_table`](crate-reviews/globally-unique-identifier-partition-table.md) | 2 | blocked by its two nife type GUIDs and its unratified names |
| [`byte_glob`](crate-reviews/byte-glob.md) | contract | cleared and moved (2026-11-20 UTC, no tag, no publish); nife pins `nifeos/byte_glob` by commit |
| [`device_tree_blob`](crate-reviews/device-tree-blob.md) | 4 | cleared to move (2026-11-20 UTC, no tag, no publish) |
| [`test_and_set_lock`](crate-reviews/test-and-set-lock.md) | 1109 | ready for review |
