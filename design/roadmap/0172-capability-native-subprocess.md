---
status: NOT-STARTED
raised: 2026-08-25
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 172. A capability-native subprocess primitive: what `cargo`'s "spawn a helper, wait, collect its output" needs, without fork/exec

The second of four self-hosting milestones, for developing nife on a nife host. This is the real
architectural piece: research into what `rustc`/`cargo` (see [milestone
173](0173-rustc-cargo-self-host.md)) actually need found that their build model runs one process per
compilation unit and links via a spawned linker, structurally, with no supported single-process
bypass in either case, even in "self-contained" builds. nife has no primitive for one program to
start another, wait for it, and read back what it produced.

This is a new syscall-surface question, and [DECISIONS
§10](../decisions/0010-capability-microkernel.md)'s own rule applies: "A method that does not fit the
model, or a brand-new syscall number, is a design fork, raise it before building it."

## This does not reopen §10

[DECISIONS §10](../decisions/0010-capability-microkernel.md) already decided the process model and
explicitly rejected Unix-shaped fork/exec, on an asymmetry: "capabilities to a Unix-shaped API" is
additive (a POSIX shim in userspace, built on top of capability handles, "nothing is thrown away"),
while the reverse, "Unix to capabilities," is "a rewrite, and historically it fails." §10 names
Fuchsia's `fdio` as exactly this: `open`/`read`/`write` on top of capability handles, not ambient
authority added to the kernel.

**This milestone is the additive direction §10 already endorsed, applied to one more Unix-shaped
convenience.** The functional need `cargo`/`rustc` (and any future job that shells out, e.g.
`kilo`/`nano`'s optional external spell-check, or a build tool invoking a linter) actually have is
narrow: start a program, hand it exactly the capabilities it should have (its stdin/stdout, specific
file grants, nothing ambient), wait for it to finish, and read back its exit status and output.
That is not "add fork/exec." It is the same shape as `fdio`: a userspace-visible convenience that
composes out of primitives the capability model already has, or a small, explicit extension to them,
not a back door around them.

## What already exists to build on

`crates/supervision_protocol`'s `build_child` (milestone 22 phase B.2, "the supervision tree: the
shared half") already constructs a child process with a specific, narrowed capability endowment and
lets a parent supervise it (see [DECISIONS §24](../decisions/0024-interrupting-the-foreground.md) for
how a shell already holds a child's interrupt endpoint). What is missing, checked directly against
that crate and against `kernel/src/user.rs`'s spawn path: a synchronous, ergonomic wait-and-collect
shape a caller like `cargo` would actually use in a loop ("run this, block until it's done, give me
its output"), as opposed to the death-notification/supervision-endpoint pattern built for long-lived
service supervision. The two may turn out to be the same mechanism used differently, or may need a
distinct narrow syscall; that sizing is this milestone's own work, not predetermined here.

## How a caller starts the child is decided

§219 (how the shell names an installed program to the spawner), decided 2026-09-26, gives this
milestone its start half. Option D is the shape: the caller sends an executable's bytes as frames it
owns, with the grants it chooses. A child whose digest the owner has not vouched for gets only those
grants and two read-only pages. Running it needs the D2 capability, which the owner gives a session.
So `cargo` runs a freshly linked test binary through the same request the shell uses. Waiting, exit
status and output stay this milestone's fork, and the gate above still names them.

## Spawn is the supported path, and `fork` is refused for good

§264 (`fork` is declined for good, and spawn is the supported way to start a program), 2026-10-08
(UTC), quoting calef: *"decline fork. support spawn. decided."* It makes this milestone the home of
the `posix_spawn`-shaped path for foreign programs. `std::process::Command::spawn` and a
`posix_spawn` shim for ported C are both thin layers over the request §219 settled, plus the wait
this milestone owes. Neither gets a milestone of its own. The shim also needs a C library, which
does not exist: §31 (the foreign-language seam: C holds no capabilities and makes no syscalls) and
the refused milestone 478 (tier three: full POSIX behind the foreign-language seam) stand in the
way, and that is calef's call, not this block's.

## What it needs

- The actual design fork, raised per DECISIONS §10 (process model: capability-based, microkernel),
  before any code: is this a new syscall method on an existing object, a new object type, or a
  userspace-only composition of `build_child` plus an existing notification mechanism (DECISIONS
  §101 (notification objects))? Answer with the six-questions discipline this
  tree already applies to forks like it (what does the tree already do in the analogous case, what
  does it cost measured rather than asserted, how reversible is it).
- Stdin/stdout/stderr as capabilities, not ambient file descriptors: a spawned child's I/O needs
  to be handed explicitly, the same "additive, not ambient" shape §10 already committed to.
- Exit-status delivery distinct from the death-notification path
  ([DECISIONS §24](../decisions/0024-interrupting-the-foreground.md)'s supervision shape, and
  [DECISIONS §32](../decisions/0032-reap-without-build.md)'s "a supervisor may collect a corpse
  without being able to build one"), since a caller waiting synchronously for one child's result is
  a different access pattern than a supervisor watching many.

## Why it matters

Directly: unblocks [milestone 173](0173-rustc-cargo-self-host.md) (`cargo` cannot spawn `rustc`, and
`rustc` cannot spawn a linker, without this). Indirectly: the same primitive is what nano's optional,
skippable spell-check/`execute command` feature ([milestone 170](0170-nano-editor.md)) would need if
that gap is ever closed, and what `git`'s optional hook/external-tool features
([milestone 171](0171-git-core-userspace.md)) would need if those are ever brought in scope.

## What this does not decide

The actual shape of the primitive (new syscall vs. composition of existing ones), whether it is
scoped to a single synchronous child or a small pool, and whether stdin/stdout are pipes, shared
memory rings, or something else. All of that is the DECISION this milestone's gate names.

## Index row

The real architectural piece the self-hosting question surfaced: `cargo`/`rustc` need to spawn a
process per compilation unit and to invoke a linker, structurally, with no bypass. DECISIONS §10
already rejected Unix-shaped fork/exec but explicitly endorsed the additive direction (Fuchsia's `fdio` shape); this is that direction applied to one more convenience. Gate: DECISION.
