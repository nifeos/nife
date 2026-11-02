# Leaving the central program table: what was not verified

[The proposal](../leaving-the-central-program-table.md) states findings read from code and marks
inference as inference. This appendix gathers what the lanes did not check and what they could not
determine. They wrote no code and booted nothing.

## Not run

- No boot, no `script/swish-check`, no system test. Every claim about runtime behavior comes from
  reading the shell, the progenitor and their tests.
- No measurement of the base list page's size, the boot cost of reading each archive program's
  note, or the memory an image pool would need to hold two images for a pipeline.
- No check that `every_compiled_in_manifest_round_trips` passes on this branch's base.

## Read but not traced to the end

- `crates/timetable/src/plan_text.rs` keeps a plan as a mask of wire ids. No writer that stores or
  sends it turned up, and the lanes did not trace every reader of `Plan`.
- Why `image_can_carry` refuses a silent output. The code gives the refusal and no reason; the
  [gaps appendix](installed-path-gaps.md) marks its guess as inference.
- Whether short options can travel without a directory grant.
- Whether a release image composed by `cargo xtask base-image` boots with the 990 rows read by the
  progenitor at spawn. The end state relies on the measurement instead.
- What a second display session does to `gpu_driver` and `display_terminal`. The progenitor has no
  guard, and nothing tests the case ([the security appendix](security.md)).
- Whether two writers on `activation/indexes` can tear a file. No lock exists; the file server's
  behavior under two writers was not read.
- Whether a per-manager index directory breaks `jig list`, which reads one copy today.
- That the progenitor and the shell treat a program with no `OWNER_GRANTS` note as asking for none
  of the owner-granted fields. Read in the code: `elf::Elf::note` answers `Ok(None)` for a missing
  note, `capability_manifest_note::decode` sets every one of these fields to false, and the `VERBS`
  and `INTERFACES` readers treat absence as no declaration. Not verified: no reader of the new note
  exists yet, so the merge into one `Manifest` is unbuilt and untested.
- Whether the shell reads any start-up file of commands. A search found none; `main.rs` was not
  read whole.

## Could not determine

- Whether milestone 961 (the kernel's special boots become commands typed at the production shell)
  means its bench rows to run in a release image. `packages/base-image.toml` excludes the `fixtures`
  package, which holds them.
- Whether any program outside this tree sends a spawn request by wire id.
- How an installed newer `date` would rank against the image's under §229 (a bare name reaches an
  installed program)'s amendment, which does not say.
- When the shell and the progenitor will first run at different versions.
- Whether an owner can tell a fake prompt drawn by a granted display session from the real one. No
  key exists that the session cannot intercept.

## Prior art

Every claim in the [prior-art appendix](prior-art.md), and every prior-art line in
[the decisions](decisions.md) and [the command-name appendix](command-names.md), is from memory and
was not re-checked.
