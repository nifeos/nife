# Leaving the central program table: what was not verified

[The proposal](../1027-leaving-the-central-program-table.md) states findings read from code and marks
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

- Whether the planner can enforce "at most one holder" of an exclusive grant at plan time. Read in
  the code: no planner code checks it today (`console_input` appears in `grant_plan` only in the
  manifest, `image_can_carry` and verb narrowing, and in the shell only to leave raw mode). The shell plans one line at a time and waits while a foreground job runs, a background job gets
  no stdin (`SpawnLoop::console_input` skips a quiet job), and the shell refuses `&` for the
  display. So plan time can catch two holders within one line and name the first. It cannot see a
  holder another spawner started: a second shell, if a session ever gets a spawn endpoint (§270
  (a package manager holds an installer endpoint, not the spawn endpoint)'s BUGS says none does),
  or the timetable. At run time a second stdin reader gets `BAD_REQUEST` from the terminal's one
  `pending_stdin` slot, and a second display session meets no guard at all. Not run: nobody booted any of this.
- Whether anything outside `grant_plan` still looks a program up by name once the table is gone,
  beyond the manager column. Read in the code and in [the consumers](consumers.md): the shell's
  `bare` and completion, the §229 (a bare name reaches an installed program) install check in
  `activation.rs`, the progenitor's `archive_name` and its `Program::GraphicalTerminal` checks,
  and the timetable's `resolve` and `plan_text`'s `saw`. In the end state each is a lookup from a
  name to bytes in the measured archive or the live table, never to a grant: the graphics checks
  move to `Manifest::display`, and the rest look up the archive by name. Not traced: every reader
  of the timetable's plan mask.
- How the shell and install learn the archive's names at run time. Read in the code: the
  progenitor already holds the measurement table that `measured_boot::verdict` reads, one
  `<name> <digest>` line per archive program, so install can ask it. The shell holds nothing of
  the archive today and learns the names only once the base list page exists (migration step 9).
  Not verified: that the base list carries every archive name, including programs with no note.
- Whether builtins need their own rule once the table is gone. Read in the code: yes.
  `grant_plan::parse` matches builtins first, and only
  `every_listed_builtin_is_a_builtin_and_no_program` keeps a program off a builtin's name, through
  `Program::from_name`. Migration step 10 replaces it with a build check on the archive's names.
- What first boot needs with no table. The image's `reboot`, `jig` and `graphical_terminal`
  receive their owner-granted fields from `grant` lines in generation 1, which the composition
  writes from the base approval file ([the end state](end-state.md)). Read in the code: generation
  1's base rows carry no grant lines today, and milestone 990 (the base image's first
  installed-programs table) measured that generation at 5,691 bytes against the progenitor's
  4,096-byte page. Not verified: how much the new lines add.
- Whether the base approval file can hold each kind. Read in the code, it cannot yet.
  `approval_file::File` knows `network`, `zones`, `service`, `installer` (a boolean, with no
  `within` form), `dirs`, `update` and `remove`, and refuses any other key (`Why::UnknownKey`).
  It has no key for `reboot`, `sync`, `catalog`, the index store and its rights, the activation
  directory, console input, the display, or `domain`, `entropy`, `machine` and `share`.
  `installed_programs::Granted` lacks the same kinds. Migration step 5 adds them.

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
[the decisions](decisions.md), is from memory and
was not re-checked.
