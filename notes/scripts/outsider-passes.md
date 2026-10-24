# Outsider passes: the launcher and the floor checker

An appendix to [`../scripts.md`](../scripts.md). It holds the full rows for the two commands an
outsider pass at the confinement claim runs. [The brief](../../briefs/confinement-outsider-pass.md)
says when to run each; the headers of `helpers/launch_pass.py` and `helpers/booted_rows.py` are the
manuals. Name: provisional, for this stem and both script names; calef names files and scripts.

| script | what it does |
|---|---|
| `script/launch-pass --reviewer glm\|fable --commit <sha> [--dry-run]` | Starts one reviewer on a Mac, in a container inside podman's Linux VM (calef, 2026-10-14 UTC): GLM 5.3 through opencode, Claude Fable through Claude Code. The container is described below the table. It makes a standalone clone with no remote and one branch, `pass`, at the pinned commit, and builds the one prompt: the Mac's runner text from [the launch brief](../../briefs/confinement-outsider-pass-launch.md), word for word with its values filled, then the reviewer's brief. It refuses a prompt that names a milestone or a roadmap block. The volume is provisioned (`script/ci-qemu`, `script/bootstrap`) with no model, then a setup turn with the model pinned and no tools reads back the model the session reports, and the brief is never sent if that differs from the pin. `launch.json`, on the host beside the clone, records the harness and version, the image digest and volume, the model pinned and reported, the session id, the commit, the prompt's SHA-256 and the times; the pass note copies it. `--complete <launch.json> --short-rows <file>` resumes that session once with `script/booted-rows --short-only` output as the whole message, and refuses a second round. Built by milestone 920 (an outsider pass is launched pinned and floor-checked). Selftest in `script/lint`, against a fake podman and a fake harness, checking every container command's mounts, variables and flags. **Name provisional.** |
| `script/booted-rows --checkout <dir> --pinned <sha> --log <file>... [--results <file>]` | Grades every claims-table row for one pass and ends `booted N of 27, floor met` or `floor short by K`, with the exit status to match. A row counts when a test the pass wrote (present at the pass's head, absent at the pinned commit) names it in an `Attacks rows:` line and ran to `ok` or a `[PANIC]` under QEMU on an ISA the row applies to. The exempt, carried and x86_64-only rows are read from the floor block in the brief, and the total from `notes/confinement-claims.md`. `--short-only` prints only the rows that do not count, the completion round's message. `--from-table <note>` replays a recorded pass from its per-row table. Built by milestone 920. Selftest in `script/lint`, with pass 8's and pass 9's replays. **Name provisional.** |

**The launcher's container.** The image is `script/runner-container`'s with both harnesses added at
the host's versions (`.github/runner-container/Containerfile.outsider-pass`), named by digest in
every run. The container sees only the clone, a results directory and the pass's own podman
volume, which holds the session state until the pass is collected. Its one secret is the model's
API key, passed by name. The launcher refuses unless the podman machine answers with at least 6
CPUs and 8 GiB, printing the `podman machine set` command, and never resizes the machine itself.

### EXAMPLES

Pass 8 (GLM 5.3), replayed from its note. The note's own count is 25.

```
$ script/booted-rows --from-table notes/confinement-outsider-pass-8.md
row 1: accepted, aarch64
row 2: booted, three ISAs
...
row 10: host only
row 11: host only
row 12: host (exempt)
...
row 34: booted, three ISAs
booted 25 of 27, floor short by 2
```

The completion round for a pass the lane has reproduced:

```sh
script/booted-rows --short-only --checkout "$scratch/nife" --pinned "$sha" --log logs/*.log > short-rows
script/launch-pass --complete "$scratch/launch.json" --short-rows short-rows
```

A prompt to read before anything is spent: `script/launch-pass --reviewer glm --commit "$sha"
--dry-run` makes the clone and writes `prompt.md`, and touches neither podman nor a model.

A launch, with a key made for this pass, and the cleanup once the lane has collected:

```sh
script/runner-container --build-only          # once: the base image
ANTHROPIC_API_KEY=... script/launch-pass --reviewer fable --commit "$sha" --scratch "$scratch"
podman volume rm "nife-pass-$(basename "$scratch")"
```

### BUGS

- `--from-table` replays a recorded grading; it does not check it. It trusts the note's grade
  column, so it reproduces a hand count and cannot catch a row graded `booted` that never ran.
  Only the log mode does that.
- A log is attributed to an ISA by the `--- kernel tests, <isa> (QEMU <version>) ---` header
  `script/test` prints, or failing that by the ISA in the file name. A log with neither counts for
  no ISA.
- The floor's `of 27` follows the claims table. When the table grows, a replay of an old pass
  reports the old pass's booted rows against the new floor.
- The runner text is the launch brief's Mac section, written for GLM 5.3, with its commit trailer
  swapped for the reviewer's. If that section changes shape (a new variable, a second trailer),
  the launcher refuses rather than guess, and its selftest fails in `script/lint`.
- The image is built with both harnesses, so both must be installed on the host, even for a launch
  of one reviewer: their versions pin the image.
- `IS_SANDBOX=1` is set because Claude Code refuses to bypass permission prompts as root unless it
  is told it is in a sandbox, and the container runs as root. It and opencode's `--auto` are
  untried in this container until the first real launch.
- GLM's key is read from `ZHIPU_API_KEY`, the variable opencode reads for the `zai-coding-plan`
  provider. Confirm it at the first launch; a wrong name shows as a setup turn that reports no model.
- The first container run of a pass builds QEMU from source into the pass's volume (about 15
  minutes), and the clone is reached through the VM's file sharing, so its builds are slower than a
  native checkout's.
- The machine check reads the first machine `podman machine inspect` lists. With several, the one
  checked may not be the default connection.
- The setup turn is one short model call per launch, spent before the attack.
- `--complete` marks the record before it sends, so a crash during the send uses up the round.
  That is deliberate: a round that may have reached the reviewer is not sent twice.
