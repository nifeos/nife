# Leaving the central program table: the migration, one pull request per step

[The proposal](../leaving-the-central-program-table.md) holds the end state and the decisions. This
appendix orders the work. Each step is one pull request, leaves `main` shippable, and says what it
proves, what tests it adds and what waits on a ruling.

## The steps

1. Every row's program carries a note copied from its row, and a test pins every shipped id. Each
   tool and fixture the note can spell gains
   `capability_manifest_note::carry!(grant_plan::Program::X.manifest())`, as
   `least_authority_demo` already does. A system test beside `measured_boot_tests.rs` checks each
   archived note equals its row, skipping `reboot`, `jig`, `graphical_terminal` and `mdr` for now.
   `the_wire_ids_already_shipped_never_move` grows from 13 ids to 28. The progenitor then endows a
   tool run by path from its true note, so it refuses `./date`, whose second stream the frames path
   cannot carry. No ruling.
2. The endowment carries its manifest. `Endowment` gains its `Manifest`, and
   `spawnproto::held_shares`, `hold_pages`, the preview and the timetable read it instead of
   `e.program.manifest()`. No wire change, no ruling.
3. The frames path carries a second stream and redirection. `run_image` delegates the diagnostics,
   sink and source endpoints as `spawn` does, and `image_can_carry` admits `BytesAndDiagnostics`. A
   `script/swish-check` line runs an installed copy of `date`. No ruling.
4. The note spells the owner-granted fields (ruled, [decision 1](decisions.md)). A note type of
   its own, `OWNER_GRANTS = 5`, carries a flag byte for `reboot`, `sync`, `installer`, `catalog`,
   `activation_directory`, `console_input` and `display`, and a rights byte for `indexes`.
   `Manifest` gains `display`, and the shell and the progenitor merge the note into it.
   `image_can_carry` still refuses every one of these fields, so nothing a person sees changes.
   Host tests round-trip each field and check that bytes with no such note ask for none.
5. The grant lines and the vouch. `installed_programs::Granted` gains the kinds in
   [the end state](end-state.md); `Vouch::Vouched` carries `OwnerGrants`; `image_manifest` refuses a
   declared field without its line (`NotGranted`, naming it) and drops the blanket refusals in
   `image_can_carry` and `encode`. `program_asks` and `jig`'s `say_asks` learn the new asks with
   the confirmation of [the security appendix](security.md). In the same pull request, the
   progenitor serves an installed installer holder only the delegated badge, refuses
   `answered console` from it, and refuses a delegated install that asks for an exclusive or
      console-class grant. Console input gains its `req.badge == 0` check. The approval file's
   grammar (`approval_file::File`) and its compiled records gain the same kinds, the delegated
   `installer within <approval>` and `indexes r` or `rw` included, plus `domain`, `entropy`,
   `machine` and `share` for the base file; today it knows only `network`, `zones`, `service`, a
   boolean `installer` and `dirs`. The ruling on the classes
   is in ([decision 2](decisions.md)).
6. The display session keys on its field. The shell sets `Wiring::graphics` from
   `Manifest::display`, not from `Program::GraphicalTerminal`, and the progenitor gates
   `Request::graphical` on the vouched manifest and a live-session flag. No ruling beyond 1.
7. The installed path reads the `VERBS` note. `endowed_image` stops refusing it; the shell's
   `hold_pages`, the preview and the progenitor's `resolve` narrow by the note's table. A
   replacement package manager then holds per verb what `jig` holds. No ruling.
8. `reboot`, `jig` and `graphical_terminal` carry their own notes, copied from their rows, and the
   step 1 check covers them. Their rows still decide. No ruling.
9. The progenitor publishes the base list, and the shell plans from it. The ruling is in
   ([decision 3](decisions.md)).
10. Archive programs spawn by name. `cargo xtask base-image` writes a `grant` line in generation 1
    for each owner-granted kind the base approval file allows a carried program, and the
    progenitor reads base grants as it reads an owner's. This needs generation 1 to fit what the
    progenitor reads (milestone 990's `BUGS`: 5,691 bytes against a 4,096-byte page). The
    §229 (a bare name reaches an installed program) refusals switch from `Program::from_name` to
    "the archive carries this name". Waits on decision 7.
11. The rows leave, one package per pull request: fixtures, `process-tools`, `core-tools`, `demos`,
    `mdr`, then `init` (`reboot`), `display` (`graphical_terminal`) and `jig`. The last one deletes
    `Program`, `programs!`, `Program::verbs`, `Program::resolver_zone` and the progenitor's
    `[Option<Elf>; PROGRAM_COUNT]`, which becomes a lookup by name. Every removed id stays a hole.
12. The owner's word table ([the command-name appendix](command-names.md)): the `word` line, the
    activation verb on the console's endpoint, `jig use`, and the lookup in `bare::resolve`. It can
    land any time after step 5. Waits on [decision 6](decisions.md) for words the image carries;
    without it, it serves every other word.
13. The leaf crate, if ruled, anywhere after step 2.

A transport track for programs installed on a disk follows step 3: images as pipeline stages, the
declared directory operand on the frames path, and interruptible and held images built from the
shell's budget. Base programs need none of it once step 10 lands.

## What changed from the eight-step plan

- Gone: the pinned table, keyed by name (old step 7), and the wire ids it would have kept. After
  step 11 no row exists, so no id survives except as a hole, and no table pins anything.
- New: the `OWNER_GRANTS` note (step 4), the grant lines and the vouch (step 5), and the display
  field (step 6). Also new: the `VERBS` reader (step 7, out of the transport track), the notes of
  `reboot`, `jig` and `graphical_terminal` (step 8) and the word table (step 12).
- Kept, in order: the notes (1), the endowment (2), the second stream (3), the base list (9), spawn
  by name (10), the rows leaving (11) and the leaf crate (13).
- The encoder's and `image_can_carry`'s refusals no longer stay forever. They go in step 5, in the
  same pull request that makes each field earn its grant, so no commit on `main` endows a field
  nothing granted.

## The tests that must exist

Each lands with the step that makes it true.

| test | proves | step |
|---|---|---|
| an `OWNER_GRANTS` note round-trips each owner-granted field, `decode` refuses any byte `encode` would not write, and bytes with no such note ask for none | the wire format, and absence as none | 4 |
| installed bytes declaring `reboot` with no grant line answer `SPAWN_REFUSED_BY_MANIFEST`, and the refusal names `reboot` | a declaration alone earns nothing | 5 |
| the same bytes with `grant <digest> reboot` hold the reboot object at slot 13 and restart the machine after their sync, in `script/swish-check` on all three architectures | the owner's grant endows | 5 |
| a grant for one digest gives nothing to a second version of the same package until the owner answers again | grants bind to bytes | 5 |
| `jig remove` of the replacement, and `jig rollback` past its install, leave its digest with no grant, and the image's `reboot` still restarts the machine | the way back | 5 |
| two installed programs and the image's `reboot` all hold the reboot object in one boot | several holders | 5 |
| an installed installer holder sending `answered console`, `Approve`, `Revoke` or `Vouch` is refused, and a delegated install asking for `display` is refused | the console class stays bounded | 5 |
| a line whose two stages both hold console input, or both the display, is refused at plan time naming the first holder; a second display session while one runs answers `SPAWN_NO_DISPLAY`, and a second stdin reader gets `BAD_REQUEST` | exclusive grants stay exclusive | 6 |
| an installed manager's `jig list` holds no installer, and its `jig install` does, by its own `VERBS` note | verb narrowing on the installed path | 7 |
| each moved program's `script/swish-check` transcript, unchanged line for line from step 9 to step 11 | the rows left cleanly | 9 to 11 |
| `reboot` typed after `jig use reboot acme-reboot` runs the replacement; after `jig use --default reboot`, the image's | the word table and its fallback | 12 |
| `jig use` on a delegated badge answers `NotServedHere` | only the owner writes words | 12 |

Every test above that refuses something gets a falsification patch under
`system_tests/falsifications/`, as milestone 892 (a std program at the prompt holds the network and
a resolver) did for its grants.

## Wire ids

- A shipped id never moves and is never reused. Step 1 pins all 28, so step 11 leaves only holes.
- The by-name bit is an addition. A progenitor that knows it still serves every id that has a row.
- An id whose row has left answers `SPAWN_NOT_IN_IMAGE`, a one-line change in `resolve` in step 11.
- Inference, not checked against a release: one base image composes the shell and the progenitor
  together today (`packages/base-image.toml`, one `version`).

## What moves the contract-crate order

Only step 13 moves a package earlier, because only the leaf crate removes the tools' link to
`grant_plan`. Steps 1 to 11 cut the churn instead. Measured on the base of the first version of
this proposal over 30 days (`git log --no-merges --since=2026-10-03`): 148 commits touched
`crates/grant_plan`, 36 of them `program.rs` or `program/`, and about a third of those 36 added or
changed a program. `spawnproto` stays the larger source of commits.

## The first steps that need no ruling

Steps 1, 2 and 3, as before, and steps 7 and 8 once step 2 lands. None of them writes a wire format
or names anything new. Steps 4, 5, 6 and 9 write wire formats or change policy, and their rulings
are in. `mdr` still waits: it links neither `capability_manifest_note` nor
`grant_plan`, so its note comes with the leaf crate or with a recorded new edge.
