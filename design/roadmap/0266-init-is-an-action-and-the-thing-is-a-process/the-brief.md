# One progenitor: the brief as minted

An appendix to
[milestone 266 (one progenitor, on all three architectures)](../0266-init-is-an-action-and-the-thing-is-a-process.md).
It holds what the block asked of its lane before the build. That is the measurement it started
from, what a lane had to do rather than assume, the one difference and the contradiction it was
asked to resolve, and the limits it predicted. What the lane found is on the page.

## The measurement, so nobody starts by guessing

Taken 2026-09-08.

| what | count |
|---|---|
| the contract string `"init"` in `kernel/`, `crates/`, `xtask/` | 39 |
| `initboot` | 29 |
| `init_boot` and `INIT_BOOT` | 14 |
| `INIT_ROLES_ENTRY` | 8 |
| `init_least_authority_demo` | 3 |
| the word `init` in `notes/` and `design/` prose | 1,011 |
| files and directories named for it | 6, including three notes and `design/init-and-granular-spawn.md` |

The contract is small and the prose is the work. Thirty-nine sites is an afternoon. The 1,011 is a
judgment per occurrence and cannot be done by a pattern. *"userspace init brings up the console"*
is the role. *"trusted init"* is the title of milestone 22 (trusted init: verify it).
`notes/trusted-init.md` is a filename, and a great many are the ordinary English sense of
initialization.

This is precisely the shape a blind `sed` has damaged this tree with before. One swept a rename
tree-wide and rewrote the row recording that a name had been *refused*.

## What a lane must do rather than assume

- Read every prose occurrence. A pattern that changes 1,011 things is wrong by construction. Report
  how many were the role, how many were English, and how many were historical titles left alone.
- Leave records that describe the past. Milestone titles, dated audit rows and measurement tables
  recorded what was true under the name it had, and rewriting them makes the record lie. The `cred`
  rename on 2026-09-08 set that precedent and named the files it left alone.
- Rename the three notes and one design file only if their subject is the role, and say so either
  way. `notes/trusted-init.md` is milestone 22's record and its title may be history.
- Check the archive manifest in `xtask` first, not last. It is the copy no compiler checks, and it
  is what broke CI on the `job_mix_task` rename on 2026-09-05.

## The one genuine difference to reconcile

`crates/system_initializer` names it exactly: the boot entry is *"the one thing the two boards
genuinely disagree about, which is the order their kernels grant capabilities in."* One difference.
A `cfg` may be the right answer and the lane should argue it rather than reach for it.

## A contradiction to resolve on the way

`crates/system_initializer`'s header says the archive entry `init` *"is `fixtures/src/hello.rs`'s
`init_boot` role on aarch64 and this program on riscv64"*. Meanwhile `xtask`'s
`portable_archive_entries()` maps `("init", "builder")` and lists `system_initializer` as a separate
entry. One of those is stale and this block does not know which. Settle it and say so; a rename that
carries a wrong claim forward has spent the opportunity to find it.

## The limits it predicted

These were the block's `BUGS` before the build.

- This is a kernel-to-image contract, which AGENTS.md puts in the expensive category alongside a
  wire format. The edit is easy and the un-shipping is not. Nothing outside this repository has acted
  on it, which is the only reason it is cheap today.
- `init` is the one name a stranger arrives already knowing, and that cost is real rather than
  rhetorical. Every other rename in this sweep replaced a name nobody outside knew.
- The 1,011 figure is a word count, not a role count. Nobody knows how many are the role, and the
  lane's first honest deliverable is that number.
- `initboot`'s successor is unnamed. `progenitor-boot` is the mechanical answer and it is a
  mouthful. Naming the boot path for what it does rather than which role it hands to may be better.
  That is a decision this block defers rather than makes.
