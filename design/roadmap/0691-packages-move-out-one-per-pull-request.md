---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: packages-move-out-one-per-pull-request
milestone_dependencies: 611
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 691. Packages move out of this repository, one per pull request

*Title and slug are drafts.*
Raised by the lane of milestone 611 (every program and crate belongs to a package), which drew the
package boundaries and gated them but moved nothing, as briefed. calef ruled on 2026-09-27 that the
end state is everything leaving this repository, with no default of staying. This is how the moving
is proposed to go. It needs calef first: a move is only possible once a package's home is ratified.

## The shape

- One package per pull request, at a quiet moment in the merge queue, since a move touches the
  workspace root that every lane shares.
- The first commit is unchanged file moves and nothing else, so `git log --follow` and review both
  see a rename. The package's `crates`, `programs` and `paths` in `packages/<name>.package.toml` are
  the list of what moves.
- The second commit repairs what pointed at the old paths: the workspace members, the gates, CI.
- The package's `home` line flips to ratified when the repository exists, and the weekly metrics
  page's "moved out" band grows by one.
- Leaves first. `contracts` and `runtime` move before the programs that link them, and `kernel`
  and `init` move last, because they carry most of the recorded exceptions.

## Before the first move

- Every exception in a package being moved is resolved, not carried to the new repository.
- `components` is split into one crate per package, since a crate cannot live in two repositories.
- The gates learn to run against a package outside this tree. That is pull request #1389's P1 base image list,
  generated from the package files, and nothing builds it yet.

## Index row

calef ruled that everything leaves this repository eventually. Proposed: move one package per pull request at a quiet moment in the queue, unchanged file moves first, then the repairs; a move needs a ratified home.
