**Provisional name.** Clean up after a lane's pull request merges. You hold the maintainer or the
steward role, a pull request you were watching has just landed, and the lane that produced it is
finished. Do the work; do not ask questions.

Why a brief. The clauses below (the merge checklist and the worktree prune) fire at one event and
are performed by one role, which is milestone 579 (which of the constitution must be carried, and
which is a brief)'s test for a brief. `AGENTS.md` keeps a pointer.

Why the mechanics belong in a file rather than in prose nobody re-reads. The failure this
prevents is the one failure in this system that destroys work rather than delaying it, and it has
happened at two scales. Eight finished worktrees accumulated before anyone looked, one of them
holding 3.3 GB. Then on 2026-07-31 the volume hit zero bytes free with 42 worktrees holding
78 GB, two lanes died mid-work, and those lanes could not even run `pgrep` to check for leaked
emulators, because every tool must create an output file before it runs. The warning signs had been
noted hours earlier, not acted on, and four more lanes were launched on top of them.

## Before you remove anything: is the lane actually finished

If a lane is blocked rather than merged, commit and push its work before removing anything. A
snapshot on the remote cannot be lost by a cleanup; an uncommitted worktree can. This clause is
carried verbatim from the constitution because it is the one that stops a cleanup from becoming a
loss.

    git -C <worktree> status --short

Anything printed is uncommitted work. Commit it and push the branch before continuing, even if the
commit message is `checkpoint` and the branch will never be merged. Do not prune a dirty worktree to
save a step.

Then list the work GitHub cannot see, for every worktree and branch on this machine at once:

    git fetch --prune origin
    helpers/at-risk-check.sh

`DETACHED` is a worktree on no branch, and `UNPUSHED` a local branch holding commits no remote
branch has, often ones pushed after its pull request merged. The merge drain's `orphan` cause sees
only remote branches, so these are the ones it misses (#1787, 2026-10-06). Read each before
removing anything it names: push it and open a pull request, or record what it found and let it go.

## The four steps

### 1. Every piece of identified work in the lane's report has a home

The lane's final report names work it found and did not do. Each such item is in one of exactly two
shapes, and "worth doing someday" is neither:

- a proposed milestone, provisional, whose number you mint at merge like every other global name;
  or
- a recorded limitation in the `BUGS` section beside the feature a reader meets.

A finding with no home holds the merge. If the pull request has already landed and you find a
homeless item in the report, file it now rather than deciding it was minor: a lane report is read
once, by one person, on the day it is written, which is exactly why this check is a step in a
checklist and not a habit.

### 2. The branch

Nothing to do. `delete_branch_on_merge` is `true` on this repository, so GitHub deletes the remote
branch itself. Prune your own stale remote-tracking refs when they accumulate:

    git fetch --prune origin

The local branch in the worktree goes away with the worktree in step 3.

### 3. Prune the worktree, in the same breath

    git worktree remove <worktree>
    git branch -d <branch>
    git worktree prune

**Deleting the branch does not remove the ~2 GB of `target/` behind it.** That is the whole reason
this step is spelled out: a branch deletion looks like cleanup and reclaims nothing. Measured on this
machine on 2026-09-23, ten lane worktrees held between 1.0 GB and 4.8 GB of `target/` each.

`git worktree remove` refuses a worktree with modifications, which is the safety net behind the
check at the top of this brief and the reason not to reach for `--force`. If it refuses, go back and
find out whose work you are about to delete.

The build output does not wait for this step, because merged lanes' worktrees can fill the disk and
fail every running lane with ENOSPC; calef asked for it to go on its own (2026-10-12 UTC). An hourly
launchd job (its plist kept off-tree by the maintainers) runs:

    python3 helpers/prune_build_output.py           # dry run
    python3 helpers/prune_build_output.py --remove

It deletes `target/` only, for a worktree whose branch has a merged pull request and no open one,
and only when nothing has written there for an hour. A `wip/<slug>-<x>` worker, which never opens a
pull request, is judged by its lane's branch instead: its `target/` goes once that lane merges or
already holds the worker's commits. The worktree and its work stay for this step.

### 4. The toolchains

There is no relink. Each worktree links its farm under its own name,
`nife-dev-<directory>-<hash>`, and the next `cargo xtask std-src` anywhere on the machine removes
the link a pruned worktree left (notes/std.md). The bare `nife-dev` link is written only by lanes
cut before 2026-10-10. Once `gh pr list --draft` shows none of
them still open, remove it once, and this paragraph with it:

    rustup toolchain uninstall nife-dev

Then the dated nightlies. Every toolchain bump installs a new dated nightly and nothing removes the old
one; on 2026-10-06 fifteen of them held 30 GB of `~/.rustup`. calef approved removing them once
nothing pins them:

    python3 helpers/prune_toolchains.py           # dry run
    python3 helpers/prune_toolchains.py --remove

It keeps every worktree's `rust-toolchain.toml` pin, Kani's pin (read from the installed Kani), and
anything not a dated nightly, so `stable` and the `nife-dev-*` links are never touched. It runs
after the prune, because a removed worktree releases its pin. It cannot run in the CI
toolchain-bump workflow, whose runner has its own throwaway `~/.rustup` and never sees this Mac's.
`notes/std.md` has the mechanism and the history of the shared link this replaced.

## EXAMPLES

Checking what the machine is actually holding, before and instead of guessing (real output, this
machine, 2026-09-23):

    $ git worktree list
    /work/nife                       34d7d89f0 [main]
    /work/nife-worktrees/a3          e6958bc5e [maintainer/installing-is-granting]
    /work/nife-worktrees/charter     7fc963909 [maintainer/a-brief-is-not-about-the-reader]
    ...
    /work/nife-worktrees/m315        10c669ecd [milestone/315-port-revocation-two-core]

    $ du -sh <worktree root>/*/target
    1.0G    /work/nife-worktrees/a3/target
    1.1G    /work/nife-worktrees/charter/target
    ...
    4.8G    /work/nife-worktrees/m315/target

    $ df -h /
    /dev/disk3s1s1   460Gi    12Gi   115Gi    10%   /

A clean removal of a merged lane:

    $ git -C <worktree root>/m315 status --short
    $ git worktree remove <worktree root>/m315
    $ git branch -d milestone/315-port-revocation-two-core
    Deleted branch milestone/315-port-revocation-two-core (was 10c669ecd).
    $ git worktree prune

And the pruned worktree's toolchain link, gone after the next `std-src` anywhere:

    $ cargo xtask std-src 2>&1 | grep removed
    std-src: removed `nife-dev-315-port-revocation-two-core-5c0e19a2`, whose worktree's farm no longer exists

## Stop, do not improvise

Three things are outside this brief. Stop and hand them back rather than guessing:

- **A worktree with uncommitted changes whose lane you cannot identify.** Commit and push it on its
  own branch and say so. Do not remove it.
- **`git worktree remove` failing for any reason other than modifications**, such as a locked
  worktree or a path that no longer exists.
- **A pull request that merged with its report naming work you cannot classify** as either a proposed
  milestone or a `BUGS` entry. That classification is judgment; say what you found and let the
  maintainer place it.

## BUGS

- Nothing here fires on its own. This is rung three of the constitution's ladder, a written
  record at the thing itself, and it only helps a reader who opens it. The two failures it records
  both happened to people who knew the rule. The steward's interval check for lane worktrees at risk
  is the nearest thing to a mechanism, and it watches for uncommitted work rather than for disk.
- There is no gate on disk headroom, so nothing warns before a write fails. `df -h /` is a manual
  step this brief cannot make anyone run, and the failure arrives as an unrelated command dying with
  `No space left on device`.
- Step 1 is unmechanisable on purpose. No check can tell an intention from an observation in
  prose; a lint that tried would carry `git grep -w TODO`'s 82% false-positive rate wearing a
  different hat. So the homeless-finding check is a human reading a report, and it has already failed
  once at scale: milestone 94 (the untracked-work sweep, and the convention that ends the category)
  left its own inventory in a pull request body for twelve days, by which point the item-level list
  was gone and had to be re-derived (`notes/untracked-work-sweep.md`).
- **A pruned worktree's toolchain link lingers until some worktree next runs `std-src`.** It is
  dangling and breaks nothing, since no other worktree resolves through it.
- The example paths are invented. The main checkout is whatever your clone is, and lanes live in
  `nife-worktrees` beside it (the worktree root); nothing checks that your layout matches.
