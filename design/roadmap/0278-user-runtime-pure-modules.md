---
status: NOT-STARTED
raised: 2026-09-12
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 278. Two modules are fenced out of the prover by a `Cargo.toml`, not by what they do

A packaging change. Nothing here is a design fork, and it is reversible: the modules
move back by moving the files back.

**In brief.** `crates/user_rt` is excluded from the host pass, from coverage, from mutation and from
Kani, and correctly so: it is the EL0 syscall floor, and `svc` on a machine with no nife kernel
under it is a fault rather than a syscall. The exclusion is crate-level. Two of the crate's
modules make no syscall at all, and they inherit the exclusion because of the file they are declared
in.

## What is actually in there

Measured 2026-09-12, on `4306c8c1`.

| Module | Lines | Syscalls | Consumers | Excluded for a real reason? |
|---|---|---|---|---|
| `mapped_window` | 176 | **0** | **29 files** | No. Pointer arithmetic with bounds checks |
| `initial_ramdisk` | 60 | **0** | 9 files | No. A `'static` slice over a fixed VA |
| `heap` | 220 | yes (`memory_region::MAP`) | 7 files | Yes |
| `virtio` | 53 | yes (5 sites) | 4 files | Yes |
| core (`invoke` and its wrappers) | ~500 | yes, it *is* the syscall | all | Yes |

`mapped_window` is the most-used thing in the crate and the least entitled to the exclusion.

## Why this matters, and it is the verification thesis rather than tidiness

`MappedWindow::check` is one line:

```rust
assert!(off.checked_add(size).is_some_and(|end| end <= self.len), ...);
```

A bounds check with an overflow guard, standing between 29 call sites and a write past a mapped
page. It is exactly what a bounded model checker is good at, and `crates/user_rt` has zero Kani
harnesses today because nothing in it can be reached.

That is milestone 191's finding in miniature. 191 found that no harness in this tree had ever caught
a defect after the day it was written, because one line of `script/verify`'s own header meant
`cargo kani` never compiled the kernel, putting 64,818 lines out of reach by construction.
Milestones 193 and 197 fixed that for `kernel/src` and for `user/`. This is the same shape one
scale down: a safety-relevant abstraction, used at 29 sites, unreachable because of where it is
declared rather than because of what it does.

The same boundary also costs coverage and mutation on 236 lines that both tools could read.

## The work

Move `mapped_window` and `initial_ramdisk` into a crate that compiles for the host. Leave `heap`, `virtio`
and the core where they are, and leave the exclusion on `user_rt` exactly as it stands. Then let the
three tools that were always able to read this code read it: a harness on the bounds check, coverage,
and mutants.

The new crate's name is an architect's. A lane ships a provisional one and says so.

## What makes this more than a file move

Four exclusion lists, and milestone 244 already found them drifting. `script/lint`'s two clippy
invocations, `xtask`'s `test`, `script/coverage`'s exclusions, and `.cargo/mutants.toml`'s, the last
two being the ones that report a number. `.cargo/mutants.toml` says in its own head comment that its
list "deliberately mirrors script/coverage's exclusions"; a mirror maintained by asking the next
person to keep two files in step is rung four.

`script/lint`'s own gate cuts the right way here and must keep cutting. It asks cargo which
workspace members reach `user_rt` and fails any new consumer, after a breakage that lived from
2026-08-03 to 2026-08-14 invisible because CI had moved to an aarch64 runner where the EL0 assembly
compiles by accident. The new crate must not depend on `user_rt`, which is the whole point, and the
gate is what proves it did not.

29 files change an import path, from `user_rt::mapped_window` to the new crate. Mechanical, and
this tree has a scar about mechanical renames: a blind `sed` once rewrote the very row recording that
a name had been refused. Read before sweeping.

## The refusals, which are the valuable half

- Make the runtime a program. Refused on a bootstrapping impossibility rather than on taste:
  `invoke` *is* `svc`/`ecall`, and IPC is built on it, so a program that had to send a message to
  reach the syscall layer would need the syscall layer to send the message. The upper modules escape
  that circle and still lose, for the opposite reason: they make no syscalls, so routing them
  through a server would add syscalls to code that has none, and a `GlobalAlloc` that performs IPC
  per allocation is its own disaster. In a capability system the program already holds its budget;
  there is nothing for a server to arbitrate.
- Move `heap` as well. It calls `memory_region::MAP` (`heap.rs:163`). It belongs with the runtime.
- Move `virtio` as well. Five syscall sites, device-specific, and already opt-in and scoped for
  the reason `mapped_window`'s own header gives.
- Split `user_rt` further, by concern. Refused as gold-plating. The line this milestone draws is
  falsifiable and mechanical, *does this code make a syscall*, and no second line in there is.
- **Do nothing, and record it in `BUGS` instead.** Defensible, and it is what the tree has done by
  accident until now. The cost of continuing is that a bounds check standing behind 29 call sites
  stays unprovable, on a project whose thesis is a machine-checked capability core.

## BUGS

- **This does not settle whether `user_rt` keeps its name.** `rt` is an abbreviation needing a
  decoder, the first failure mode `AGENTS.md` names, and the 2026-08-23 batch expanded eleven such
  names in one pass (`fs_proto` to `filesystem_protocol`, `cred_proto` to `credential_protocol`). That
  ruling is an architect's and is independent of this move; whichever way it goes, the crate being
  split is the same crate.
- **A harness is not yet a proof of anything useful.** Reaching the code is a precondition. Milestone
  194's falsification discipline (§134) is what then says whether the harness could ever come back
  red, and a harness over `checked_add` that cannot be falsified is chaff of exactly the kind 191's
  reverse pass found.
- The consumer count is today's. 29 files reach `mapped_window` on `4306c8c1`; the tree grows,
  and the number is a scale rather than a fixed cost.

## Follow-on

- **Milestone 393.** Four places record which crates cannot compile for the host and this
  milestone touches all four, so it would have made a fifth. Deriving that set once, the way
  `script/lint` already derives it for its own check, closes the class rather than this instance.
  Not scoped here; numbered on 2026-09-19 by milestone 433's drain of the pile.

## Index row

`crates/user_rt` is excluded from the host pass, coverage, mutation and Kani because it is
the EL0 syscall floor, and the exclusion is crate-level: `mapped_window` (176 lines, 29 consumers)
and `initial_ramdisk` (60 lines) make no syscall at all and inherit it from the file they are declared
in. `MappedWindow`'s one-line bounds check with an overflow guard stands behind 29 call sites and
cannot be reached by a prover; the crate has zero harnesses. Milestone 191's finding one scale
down. Touches the four exclusion lists milestone 244 found drifting. Does not settle whether `user_rt` keeps its name.
