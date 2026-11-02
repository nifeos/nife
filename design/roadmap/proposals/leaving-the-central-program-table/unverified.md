# Leaving the central program table: what was not verified

[The proposal](../leaving-the-central-program-table.md) states findings read from code and marks
inference as inference. This appendix gathers what the lane did not check and what it could not
determine. The lane wrote no code and booted nothing.

## Not run

- No boot, no `script/swish-check`, no system test. Every claim about runtime behavior comes from
  reading the shell, the progenitor and their tests.
- No measurement of the base list page's size, the boot cost of reading each archive program's
  note, or the memory an image pool would need to hold two images for a pipeline.
- No check that `every_compiled_in_manifest_round_trips` passes on this branch's base; the lane read
  it and did not run it.

## Read but not traced to the end

- `crates/timetable/src/plan_text.rs` keeps a plan as a mask of wire ids. The lane found no writer
  that stores or sends it, and did not trace every reader of `Plan`.
- Why `image_can_carry` refuses a silent output. The code gives the refusal and no reason; the
  [gaps appendix](installed-path-gaps.md) marks its guess as inference.
- Whether short options can travel without a directory grant. They ride inside the directory
  grant's words (`dir_grant` in `components/src/swish/running.rs`); the lane did not look for a
  second carrier.
- Whether a release image composed by `cargo xtask base-image` boots with the 990 rows read by the
  progenitor at spawn (`vouched` reads the live generation). The end state relies on the
  measurement instead, which every image has.
- §251 (restarting the machine is a kernel object) compares the reboot endowment with `entropy`,
  and milestone 805 (`reboot` at the prompt) rules that no note may spell `reboot`. The lane reads
  them as consistent and did not ask anyone.

## Could not determine

- Whether milestone 961 (the kernel's special boots become commands typed at the production shell)
  means its bench rows to run in a release image. `packages/base-image.toml` excludes the `fixtures`
  package, which holds them. This decides whether the bench rows are fixtures or tools.
- Whether any program outside this tree, such as a basalt-published program, sends a spawn request
  by wire id. The search covered this tree only.
- How an installed newer `date` would rank against the image's under §229 (a bare name reaches an
  installed program)'s amendment; the amendment does not say, and question 2 asks.
- When the shell and the progenitor will first run at different versions. The lane infers that one base
  image composes them together today, from `packages/base-image.toml`'s single `version`.

## Prior art

Every claim in the [prior-art appendix](prior-art.md) is from memory and was not re-checked.
