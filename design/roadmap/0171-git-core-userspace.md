---
status: NOT-STARTED
raised: 2026-08-25
milestone_dependencies: 809, 801
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 171. `git` core plumbing: version control on a nife host with no new primitive

The first of four milestones answering calef's self-hosting question: "how could I shift nife
development onto a nife host, so that would have me using nife daily." They are split by
tractability rather than proposed as one undertaking; this is the piece that needs the least.

Software-only. Informed by [milestone 169](0169-kilo-editor.md) (`kilo`, the first
real C program against [DECISIONS §31](../decisions/0031-foreign-language-seam.md)'s seam) but not
hard-gated on it: git core needs the identical seam pattern kilo proves, not the terminal
raw-input primitive kilo exists to build, so the two can proceed in parallel.

## What "git core" means here, precisely

Checked against git's own architecture, not folklore: the object database (`.git/objects`, content-
addressed blobs/trees/commits), ref updates (`.git/refs`, `.git/HEAD`), and the index file are all
**direct file I/O with no subprocess involved**. `init`, `add`, `commit`, `diff`, `log`, `branch`,
and `checkout` need zero fork/exec in a default configuration. Everything that *does* spawn a
process is opt-in and off by default: the pager (`core.pager`, off with `--no-pager`), hooks (none
unless installed), textconv filters, credential helpers, and external diff/merge tools. Git is
normally static-linkable, with no hard threading requirement for these operations.

This milestone is exactly that surface: the plumbing and the porcelain built directly on it,
scoped to local, single-repository, no-subprocess operation. Not in scope: anything that shells out
by design (hooks, external merge drivers), and anything that needs the network (`clone`/`fetch`/
`push` over a wire protocol, which is a separate milestone's problem, likely shared with
[milestone 174](0174-nife-thin-dev-client.md)'s remote-build work once that exists).

## What it needs

- The same [DECISIONS §31](../decisions/0031-foreign-language-seam.md) treatment as `kilo`: git's C
  source rewritten against nife's Rust-mediated shim rather than making syscalls directly, scoped to
  the no-subprocess plumbing above.
- Capability-scoped file I/O in place of git's usual global-path assumptions, the same translation
  every C port on nife needs (`files.c`-shaped work, matching how milestone 169's own scoping section
  describes `kilo`'s file handling).
- Nothing from milestone 169's raw-terminal-input primitive: git's core commands are not a screen
  editor, they read arguments and print output through the existing `OPERATION_WRITE` ANSI-passthrough path
  ([DECISIONS §21](../decisions/0021-terminal-in-userspace.md)).

## Why it matters

Directly: it is the second piece, after an editor, of "edit and version-control natively on a nife
host," which is the load-bearing precondition for [milestone 174](0174-nife-thin-dev-client.md)'s
thin-development-client path, the nearer-term alternative to full local self-hosting
([milestone 173](0173-rustc-cargo-self-host.md)).

Indirectly: git is the second real, non-editor C program to prove the §31 seam holds for something
with real users depending on correctness (a corrupted object database is a worse failure than a
cosmetic editor bug), which is evidence worth having before larger C/C++ ports
([milestone 172](0172-capability-native-subprocess.md), 173) are attempted.

## What this does not decide

Whether `clone`/`fetch`/`push` (needing a network client, and eventually
[DECISIONS §31](../decisions/0031-foreign-language-seam.md)-shaped work over whatever wire protocol)
are folded into this milestone or left for whoever builds milestone 174's remote-development
protocol, since the two may end up sharing a network-client crate. Left for whoever picks this up.

## Done when

calef, 2026-10-12 (UTC), under §297 (software reaches a nife system as a package `jig` installs
from basalt's index, the kernel included): "They should all be installed via jig through basalt."

`git` must run on nife installed by `jig install` from basalt's package index. basalt builds the
package against the nife commit it pins. The install waits on milestone 809 (the package client
becomes a program, `jig`, with the verbs an index needs). It also waits on milestone 801 (packages
over the internet: a host name, HTTPS and the distribution's index). No work toward that starts
until those milestones are built (calef, 2026-10-12 UTC: "For our other programs let's block start
on packaging.").

## Index row

The second of four self-hosting milestones from calef's own question about developing nife on a
nife host. Git's object database, refs and index are direct file I/O with no subprocess involved
by default; `init`/`add`/`commit`/`diff`/`log`/`branch`/`checkout` need zero fork/exec. Nano-class
DECISIONS §31 work, no new kernel primitive.
