# Milestone 198 appendix: why a package manager comes early

An appendix to [milestone 198 (a package manager)](../0198-package-manager.md). The main page is
complete without it. This is the block's case for doing the work before any third party exists, and
the prior art it builds on.

calef's two halves, from 2026-08-30: no third party sees nife until there is a package manager and a
trivial install, and he wants it early, to make our own lives easier. Both halves matter and they
point the same way.

## This is a precondition on principle 1, not an item under it

AGENTS.md ranks work by the shortest path to a system a customer runs. That path is vacant, for two
reasons. The first customer's deadline passed and they went to borg over SSH, and this system could
not accept a second one if it appeared.

That makes packaging structurally different from the milestones it sits beside. It is not on the
customer path; it is the door. A roadmap that ranks by the shortest path to a customer while
being unable to take one is ranking against a door it has not built.

## The second half is the one that earns it early

The third-party argument alone would justify deferring this indefinitely, since there is no third
party and no date for one. The reason to do it early is that the builders are the ones paying for
its absence today, hand-wiring per program what a package would install once:

- Milestone 40 (documentation as a system service) already ships *"installed by the package that
  owns it"* and its per-package index shard, against no package that exists.
- Milestone 47's conclusion is that installing a program is granting it into a namespace, which
  is a packaging statement with no packager.
- `crates/system_initializer` spends a capability through a system call per program, per architecture,
  and every new program edits it.
- Milestone 150 (adding a program should not need eight hand-maintained lists) is the same complaint
  from the other end, and the count is the argument: eight lists, hand-maintained, per program.

## What already exists to build on, so this does not start cold

- Milestone 39 has the structural recommendation: monorepo now with the distribution as a
  separate manifest repo, executed as multiple workspaces.
- `design/haiku-bfs-and-packages.md` is the prior art the roadmap already tells you to read
  first. Haiku's `packagefs` activates packages rather than installing them. It composes a
  filesystem view from read-only package files rather than letting installers mutate shared
  directories. It arrived near milestone 47's conclusion from an entirely different motive, atomic
  and rollback-able installs, which is the useful kind of convergence.
- `design/what-a-distribution-packages.md` is the speculation about the units, and is explicitly
  labeled as speculation.
- DECISIONS §135 (running GPL software is aggregation) makes packages the channel for copyleft.
  So this milestone is also what unblocks `git` and `nano` arriving the honest way rather than being
  built into an image.
