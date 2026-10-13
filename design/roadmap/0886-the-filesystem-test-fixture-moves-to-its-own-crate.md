---
status: NOT-STARTED
raised: 2026-10-11
milestone_dependencies: 845
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 886. The filesystem test fixture moves to its own crate

*(The title and slug are provisional.)*

calef ruled on #1915, milestone 845 (the filesystem-protocol crate root is split along its seams),
on 2026-10-11 (UTC): "Keep it for now, and open the follow-up milestone". This is that milestone.
845 keeps `filesystem_protocol::fixture` in the protocol crate as `fixture.rs`, with its tests in
`fixture/tests.rs`, so that its split stays a pure move. This block moves it out.

It waits on 845. Moving `fixture` out of the 5,744-line `lib.rs` before 845 lands would make two
lanes cut the same file at once.

The crate name below is provisional and waits for calef's naming ratification pass; it does not
block the lane.

## What calef ruled (2026-10-11, UTC)

Relayed one question at a time by the maintainer, each with its prior art and alternatives.

- **Which items stay: A.** `READY`, `DESCENT_REFUSED` and the `rm` status convention (`rm::OK`,
  `rm::status`) stay in `filesystem_protocol`, in a module of their own (name provisional). They
  are words shipping programs send each other, and every other protocol here keeps its readiness
  word in its own crate (`entropy_protocol`, `byte_sink_protocol`, `login_protocol`). Refused:
  moving everything, which would make the kernel, `virtio` and `system_initializer` depend on a
  test-data crate to boot; and one shared readiness crate, which would merge words that differ per
  protocol on purpose. Everything else, `crash::CUT`, `throughput::RECORD` and `blank` included,
  moves.
- **When std stops copying it: A, as soon as 845 lands.** Step 1 below is its own small pull
  request, ahead of the crate move. Of the 27 commits that touched `filesystem_protocol/src/lib.rs`
  from 2026-09-11 to 2026-10-11, 10 edited `mod fixture`, and each made every worktree's std farm
  stale. The crate move later deletes the skip line, since `fixture.rs` will be gone. calef's
  ruling on 845 that std's copy keep the same text covered that pure move, not this change.

Reuse: not applicable; this moves code and adds none.

## What `fixture` is

Measured at `9532eeb53`. `pub mod fixture` is lines 2566 to 3806 of
`crates/filesystem_protocol/src/lib.rs`: 1,241 lines, 842 of them comments, 255 public items. It
holds the names and contents of the files the test image ships, the witnesses' verdict words and
the shapes of the `walk` and `throughput` benchmarks. It also holds the layout of the blank disk
that milestone 57 (partitioning and formatting a real drive) writes.

It lives in the protocol crate because both sides of every filesystem test already depend on that
crate. One item reaches back into it: `throughput::UNIT` is `fs::TRANSFER_MAX`. So a new crate
depends on `filesystem_protocol`, and the protocol crate cannot re-export the new one without a
dependency cycle. Every user is rewritten; there is no shim.

## std carries it today

Yes, all of it. `std_generate_modules` in `xtask/src/farm.rs` copies `lib.rs` into the std farm as
`sys/pal/nife/fsproto.rs`, cutting only at `#[cfg(test)] mod tests`. `fixture` sits above that
line, so it is copied whole.

Reproducing the generator's transform at `9532eeb53` gives a `fsproto.rs` of 3,703 lines and
227,590 bytes. `fixture` is 1,241 of those lines (34%) and 83,820 of the bytes (37%). The farm in
the main checkout, last generated 2026-10-06, has it at line 2445, 1,222 lines long.

std reads none of it. The overlay names `fsproto::dir` 24 times, `reply_errno` four times, and
`dirent` and `PAGE` once each. `sys/pal/nife/mod.rs` puts `#[allow(dead_code)]` on the module and
names `fixture` as one reason. And `std_inputs_stamp` hashes `lib.rs`, so editing a test file's
expected contents marks the farm stale and rebuilds std.

That is a reason to do this sooner. 845 keeps it: its generator inlines every module back, and
calef ruled that std's copy should stay the same text.

There is a cheaper half. Once 845 has landed, the generator can skip `fixture.rs`, one line in
`farm.rs`. std then stops carrying test data and stops rebuilding for it, and nothing else moves.
That is step 1 below, and it can land on its own if the crate move waits.

## The premise is half right

The ruling's premise is that `fixture` is test data, not wire protocol. For most of its items that
holds. For a few it does not: shipping programs use them to talk to each other.

| item | used in shipping code at | what it is |
|---|---|---|
| `READY` | 15 sites in 12 files: `redoxfs_server`, `virtio`'s blk server, the NVMe and DesignWare drivers, three caretakers, `login`, `system_initializer`, the kernel's `fs_service` and two kernel benches | the readiness word a storage or file server sends before it serves |
| `DESCENT_REFUSED` | `fs_subtree_caretaker` | the caretaker's refusal word |
| `rm::OK`, `rm::status` | `components/src/rm.rs` | `rm`'s exit-status convention |
| `crash::CUT` | `redoxfs_server`'s crash injector | the word it sends before it dies |
| `throughput::RECORD` | a `const` assertion in `redoxfs_server/src/lib.rs` | the record size its throughput phase assumes |
| `blank` | `disk_partitioner`, `system_installer` (only `LBA`), `mkfs` | milestone 57's test-disk layout, which the partitioner writes |

`READY` is a protocol word misfiled. calef ruled that it and `DESCENT_REFUSED` stay in
`filesystem_protocol`, and the `rm` status convention with them. A test-only crate that the kernel,
`virtio` and `system_initializer` must depend on to boot would be the same mistake reversed. The
other three are test hooks or test layout compiled into programs. They move with the fixture, and
those programs depend on the new crate.

## Who uses it

Measured at `9532eeb53` with a scan of every tracked `.rs` file outside `vendor/` and the crate. The
path `filesystem_protocol::fixture` occurs 185 times in 53 files; 35 of those lines are comments.
Two more files import it in a brace list (`rm.rs`, and `fs_test_client.rs`, which then says
`fixture::` 102 times). So 187 sites change.

| where | files | sites |
|---|---|---|
| `system_tests/` | 15 | 57 |
| `components/` | 10 | 29 |
| `xtask/` | 4 | 29 |
| `std_exerciser/` | 1 | 26 |
| `kernel/` | 5 | 13 |
| `redoxfs_server/` | 6 | 12 |
| `crates/` (`walk_pricing`, `system_initializer`, `virtio`; four others in comments only) | 8 | 11 |
| `fixtures/` | 2 | 7 |
| `bench/host/` (comments only) | 2 | 3 |

Ten manifests name the crate and use `fixture` in code. If the recommendation above holds,
`system_initializer` and `virtio` need no new dependency. Outside Rust, 12 files cite the path:
four roadmap blocks, six notes and two falsification patches under `system_tests/falsifications/`.

## The work

1. As soon as 845 lands, in a pull request of its own: the generator skips `fixture.rs`,
   `STD_SRC_PATCH_VERSION` is bumped, and the `allow` comment in `sys/pal/nife/mod.rs` stops
   naming it. Show the shorter `fsproto.rs` in the pull request.
2. The protocol words in the table above move to the crate root or a module of their own, name
   provisional, and their users follow.
3. The rest becomes a crate, `crates/filesystem_fixture` (provisional), with `fixture.rs` as its
   `lib.rs` and `fixture/tests.rs` as its tests. It is `no_std` and depends on
   `filesystem_protocol` alone.
4. Each user changes `filesystem_protocol::fixture` to `filesystem_fixture`. A file that says
   `fixture::` bare imports `filesystem_fixture as fixture`, so its body does not change.

## The name

`filesystem_fixture` is the proposal. It is the module's existing name lifted to a crate, so every
call site reads the same after the prefix, and a reader who knew the module finds the crate. It
does not end in `_protocol`, because it is not a wire contract; that suffix is for contract crates
since milestone 265 (`_proto` is a truncation). `filesystem_test_image` was considered and is
narrower than the contents: it names the files the image ships, but not the verdict words or the
benchmark shapes. calef names crates.

## What moving it breaks

- `helpers/interface_stability.py` lists `filesystem_protocol` as a contract crate, so the week this
  lands its breaking count jumps by every public item that leaves. That is the change being
  measured, not noise; the commit says so.
- The two falsification patches that diff a test using `filesystem_protocol::fixture` must be
  regenerated and shown to fail again.
- `design/comment-block-baseline.tsv` has no row in lines 2566 to 3806 today. A lane checks again
  after 845, since calef's amendment to §267 (a comment states the constraint as it is now) on
  #1915 moves a row with its block when a file splits.
- Six other crates have a `pub mod fixture` of their own, all under 82 lines. Two of them,
  `socket_protocol` (47) and `byte_sink_protocol` (35), are also copied into std. They are out of
  scope; this block records them so the next lane does not have to measure again.

## Done when

1. `filesystem_protocol` has no `fixture` module, and no test data remains in it.
2. The generated `fsproto.rs` has no `fixture` module, and the std farm builds.
3. The kernel, `virtio` and `system_initializer` do not depend on the new crate.
4. Every host test and doc test that passed before passes with no test body changed, and so does
   `script/test` on aarch64, riscv64 and x86_64.
6. `system_installer` takes its block size from `blk`, not from `blank::LBA`, so no shipping
   installer depends on the test crate.
5. The new crate's `lib.rs` carries a `//! Name:` line marked provisional.

## Index row

`filesystem_protocol::fixture` is 1,241 lines of test data in a wire-protocol crate, and std copies
all of it into every build: 34% of the generated `fsproto.rs`. This moves it to a crate of its own
at 187 sites, after 845. A few of its items are protocol words that shipping programs use, and
those stay, per calef's ruling. std stops copying it as soon as 845 lands.
