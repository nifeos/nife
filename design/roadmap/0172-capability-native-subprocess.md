---
status: NOT-STARTED
raised: 2026-08-25
milestone_dependencies: none
decision_dependencies: 301
machine_requirements: none
specific_machine: none
needs_person: no
---
# 172. A capability-native subprocess primitive: what `cargo`'s "spawn a helper, wait, collect its output" needs, without fork/exec

The second of four self-hosting milestones, for developing nife on a nife host. This is the real
architectural piece. Research into what `rustc`/`cargo` (see [milestone
173](0173-rustc-cargo-self-host.md)) actually need found that their build model runs one process per
compilation unit and links via a spawned linker. Structurally there is no supported single-process
bypass in either case, even in "self-contained" builds. nife has no primitive for one program to
start another, wait for it, and read back what it produced.

This is a new syscall-surface question, and [DECISIONS
§10 (process model: capability-based, microkernel)](../decisions/0010-capability-microkernel.md)'s own rule applies: "A method that does not fit the
model, or a brand-new syscall number, is a design fork, raise it before building it."

## This does not reopen §10

[DECISIONS §10](../decisions/0010-capability-microkernel.md) already decided the process model and
explicitly rejected Unix-shaped fork/exec, on an asymmetry. "Capabilities to a Unix-shaped API" is
additive (a POSIX shim in userspace, built on top of capability handles, "nothing is thrown away").
The reverse, "Unix to capabilities," is "a rewrite, and historically it fails." §10 names
Fuchsia's `fdio` as exactly this: `open`/`read`/`write` on top of capability handles, not ambient
authority added to the kernel.

**This milestone is the additive direction §10 already endorsed, applied to one more Unix-shaped
convenience.** The functional need `cargo`/`rustc` actually have is narrow. So do any future jobs
that shell out, like `kilo`/`nano`'s optional external spell-check or a build tool invoking a
linter. Start a program; hand it exactly the capabilities it should have (its stdin/stdout,
specific file grants, nothing ambient). Wait for it to finish; read back its exit status and
output. That is not "add fork/exec." It is the same shape as `fdio`: a userspace-visible
convenience that composes out of primitives the capability model already has. Or a small, explicit
extension to them, not a back door around them.

## What already exists to build on

`crates/supervision_contract`'s `build_child` (milestone 22 phase B.2, "the supervision tree: the
shared half") already constructs a child process with a specific, narrowed capability endowment. It
lets a parent supervise it (see [DECISIONS §24](../decisions/0024-interrupting-the-foreground.md) for
how a shell already holds a child's interrupt endpoint). What is missing, checked directly against
that crate and against `kernel/src/user.rs`'s spawn path: a synchronous, ergonomic wait-and-collect
shape. A caller like `cargo` would use it in a loop ("run this, block until it's done, give me
its output"). The death-notification/supervision-endpoint pattern was built for long-lived
service supervision. §301 rules they are the same mechanism used differently: the wait is the §26
receive, the status rides `SYS_EXIT` into the `EVENT_EXIT` message, and there is no new syscall.

## How a caller starts the child is decided

§219 (how the shell names an installed program to the spawner), decided 2026-09-26, gives this
milestone its start half. Option D is the shape: the caller sends an executable's bytes as frames it
owns, with the grants it chooses. A child whose digest the owner has not vouched for gets only those
grants and two read-only pages. Running it needs the D2 capability, which the owner gives a session.
So `cargo` runs a freshly linked test binary through the same request the shell uses. The wait is
decided too: [§301 (exit status and output)](../decisions/0301-exit-status-and-output.md) puts the
status word on the death message.

## Spawn is the supported path, and `fork` is refused for good

§264 (`fork` is declined for good, and spawn is the supported way to start a program), 2026-10-08
(UTC), quoting calef: *"decline fork. support spawn. decided."* It makes this milestone the home of
the `posix_spawn`-shaped path for foreign programs. `std::process::Command::spawn` and a
`posix_spawn` shim for ported C are both thin layers over the request §219 settled, plus the wait
this milestone owes. Neither gets a milestone of its own. The shim also needs a C library, which
does not exist. §31 (the foreign-language seam: C holds no capabilities and makes no syscalls) and
the refused milestone 478 (tier three: full POSIX behind the foreign-language seam) stand in the
way. That is calef's call, not this block's.

## What it needs

- The design fork is decided:
  [§301 (exit status and output)](../decisions/0301-exit-status-and-output.md), 2026-10-15, carries
  the status word on `SYS_EXIT` into the death message, and keeps output grant-routed. The
  primitive is a userspace composition of `build_child` plus the receive on the supervision
  endpoint. What remains here is §301's one kernel touch on the exit path: `sched::exit` forwards
  `SYS_EXIT`'s first word, and `user_mode_runtime` passes the code it is given. Then the loop:
  spawn, endow, receive, read the code, and return the output the grants carried.
- Stdin/stdout/stderr as capabilities, not ambient file descriptors: a spawned child's I/O needs
  to be handed explicitly, the same "additive, not ambient" shape §10 already committed to.
- The status rides the death message ([§26 (the fault
  endpoint)](../decisions/0026-fault-endpoint.md)'s `EVENT_EXIT` words, per
  [§301 (exit status and output)](../decisions/0301-exit-status-and-output.md)), so no separate
  status channel exists to build. The synchronous wait a caller like `cargo` uses is a receive on
  the supervision endpoint it already holds, with
  [DECISIONS §32](../decisions/0032-reap-without-build.md)'s "a supervisor may collect a corpse
  without being able to build one" (`REAP`) behind it.

## Why it matters

Directly: unblocks [milestone 173](0173-rustc-cargo-self-host.md) (`cargo` cannot spawn `rustc`, and
`rustc` cannot spawn a linker, without this). Indirectly: the same primitive is what nano's optional,
skippable spell-check/`execute command` feature ([milestone 170](0170-nano-editor.md)) would need if
that gap is ever closed. It is also what `git`'s optional hook/external-tool features
([milestone 171](0171-git-core-userspace.md)) would need if those are ever brought in scope.

## What this does not decide

The build shape of the loop is this milestone's own work. How do stdin/stdout present themselves to
the child (pipes, rings, or endpoints)? Is the first consumer scoped to one synchronous child or a
small pool?
[§301 (exit status and output)](../decisions/0301-exit-status-and-output.md) fixed only where the
status travels and that output stays grant-routed.

## Index row

The real architectural piece the self-hosting question surfaced: `cargo`/`rustc` need to spawn a
process per compilation unit and to invoke a linker, structurally, with no bypass. DECISIONS §10
already rejected Unix-shaped fork/exec but explicitly endorsed the additive direction (Fuchsia's `fdio` shape); this is that direction applied to one more convenience. The wait is decided:
[§301 (exit status and output)](../decisions/0301-exit-status-and-output.md) carries the status
word on `SYS_EXIT` and the death message.
