---
status: AMENDED
raised: 2026-09-15
decided: 2026-09-15
ratified_by: calef
---

# 151. The goal of the repository split is independent release and third-party programs

calef, 2026-09-15, in conversation, thinking through the eventual multi-repository
structure: *"Independent releases is the goal and third party programs is the goal."*

## What this settles

Milestone 39 (repository structure for a loosely-coupled OS) recorded four options and took no
decision, on the ground that nothing external needed one yet. Its options B, C and D were three ways
of reaching, or deferring, a decoupled structure; the open question underneath them was what the
structure is for. This decides that:

- Independent release. Each piece carries its own version and ships on its own cadence, behind an
  interface that does not change under its consumers' version requirement.
- One purpose per repository. Every piece leaves this repository for a repository of its own with
  one purpose: the kernel, the programs, the tooling and the crates. No piece has to pass a test to
  leave. The order below decides only when it leaves.
- Third-party programs. The structure is the one an outside author writes a program against
  without this project in the loop, which is milestone 39's option C described as *"what an ecosystem
  with third-party components looks like"* and milestone 198's *"no third party sees nife until there
  is a package manager and a trivial install."*

So the endpoint is the decoupled ecosystem, not the monorepo held forever. Milestone 39's own
recommendation is consistent with this: get there through option B (workspaces inside one
repository) and let the distribution be a manifest repository (`basalt`, reserved by milestone 120
(the rename)). Each piece leaves that repository whole, in the order below, rather than by directory
surgery up front.

## The order: bottom-up

Nothing that leaves this repository refers back into it. So the split goes bottom-up in the
dependency graph: the crates a program links leave before it or with it, and nothing that has left
depends on a path or a pin into this repository. This repository may depend on what has left. The
leaf pilot `jh7110_entropy` (milestone 818 (proven JH7110 TRNG logic, released on its own)) leaves
first, then the contract crates, then the programs and tooling once the crates they link have left.
The kernel leaves last among the code, to `nife-kernel` (provisional). Once everything has left,
`nifeos/nife` holds only the history.

Churn does not hold a move. A contract crate leaves and carries a version while its interface still
changes, and each consumer checks that version (milestone 956 (the linked interfaces carry checkable
versions)). The single-command, all-architecture `script/test` proves the whole system today, and
the split has to keep that proof running (see BUGS).

## The model is a Linux distribution, not a BSD (calef, 2026-09-15)

A BSD ships a base system: kernel plus core userland as one cohesive tree, developed and released
as a unit, with ports layered on top. There is a privileged base, and it *is* the OS. A Linux
distribution has no base tree. The kernel is one upstream project on its own cadence, and every
userland piece is a separately-versioned upstream. The distribution is an integrator that packages
those independent pieces together with glue and a package manager, owning none of them.

nife is the second. Three consequences that are the point of naming it rather than a stylistic note:

- The kernel is one independently-released component, not a privileged base. From the
  distribution's view it is packaged like any other piece. This is why independent release (above) is
  the property that matters and not "the base tree stays coherent": there is no base tree to keep
  coherent.
- `basalt` is the distribution, not a base manifest. It packages the kernel, the components, and
  the system glue, the way Debian assembles Linux plus a userland it did not write. It is an
  integrator and an assembly step, which is exactly milestone 39's packaging observation (the program
  manifest plus measured-boot hashing are three quarters of a package format already). It is **not**
  a pinned manifest over a base system that a BSD-shaped reading of milestone 39's "distribution as a
  manifest repo" recommendation would suggest.
- Third-party programs are first-class, not ports on top of a base. A program an outside author
  writes is the same kind of thing as one this project wrote; the distribution packages both. That is
  the Linux-distribution posture and it is what "third-party programs is the goal" means structurally.

**The model does not set the order.** The bottom-up rule above does. A piece lives in this
repository until its move, and the moves start at the leaves. The *target model* being a
distribution rather than a base system neither hastens nor holds a move. It corrects the mental
model of the endpoint, and with it the maintainer's earlier BSD-leaning comparison.

## What is unblocked

- Milestone 39 can move from RECORDED to a direction: its option C is the stated destination,
  reached as B. It no longer waits on "what is the structure for."
- The wire-protocol crates gain a reason to carry real versions, which milestone 39's packaging
  observation names as the thing that lets components evolve independently at all. Independent release
  is not possible while every crate is `version = "0.1.0"`.
- `basalt` (milestone 120's reserved distribution repository) has its job named: it assembles
  released pieces, and it can begin as a manifest repository that names what the distribution contains
  before it holds any code.

## BUGS

- The integration gate is the real cost and is not solved by this decision. A multi-repository
  world has to keep proving the whole system on every supported architecture, and milestone 39 records
  that a naive split makes that proof "live somewhere awkward or quietly stop running." Deciding the
  goal does not build that gate.
