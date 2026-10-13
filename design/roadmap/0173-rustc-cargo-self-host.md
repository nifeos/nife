---
status: NOT-STARTED
raised: 2026-08-25
milestone_dependencies: 172, 809, 801
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 173. `rustc`/`cargo`/LLVM natively on nife: full local self-hosting

The third of four self-hosting milestones, and the big one. It is sized by research rather than
guessed at: the blocker is not what it first looks like.

[Milestone 172](0172-capability-native-subprocess.md)'s capability-native
subprocess primitive is a hard prerequisite, not a convenience: `cargo` spawns `rustc` once per
compilation unit as its whole build model (verified via `rustc_driver`'s own use inside `clippy` and
`miri`, which still spawn a driver process per crate through
`RUSTC_WRAPPER`/`RUSTC_WORKSPACE_WRAPPER` rather than linking the compiler in-process), and `rustc`
spawns the system linker even in its "self-contained," bundled-`rust-lld` mode (checked against
rust-lang/rust's own linker-flavor work: the bundled linker still runs as a separate spawned
process, there is no shipped in-process linking path today). Neither has a supported bypass.

## The blocker is not the one it looks like

The size of this milestone invites the assumption that it repeats the Node.js/V8 finding (see
`notes/`, or ask whoever holds that research): a JIT wall nothing gets past. It is a different
wall. LLVM's default codegen backend emits object files ahead-of-time; a normal `cargo build`
never generates or executes machine code at runtime. This is not nife's static-`ET_EXEC`-only
loader's problem at all. The actual blocker is that fork/exec sits at the architectural center of
both `rustc`'s linking step and `cargo`'s entire build model, which is exactly what
[milestone 172](0172-capability-native-subprocess.md) exists to close.

## What else this needs, once 172 exists

- **LLVM itself, ported under [DECISIONS §31](../decisions/0031-foreign-language-seam.md).** Far
  larger in scope than any single-purpose interpreter: not one purpose-built engine but rustc's
  entire general-purpose backend, with its own build system, target infrastructure and optimization
  pipeline. Size this honestly before committing to it; do not assume it is "vim but bigger."
- Threading, tunable rather than blocking. `rustc` parallelizes codegen units across threads and
  `cargo` parallelizes the crate graph; both have a real, well-trodden single-threaded fallback
  (`-Z threads=1`, single codegen unit), already used by constrained/embedded toolchain builds
  elsewhere. Slower, not structurally blocked, and does not need reopening
  [DECISIONS §105](../decisions/0105-thread-spawn-decline-for-now.md) (`std::thread::spawn` stays
  declined until a customer needs it, not barred outright) the way a hard threading requirement
  would.
- **Dependencies already on disk, not fetched.** This tree already vendors rather than pulling from
  crates.io at build time (`patches/README.md`'s own convention); `cargo vendor` run ahead of time
  turns "needs network plus a registry" into "needs the source already present," a real but bounded
  prerequisite rather than a structural one.

## The honest scope signal: Redox OS's own history

Redox OS (github.com/redox-os), whose RedoxFS this tree already vendors, reached self-hosting in
January 2026: a real commit made entirely from within Redox, on the third attempt, requiring
kernel, signal, and networking work along the way. Redox already has a POSIX-shaped fork/exec model
and a relibc, materially closer to what `rustc`/`cargo` assume than nife's pure-capability design
starts from, and reaching self-hosting still took roughly a decade of project maturity. This is not
a reason not to attempt this milestone; it is the honest baseline for how large "full local
self-hosting" actually is, so it is scoped rather than promised.

## Why it matters

This is the milestone that would let calef build nife entirely on a running nife system, the literal
target of his own question. [Milestone 174](0174-nife-thin-dev-client.md) is the nearer-term
alternative that reaches "daily driver" sooner without this milestone's full scope.

## What this does not decide

Whether [milestone 172](0172-capability-native-subprocess.md)'s primitive, once built, is sufficient
as-is or needs extension once real `cargo`/`rustc` invocation patterns are tried against it; whether
LLVM is ported whole or some minimal subset targeted first; and the actual multi-year-or-shorter
timeline, which this block deliberately does not estimate given how little of milestone 172 exists
yet to measure against.

## Done when

calef, 2026-10-12 (UTC), under §297 (software reaches a nife system as a package `jig` installs
from basalt's index, the kernel included): "They should all be installed via jig through basalt."

`rustc`, `cargo` and the linker they spawn must run on nife installed by `jig install` from basalt's
package index. basalt builds the package against the nife commit it pins. The install waits on
milestone 809 (the package client becomes a program, `jig`, with the verbs an index needs). It also
waits on milestone 801 (packages over the internet: a host name, HTTPS and the distribution's
index). No work toward that starts until those milestones are built (calef, 2026-10-12 UTC: "For our
other programs let's block start on packaging.").

## Index row

The big one, sized rather than guessed at: not a JIT wall (LLVM's default codegen is
ahead-of-time, not runtime), but fork/exec sitting at the center of cargo's build model and
rustc's linking step. Redox OS, whose RedoxFS this tree already vendors, reached self-hosting in
January 2026 from a far more POSIX-friendly starting point and it still took roughly a decade of
project maturity, an honest scope signal rather than a discouragement.
