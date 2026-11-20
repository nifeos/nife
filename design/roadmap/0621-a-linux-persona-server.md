---
status: NOT-STARTED
raised: 2026-09-29
promoted_from: a-linux-persona-server
milestone_dependencies: none
decision_dependencies: 195, 197, 208, 220
machine_requirements: none
specific_machine: none
needs_person: no
---
# 621. A Linux persona server, confined

From calef's question on 2026-09-29, "could nife run podman?". The binary cannot, and this file
records the thing worth building instead, in two rungs.

The big rung: a userspace server that implements the Linux system call surface by translation onto
nife capabilities, one instance per guest, itself spawned and confined like any other program.
The guest's binaries run against the persona; the persona holds the guest's capabilities, pages
and devices; the kernel never learns that Linux exists. The shape is gVisor's Sentry and WSL1's
(from memory, not re-read for this milestone), with one advantage neither has: here the persona
itself is confined by capability, and gVisor on Linux cannot confine itself.

The small first rung: an OCI import shim. It translates an image's payload into a nife package.
That is one §197 (a package is one archive file) archive with a §195 (a reviewed recipe vouches
for a package) recipe, entering the §208 (installing a package is granting it) installed-programs table
like any package. This is repackaging, flatpak-shaped: it runs software rebuilt for nife rather
than arbitrary Linux binaries, and it makes the distribution half of the container story native
without promising the compatibility half.

## Refused, with reasons

- Porting podman, or any Go container engine. The Go runtime is a Linux program (futex, clone,
  epoll), and podman's substance is kernel subsystems (namespaces, cgroups, overlayfs, seccomp)
  rather than libraries it links. A runtime port plus a kernel personality buys the protocol and
  not the confinement.
- OCI runtime protocol compatibility: speaking to runc, or being driven by podman. The value is
  running the software, not wearing the engine's interface. The shim imports; the persona
  executes; nothing imitates a container engine.

## What serving containers must match

Import is the chosen shape because capabilities and coexisting package versions give isolation
comparable to a container's or stronger. calef, 2026-10-18 (UTC), asked whether that holds. For a
private filesystem view (subtree grants, with no ambient filesystem), for not seeing or signaling
other processes, for dependency versions, for kernel surface and for a memory limit, it does. Two
things a container gives and nife does not yet have, and container serving needs both before it
claims parity:

- **A network grant scoped to addresses and ports.** The manifest's `network` is one bit today:
  all of the network, or none. A container gets its own address, its own ports and firewall rules.
- **A CPU budget the scheduler enforces.** Milestone 282 (a thread's CPU time, and the `top` it
  makes possible) measures a thread's CPU time; nothing limits it. A container gets `cpu.max`.

An I/O bandwidth limit (`io.max`) is not required yet. An imported Linux binary runs under the
persona, so its isolation is also the persona's confinement, which this milestone builds.

## Where it ranks

Strategic, not next. The customer path is vacant, and the claim that nobody needs nife (a non-fatal
note in fatal risk 8's appendix) is answered by a workload rather than by code. The predecessors are
queued: the std port as the native software story, §220 (signed builds) as the trust an imported
binary needs, and the packaging milestones as the distribution shape. This file records the decision
and starts nothing ahead of them.

Name provisional.

## Index row

A confined userspace server that implements the Linux system call surface by translation onto nife
capabilities, one instance per guest, so the kernel never learns that Linux exists. A first rung
imports an OCI image's payload as an ordinary nife package, which makes the distribution half of
the container story native without promising the compatibility half. This answers the question
behind calef's ask, and it keeps the one advantage gVisor's Sentry cannot have: the persona itself
is confined by capability.
