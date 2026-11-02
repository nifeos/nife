# Leaving the central program table: the migration, one pull request per step

[The proposal](../leaving-the-central-program-table.md) holds the end state and the decisions. This
appendix orders the work. Each step is one pull request, leaves `main` green, and says what it
proves and what it unlocks. Steps 1 to 3 need no ruling and could start now.

## The steps

1. Every row's program carries a note copied from its row, and the shipped ids are all pinned.
   Each tool and fixture whose manifest the note can spell gains
   `capability_manifest_note::carry!(grant_plan::Program::X.manifest())`, the line
   `least_authority_demo` already carries. A system test beside `measured_boot_tests.rs` decodes
   each archived program's note and checks it equals its row, skipping the pinned rows and `mdr`
   (see the last section). The
   same pull request extends `the_wire_ids_already_shipped_never_move` from 13 ids to all 28. It
   proves that every non-pinned row is expressible as its program's own declaration, on every
   boot. It changes one behavior: the progenitor endows a tool run by its path from its true note
   rather than from `NO_NOTE_MANIFEST`. So it refuses `./date`, whose second stream the frames path
   cannot carry; before, `./date` ran without its clock. It may split by package if the diff is large.
2. The endowment carries its manifest. `Endowment` gains its `Manifest`, and
   `spawnproto::held_shares`, `hold_pages`, the preview and the timetable read it rather than
   `e.program.manifest()`. No wire change. It proves need A of
   [the consumers](consumers.md) no longer depends on a row, and it removes the three comments that
   warn readers off `IMAGE_ROW`.
3. The frames path carries a second stream and redirection. `run_image` delegates the
   diagnostics, sink and source endpoints as `spawn` does, and `image_can_carry` admits
   `BytesAndDiagnostics`. It proves an installed tool can hold the stream eight tools declare,
   through a `script/swish-check` line that runs an installed copy. It unlocks installing those
   eight from a disk. This track is independent of steps 4 to 8 and serves programs installed on a
   disk.
4. The progenitor publishes the base list, and the shell plans from it (waits on
   [decision 1](decisions.md)). The progenitor writes the (name, note) page at boot and hands it to
   the shell. The shell's planner, completion and `caps` read the page for every name it holds, and
   still spawn by id. A host test checks the page round-trips; the shell refuses a line whose page
   manifest disagrees with the row, which step 1 already made impossible. It proves the shell can
   plan without the table.
5. Archive programs spawn by name (waits on decisions 1 and 2). A new request bit carries the
   name on the argument page; the progenitor looks it up in its measured archive, reads its own
   copy of the note, and endows from it under the archive's vouch, minus every pinned field. In the
   same pull request, the install refusal of §229 (a bare name reaches an installed program) and the
   prompt's `bare` switch from `Program::from_name` to "the archive carries this name", so no later
   step can quietly let a package claim `date`. The shell sends a bare word by name whenever the
   base list holds it. Every row still exists and still answers its id. It proves each tool's
   transcript stays the same when it runs by name.
6. The rows leave, one package per pull request. Fixtures first (11 rows, kind `test`, no
   package boundary), then `process-tools` (7), `core-tools` (5), `demos`, and `mdr` once it
   carries a note. Each pull request
   deletes the rows and their manifest functions, and moves each manifest constant into the
   program's source. It also moves the sweeps of need E from `Program::ALL` to the programs that
   carry a note: `check_declared_programs`, the script-line check, the measurement sweep, the
   preview and sentence tests. Each removed id stays a hole, which step 1's pin enforces. It proves the program table is
   no longer the source of any non-pinned grant. The fixtures step also replaces `sentences.rs`'s
   two demonstration arms with one generic sentence for a register answer.
7. The pinned table keys on names. What remains of `Program` is the pinned rows. The
   progenitor's `[Option<Elf>; PROGRAM_COUNT]` shrinks to them or becomes a name lookup, and the
   pinned rows spawn by name too, leaving every id reserved and unused. Whether the enum keeps the
   name `Program` is a naming question (decision 5). It proves the table is the size of the pinned
   grants.
8. The leaf crate (waits on calef's open ruling on `program_manifest`, provisional name).
   `Manifest`, its declaration types and the slot constants move to the leaf; the tools,
   `capability_manifest_note` and `grant_plan` link it; `rm` holds its own option bits. It unlocks
   the contract-crate order: the leaf and `capability_manifest_note` can leave before step 6 of
   that order, so `process-tools` and `core-tools` stop waiting for `grant_plan`. This step can
   come anywhere after step 2; it does not depend on steps 4 to 7.

A transport track for programs installed on a disk follows step 3 and runs beside the rest, one
pull request each. It covers images as pipeline stages (a staging region per stage) and the
declared directory operand on the frames path. It also covers interruptible and held images built
from the shell's budget, and reading the `VERBS` note at both ends. The base tools need none of it once step 5 lands.

## What moves the contract-crate order

Only step 8 moves a package earlier in the order, because only the leaf crate removes the tools'
link to `grant_plan`. Steps 1 to 7 cut the churn instead. Measured on this branch's base over 30
days (`git log --no-merges --since=2026-10-03`): 148 commits touched `crates/grant_plan`, 36 touched
`program.rs` or `program/`, 55 touched `spawnproto.rs` and 43 touched `lib.rs`. About a third of
the 36 add or change a program (the bench rows, the held jobs, `mdr`, the `jig` and resolver
work); the rest are renames and documentation sweeps that would have touched
any file. So leaving the table removes the per-program edits and perhaps a quarter of the crate's
commits. It does not make `grant_plan` quiet; `spawnproto` is the larger source.

## Wire ids

- A shipped id never moves and is never reused. Step 1 pins all 28, so steps 6 and 7 can only
  leave holes.
- The by-name bit is an addition. A progenitor that knows it still serves every id that has a row,
  so a shell from before step 5 keeps working against a progenitor from after it, until its row
  leaves.
- An id whose row has left answers as `Program::from_id` answers today, `None`, so the request fails
  with `SPAWN_FAILED`. Answering `SPAWN_NOT_IN_IMAGE` there instead would tell an older shell what
  happened; that is a one-line change in `resolve`, worth making in step 6.
- Inference, not checked against a release: one base image composes the shell and the progenitor
  together today (`packages/base-image.toml`, one `version`). A skew between them can happen only
  once §241 (a threadbare base) lets `swish` update as a package apart from the slot.

## Test strategy

- The proof for each moved program is its `script/swish-check` transcript, unchanged, line for
  line, from step 4 to step 6. A program whose transcript changes did not move cleanly.
- From step 1 to step 6 the note and the row coexist, and the on-target sweep checks they agree on
  every boot. Disagreement cannot reach a person, because the row still decides.
- The note encoder's compile-time refusals and `image_can_carry` keep the pinned fields off every
  note; `every_compiled_in_manifest_round_trips` keeps checking the pinned rows' manifests.
- Host tests that sweep `Program::ALL` for planner behavior (`crates/swish/src/preview/tests.rs`,
  `planning/tests`) take their manifests from `test_support.rs` constants or from the fixtures' own
  constants, so the planner keeps a test for every declaration shape once the rows leave.

## The first step that needs no ruling

Step 1. It writes no wire format and names nothing new. It adds no dependency for the programs in
`components/` and `fixtures/`, which already link `capability_manifest_note`. `mdr` is the
exception: `mdr/Cargo.toml` links neither that crate nor `grant_plan`, so a note there would pull
the planner into the `mdr` package. Step 1 leaves `mdr` out, and `mdr` gains its note in step 8,
through the leaf crate, or with a recorded new edge if decision 6 refuses the leaf. Steps 2 and 3 also need no ruling and can run
beside it.
