---
status: BUILT
raised: 2026-10-04
built: 2026-10-17
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 751. Fuzz the services' request handlers

Built by the lane `lane/fuzz-service-handlers` from part (a) of the proposal on #1592 (*fuzz the
surface a confined process can reach*), which calef ruled yes on 2026-10-04 (UTC). Part (b), the
system call fuzzer, was not ruled and is not here. The number 751 is provisional until the queue lands
it, and the proposal is promoted into this block once #1592 merges. *(Title and slug are drafts;
target, helper and module names are provisional.)*

## What it does

Three host `cargo-fuzz` targets, each replaying a session of requests against a service built on
the host and checking a stated rule after every reply, not only "no panic". `notes/fuzzing-the-services.md`
has the rules, the rates and the findings in full.

- `redoxfs_server_session` drives `redoxfs_server::Server::handle`, the file server's request
  dispatch, moved out of the EL0 binary for it with no behavior change. Rules: admission (a revoked
  or unknown badge succeeds at nothing; a bound badge only through `ROOT` or what it minted) and
  content (a bound badge never reads a file outside its grant). The image is formatted once and
  reopened per input through a copy-on-write disk: 688 sessions/s became about 2,180.
- `system_log_session` drives `Log::handle` and `Log::fill`. Rules: framing, stamping, per-user
  scope and sequence order on every read. About 15,000 sessions/s.
- `compositor_session` drives `compositor::commit_damage` (extracted from `serve_frame`) and
  `composite`. Rules: a lying client's damage stays in its own window, an honest status word, and
  the screen is right inside the damage and untouched outside it. About 1,680 sessions/s.

## Proof condition

1. The three targets are in `fuzz/Cargo.toml`, `script/fuzz --list` and the CI sweep, each with a
   stated rule. Met.
2. Each finds a planted defect, with the time recorded: under two seconds each, from an empty
   corpus. The patches are in `fuzz/falsifications/`; the file server's is milestone 726 (an unknown
   badge fails closed in subtree_scope)'s fail-open arm restored. Met.
3. `net_stack` and the three file caretakers carry a `BUGS` entry saying what extraction blocks a
   target, and the note measures `net_stack`'s. Met.
4. The CI `fuzz` job stays inside its 20-minute budget. Met: the median over 29 merge-group runs
   (2026-10-16 and 2026-10-17 UTC) is 712 s, 11.9 minutes, range 678 s to 744 s;
   `notes/fuzzing-the-services.md` lists the run ids.

## What it found

Three defects that killed or escaped the file server, each fixed in `redoxfs_server` with a host
test that fails without the fix. A grant's root could be closed and its slot reused, so a bound
badge's `ROOT` named an object outside its grant. A write whose end wrapped 64 bits panicked the
engine. A file end past the engine's node tree panicked it; the server now bounds it at
`MAX_FILE_END`, and #1606 takes the vendored fix. A fourth, a stall from shrinking a huge sparse
file, is recorded and proposed rather than fixed.

## Follow-on

- **Done.** Proof condition 4 is measured: `notes/fuzzing-the-services.md`, "What CI pays".
- **Done.** The level-4 `NodeLevel::new` constant, as a RedoxFS pin divergence, is #1606
  (`lane/redoxfs-level4`).
- **Milestone 772.** Milestone 772 (a client cannot stall the file server with a sparse file). The sparse-file stall in `Server::truncate`'s `BUGS` section, as
  `design/roadmap/0772-a-client-cannot-stall-the-file-server-with-a-sparse-file.md`.
- **Recorded.** A host
  part for `net_stack` and the caretakers is in their `BUGS` sections
  (`components/src/net_stack.rs`).

## Index row

Host fuzz targets replay sessions against the file server, the system log and the compositor, each
checking a confinement rule after every reply. Each finds its planted defect in under two seconds;
the file server's found three real defects, fixed.
