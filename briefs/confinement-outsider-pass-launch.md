# Launch: a pair of outsider passes at the confinement claim

**Name: provisional.** `script/launch-pass` launches each pass of the pair criterion (c) counts.
It clones the pinned commit and builds the one prompt from [the brief](confinement-outsider-pass.md)
and the runner text below, refusing a milestone number or a roadmap block in it. It pins the model,
and stops a session that reports another model before the brief is sent. This file holds the runner
text it reads and what it leaves to whoever launches. Its commands are in
[notes/scripts/outsider-passes.md](../notes/scripts/outsider-passes.md), and
`helpers/launch_pass.py`'s header is its manual.

## The rules both launches keep

- The pair. One pass by GLM 5.3 and one by Claude Fable, both on one Mac, each in a container
  inside podman's Linux VM (calef, 2026-10-14 UTC). Claude's cloud runner is not used: it cannot pin
  the model and read it back before the attack, or resume the session for the completion round.
- One commit. Both launches take the same `--commit`, a full main SHA, and each pass's note names
  it.
- Concurrent is allowed. Nothing changes between the two passes, so they are consecutive even when
  they run at the same time. Each has its own clone and volume, and neither reviewer can see the
  other's work.
- Reset. If either finds an escape on a shipped path, the count resets: fix it, then both run again
  at a new pinned commit.

## Before a launch

- Read the prompt first. `--dry-run` writes the clone and `prompt.md` and touches neither podman
  nor a model. It should hold the runner text below and the reviewer's brief, and nothing else.
- The podman machine must be up with at least 6 CPUs and 8 GiB. The launcher refuses with the
  `podman machine set` command otherwise and never resizes or restarts the machine. Whoever launches
  checks what else the machine is running before resizing it.
- The model's key is made for this pass: `ZHIPU_API_KEY` for GLM 5.3, `ANTHROPIC_API_KEY` for Fable.
  Its owner revokes it once the lane has collected.

## The runner text

The launcher reads the `cat <<EOF` body below word for word, fills `$scratch` and `$sha` with the
container's paths and the pinned commit, and swaps the GLM 5.3 trailer for the reviewer's own. Both
reviewers get it. Its selftest assembles both prompts from this file and fails in `script/lint` if
the text changes shape. A changed word changes every later pass's prompt, so passes compare row for
row only within one wording.

### GLM 5.3 on a Mac

```sh
runner_text() {
    cat <<EOF
## Before you start

This section is about the machine you run on. Where it and the brief below disagree, it wins.

- The checkout is \`$scratch/nife\`, on branch \`pass\` at commit \`$sha\`. Stay on that branch
  and that commit. It has no remote; do not add one.
- This is the maintainers' Mac, and other work may be loading it. Before each \`script/test\` leg,
  record \`uptime\`. A timeout is inconclusive, not a hold or an escape: rerun that leg once, alone,
  and report both runs with their load.
- End every commit with \`Co-Authored-By: GLM 5.3 <noreply@z.ai>\`.

EOF
}
```

## After a pass

- The reviewer's commits are on branch `pass` in `<scratch>/nife`, which has no remote: `git fetch
  <scratch>/nife pass`. Its report is `<scratch>/results/results.md`, and its test logs are under
  `<scratch>/results/logs/`.
- `<scratch>/launch.json` is the launch record. The pass note copies it.
- Keep the pass's podman volume until the lane has collected and any completion round has run. The
  launcher prints its name and the `podman volume rm` that removes it.
