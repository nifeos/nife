---
status: BUILT
raised: 2026-10-11
built: 2026-10-12
milestone_dependencies: none
decision_dependencies: 266, 282, 291
machine_requirements: none
specific_machine: none
needs_person: no
---
# 873. The catch-all userspace test file is split by topic

*(The title and slug are provisional.)*

`system_tests/src/user/tests.rs` is 3,533 lines at `396187b0b`. 
§266 (a Rust source file stays under 2,000 lines) sets the ceiling. Milestone 846 (the file-server
core moves its tests out) is the closest model, since this file is tests and nothing else.

The cut and every module name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

Reuse: not applicable; this moves code and adds none. The precedent is the 84 other files in
`system_tests/src/user/`, 73 of them a `*_tests.rs` module on one topic, declared in `user.rs`.

The lane wrote the cut up on 2026-10-12 (UTC) as §291 (the catch-all userspace test file splits
into topic files beside it). It finds each fork below already answered by the tree, and asks
calef nothing; the maintainer recorded it as settled by precedent on #1966 the same day. Two premises here were wrong. The file holds 69 tests, not 71: two
grep hits were comments. And `userspace_init_tests` would sort after `thread_leak_police`, which
polices only the modules that sort before it, so the file ships as `init_tests`.

Built 2026-10-12 (UTC): `tests.rs` is gone, eleven siblings replace it, and every test runs on the
same architectures as before, with the same counts on all three.

## What is in the file

Measured on `396187b0b` from the file's own top-level items. Ranges are approximate at the edges,
where a doc comment belongs to the item below it. The file holds 71 `#[test_case]` functions.

| lines | count | tests | what |
|---|---|---|---|
| 1 to 199 | 199 | 0 | imports, program-image helpers, `spawn_bare`, `reap_bare`, network constants |
| 200 to 359 | 160 | 5 | reaching EL0, a kernel address refused, preemption, the trap frame |
| 360 to 583 | 224 | 8 | the ELF loader: forged images refused, the initrd's own image runs |
| 584 to 929 | 346 | 3 | ASID tagging and flush, a read-only segment |
| 930 to 973 | 44 | 1 | the hardware says EL0 cannot read kernel memory |
| 974 to 1147 | 174 | 3 | shared memory, `map_physical`, a new thread holds nothing |
| 1148 to 1536 | 389 | 9 | the virtio disk, `std::fs`, the redoxfs server, per-file grants |
| 1537 to 2093 | 557 | 15 | virtio-net, DHCP, sockets, name resolution, TCP, HTTP, `std::net` |
| 2094 to 2203 | 110 | 2 | the least-authority demo, spending a memory region |
| 2204 to 2415 | 212 | 6 | DMA descriptor escapes, block writes, the PCIe transport |
| 2416 to 2618 | 203 | 2 | an address space freed, a user-built address space |
| 2619 to 2808 | 190 | 5 | userspace init |
| 2809 to 2978 | 170 | 2 | the cycle-counter grant, CoreMark |
| 2979 to 3357 | 379 | 4 | child threads, reclaim, spawn-to-reap, a composed process |
| 3358 to 3533 | 176 | 6 | rendezvous, delegation, call and reply, frame capabilities |

Most helpers in the first rows serve one topic. Seven serve more than one: `spawn_bare`,
`reap_bare`, `a_kernel_address`, `blk_image`, `outlaw_image`, `loader_subject_image` and
`std_exerciser_image`. The largest, `spawn_bare`, is 30 lines.

## The verdict: topic files, as siblings

There is no code to keep whole here. The tests-only axis that milestone 846 chose does not apply,
because everything is a test. The question is only where each topic goes.

Two layouts fit. One makes each topic a sibling module in `system_tests/src/user/`, beside the 84
files already there. The other makes `tests.rs` a parent with a `tests/` directory under it.

The directory keeps the shared helpers private to `tests`, and it adds no line to `user.rs`. That
matters, because `user.rs` is the hotter file: 26 first-parent merges touched it in the 14 days to
2026-10-11, against 11 for `tests.rs`.

The siblings win anyway. A reader looking for the network tests runs `ls` once and sees
`net_confinement_tests.rs`, `name_resolver_tests.rs` and the new file side by side. Under the
directory, the same topic lives in two places with two path shapes. New tests already land as
siblings: pull request #1892 adds `futex_tests.rs` and `process_tests.rs` rather than growing this
file.

The shared helpers then go in one small sibling, `pub(super)`. That reaches other test modules
under `user`, and nothing outside `cfg(test)`.

## A proposed cut

Every name here is provisional.

| module, provisional | from the table | about |
|---|---|---|
| `el0_boundary_tests` | 200 to 359, 930 to 973 | 210 |
| `elf_loader_tests` | 360 to 583 | 230 |
| `address_space_tests` | 584 to 929, 2416 to 2618 | 560 |
| `capability_tests` | 974 to 1147, 3358 to 3533 | 360 |
| `filesystem_tests` | 1148 to 1536 | 400 |
| `network_tests` | 1537 to 2093, the network constants | 630 |
| `block_driver_tests` | 2204 to 2415 | 215 |
| `userspace_init_tests` | 2094 to 2203, 2619 to 2808 | 310 |
| `cycle_counter_tests` | 2809 to 2978 | 180 |
| `child_thread_tests` | 2979 to 3357 | 385 |
| `test_programs` | the seven shared helpers | 100 |

`tests.rs` is then gone. No new file is near 1,500 lines. `user.rs` gains one declaration per
module, each under `#[cfg(all(test, initrd))]` as `mod tests;` is today. It ends near 1,190 lines,
far under 3,000. The doc comment above `mod tests;` in `user.rs` moves to `test_programs`.

`process_tests` is not available as a name: #1892 takes it.

## What an architect has to rule

1. Siblings, or a `tests/` directory. The recommendation is siblings, for the reason above: one
   shape for every topic file in the suite. That reason holds at equal cost. The directory's
   advantage, fewer edits to `user.rs`, is about merge timing and not about the result.
2. Whether some topics fold into an existing sibling instead of a new file. The name-resolution
   test could join `name_resolver_tests.rs`, and the composed-process test could join #1892's
   `process_tests.rs`. The recommendation is no, in this milestone: a pure move is reviewed as one,
   and a fold mixes moved lines with edited ones.
3. Every module name in the table.

No public crate API, wire format or syscall changes under any answer. The suite is a test image.

## What moving the code breaks

Found with `grep` at `396187b0b`.

- This is the test-wiring hotspot that `notes/skills/maintainer/SKILL.md` names. Expect a conflict
  with any lane open in it or in `user.rs`, and take the split when none is. The skill's line, and
  the comment in `script/audits` that names the same hotspot, need the new paths.
- Seven falsification patches carry this module in their names, `user.tests.<test_name>.patch`.
  `script/falsifications` derives the path from the file a test lives in. A test in
  `user/network_tests.rs` must have its patch at `user.network_tests.<test_name>.patch`, and
  `--check` reports a mismatch. All seven are renamed, along with the `Falsification:` line above
  each test.
- None of the seven diffs this file. Each patches the code under test: `kernel/src/syscall.rs`,
  `kernel/src/sched.rs`, the two `mmu.rs` files, `crates/paging`, `crates/elf` and
  `crates/domain_name_system`. Their hunks do not change, so no patch needs regenerating. Each must
  still be replayed red under its new name.
- The new-confinement-test ratchet in `script/falsifications` decides "new" by function name. A
  moved test keeps its name, so it passes.
- `script/stack-frame-check` sorts test frames by `_tests::` and `::tests::` in the symbol. Both
  layouts still match.
- 17 citations of the form `user/tests.rs:NNNN` sit in 11 files. Four are in the patches' own prose,
  dated records of where each went red. 30 tracked files name the path in all, and 32 name the
  module path `user::tests::`, mostly in roadmap history.
- `design/file-length-baseline.tsv` lists the file at 3,533; that row goes in the same change.
  `design/comment-block-baseline.tsv` keys two rows on this path, and both blocks move.
  `tests.rs:649` (43 lines, the doc of `an_asid_flush_reaches_the_other_cpus`) lands in
  `user/address_space_tests.rs`. `tests.rs:2767` (42 lines, the doc of
  `a_granted_thread_reads_the_cycle_counter_and_an_ungranted_one_faults`) lands in
  `user/cycle_counter_tests.rs`. The ratchet admits no row on a path the merge base gave none, and
  a block in a new file meets the 40-line cap outright. So it fails both unless each is cut to 40
  lines in the same change. The alternative is a fix to the ratchet, letting a row move with its
  block, ruled separately.
- No Kani harness is in the file.
- Pull request #1892 was in flight at `396187b0b`, as a draft. It does not touch this file, but it
  adds 28 lines to `user.rs`, adds sibling test files and patches, and edits
  `user.tests.a_process_composed_from_two_capabilities_runs_in_the_space_it_built.patch`, which
  this split renames. Re-measure every range here after #1892 merges.

## Done when

1. `system_tests/src/user/tests.rs` is gone, no new file is over 1,500 lines, and `user.rs` is under
   2,000.
2. All 71 tests run on each architecture they ran on before, with no test body changed and no
   `cfg` gate widened or narrowed.
3. `script/falsifications --check` passes, and each of the seven renamed patches is replayed red.
4. `script/test` passes on aarch64, riscv64 and x86_64.
5. Each new module carries a `//! Name:` block marked provisional.

## Follow-on

- **Recorded.** `thread_leak_police` polices only the modules that sort before it, and seven
  already sort after it. Its BUGS entry in `system_tests/src/user.rs` now names them. This split's
  eleven names all sort before it.
- **Decision.** Three defects found while moving and left alone (a doubled doc sentence, and two
  stale comments in the kernel crate) are under "Found while moving" in
  `design/decisions/0291-the-catch-all-userspace-test-file-splits-into-topic-files-beside-it.md`.
- **Decision.** The eleven module and file names ship provisional for calef's naming pass, as
  `design/decisions/0291-the-catch-all-userspace-test-file-splits-into-topic-files-beside-it.md`
  records.
- **Refused.** Folding the name-resolution test into `name_resolver_tests.rs`, or the composed
  process into `process_tests.rs`: a fold mixes moved lines with edited ones, and each topic file
  already holds its tests whole.

## Index row

`system_tests/src/user/tests.rs` is 3,533 lines and 71 tests, the suite's catch-all. Every
newer test has its own topic file beside it. This moves its tests into ten topic files and a small
helper module, renaming seven falsification patches. Siblings or a directory, and every name, are
an architect's call.
