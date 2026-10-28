---
status: NOT-STARTED
promoted_from: release-userspace-in-board-images
raised: 2026-10-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: riscv64 silicon
specific_machine: radon (every job-mix and soak number on record was taken there)
needs_person: yes
---
# 797. Release userspace in board images

Raised 2026-10-05 (UTC) by `lane/radon-2026-10-05-record`. Title and slug are drafts.
`needs_person` is yes only because a new baseline is boots on radon.

## What was seen

`script/board-image` builds the kernel with `cargo build --release` and the userspace archive with
`cargo xtask initial-ramdisk-riscv`, which packs `target/<target>/debug/` because nothing sets xtask's
`RELEASE` flag on that path (`xtask/src/archive.rs`, `profile_dir()`). So every radon payload runs a
release kernel under debug programs. The job mix measures user code as well as the kernel's: a
syscall's userspace stub, `compute`'s grind, the spawn job's loader.

On 2026-10-05 the stub alone was worth 3 to 8 ticks of a 110-tick `null_syscall` job, depending on
the build ([`notes/job-mix/radon-2026-10-05.md`](../../notes/job-mix/radon-2026-10-05.md)). A
debug stub is larger and spills more, which makes it both dearer and more sensitive to where it
lands than the shipped code would be.

## Why it is not a one-line fix

Every job-mix and soak number recorded from radon was taken with debug userspace. Changing
the profile changes all of them at once. The change has to come with a new baseline, taken
interleaved against the old payload on the same evening. Every note that quotes a radon number
has to say which side of the change it is on. `swish-check --release` already met the same mismatch
and fixed it for its own path (the comment at `has_initial_ramdisk_riscv`).

Reuse: xtask's existing `RELEASE` flag and `profile_dir()`, which `bench`, `soak` and `install`
already set; nothing new is built.

## Done means

- `script/board-image` packs release userspace, by default, and says so in its header; the old
  behavior reachable by a flag only if a reader needs to reproduce an old payload.
- A radon evening of interleaved boots, old payload against new, that records the job mix's shift
  for every kind, and a line in `notes/job-mix.md` dating the change.
- The `BUGS` entry in `script/board-image` that points here, removed.

## Index row

`script/board-image` builds a release kernel but packs debug userspace, so every radon payload measures a debug syscall stub worth 3 to 8 ticks of a 110-tick job. Building release userspace makes the board numbers match what a customer would run, and starts a new radon baseline.
