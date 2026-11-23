# Shared state: what a lane may not claim, and what a branch may not hold

*Appendix to `nifeos/nife:AGENTS.md`, which carries the rules: anything global to the tree is
the integrator's at merge, a lane that gates takes the `nife-dev` link and says so, and a finding
lands in `notes/` rather than living on a branch. This file carries the collisions of 2026-07-30 and
the reason `fix/redoxfs-write-loop` survived a prune. The toolchain mechanism is in
[`notes/std.md`](../../notes/std.md) and the relink command is in
`nifeos/nife:briefs/merge-and-cleanup.md`. Moved here 2026-09-23 (UTC) on
calef's authorization, unchanged in substance.*

**Anything global to the tree is assigned by the integrator at merge, never claimed by a lane.**
Concurrent lanes cannot see each other, so a lane that reaches for a shared resource is guessing.
Two kinds bit us on 2026-07-30:

- `design/decisions/` section numbers, three collisions in one day. Preferred: a lane does not
  touch `design/decisions/` at all, puts the reasoning in `notes/` and in its report, and the
  integrator mints the section at merge. (The calendar lane of milestone 51 (wall-clock time, the
  `date` command, and an NTP service) did exactly this, unprompted, and it was the only one of four
  that caused no conflict.) If a lane must write the section to make its own gates pass, the number
  is provisional: say so in the report, and expect renumbering.
- Counts that span the tree. The Kani harness count was written as 76 on one branch and 80 on
  another; the merged tree had 95. Both were counted honestly. Take such a number at merge, from the
  merged tree.

Some shared state is global to the *machine*, not the repo, and `rustup toolchain link` is the
one that has bitten: `nife-dev` is one symlink for the whole user account, so it means whichever
worktree ran `xtask std-src` last. Every lane that gates takes it, unavoidably, because
`script/test` calls `std_src()` transitively and a fresh worktree always has a cold farm. That is
expected: do not tell a lane not to do the thing gating requires, tell it to say in its report
that it took the link. Relinking is the integrator's duty at merge and the command is in
`briefs/merge-and-cleanup.md`. notes/std.md has the mechanism, the 2026-08-18 cross-contamination
that prompted it, and why `std_src` relinking loudly still does not make concurrent lanes safe.

An unmerged branch is either abandoned or it is holding knowledge that is not on `main`, and the
second case is a bug in where the knowledge lives. `fix/redoxfs-write-loop` survived that prune
because it carried an investigation's conclusion that `notes/fs-server.md` does not. Nobody reads
branches. If a branch holds a finding worth keeping, land the finding in `notes/` and then delete
the branch; do not keep the branch as the record.
