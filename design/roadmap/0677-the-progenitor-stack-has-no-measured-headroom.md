---
status: BUILT
raised: 2026-09-26
built: 2026-09-27
promoted_from: the-progenitor-stack-has-no-measured-headroom
---
# 677. The progenitor's stack has no measured headroom

Status BUILT 2026-09-27: built on PR #1409. *(Title and slug are drafts.)*

Raised by milestone 600 (the graphical terminal stack is built in userspace), whose growth of the
progenitor's frame overflowed the progenitor's stack.

A measurement, then a constant or a gate.

## The finding

The progenitor runs on eight stack pages (`kernel::user::INIT_STACK_PAGES`, whose doc calls 32 KiB
"generous"). `system_initializer::boot`'s own frame is 12,768 bytes in the debug build `swish-check`
boots (`sub sp` in its prologue, aarch64), and it lives for the whole boot because the spawn service
runs inside it.

Growing that frame by 560 bytes makes `swish-check` on aarch64 die at `package install greeting`
with a data abort 24 bytes below the stack's last page (`far 0x4f8fe8`, `sp 0x4f8fe0`), while main
passes the same script. So main's deepest path reaches within roughly 540 bytes of the guard,
inferred from those two numbers rather than measured.

Nothing measures this stack. `script/stack-depth-check` walks kernel thread stacks only, and
`script/stack-frame-check` gates single frames.

## What to find out, then build

1. Measure the progenitor's high-water mark on `swish-check` (a watermark like milestone 84 (stack high-water: measure kernel stack depth), or a
   call-graph walk from `_start`).
2. Then either raise `INIT_STACK_PAGES` with the number beside it, or gate the depth, so the next
   lane that adds a local to `boot` fails loudly instead of at a prompt.

## Built

*Names below are provisional.*

**Measured.** A kernel gauge, `kernel/src/progenitor_stack.rs`, paints the progenitor's stack pages
as `boot_progenitor` maps them and prints the high-water mark from the idle loop (the yield syscall
on `x86_64`) each time it settles at a new peak. On `script/swish-check`, out of 32,768 bytes:

| | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| debug, at the prompt | 19,000 | 18,976 | 18,184 |
| debug, `package install` | **32,440** | **32,184** | **31,304** |
| release, `package install` | 16,432 | 16,544 | not run |

So eight pages left 328 bytes to spare on aarch64 debug, against the 540 inferred above. `boot`'s frame is 12,848 bytes of it in debug and stays live under the
spawn service.

**Gated.** `kernel::progenitor_stack::HEADROOM_FLOOR` is two pages. A boot that leaves less prints
`BELOW`, and `script/swish-check` fails on it, naming the command; it also fails if the gauge never
prints. `swish-check` strips the gauge's lines out of the transcript as they arrive, since they land
between a prompt and the next echo.

Fixed with the numbers. `INIT_STACK_PAGES` went from 8 to 12: the debug peak is now 66%, and the
gate fires 8.5 KB above it. The more elegant fix is to stop the spawn service standing on `boot`'s
frame, and that is proposed separately
([0696-the-spawn-service-runs-outside-boots-frame.md](0696-the-spawn-service-runs-outside-boots-frame.md))
because four open lanes were editing those functions. By the elegance test the raise wins on effort
and collision, not on elegance, and the note says so.

Also: `script/swish-check --release` (aarch64 and riscv64 tested), which the release column needed,
and `cargo xtask initrd-riscv` now packs the profile it builds.

notes/stack/progenitor-stack.md has the frames, the decision and the `BUGS`.

## Follow-on

- **Milestone 696.** Milestone 696 (the spawn service runs outside `boot`'s frame). The more elegant fix than a bigger stack, running the spawn service outside `boot`'s frame: `design/roadmap/0696-the-spawn-service-runs-outside-boots-frame.md`.

## Index row

On eight stack pages the progenitor had 328 bytes to spare, and a 560-byte frame growth overflowed it. Built: a kernel gauge reports the stack's high-water mark, and `script/swish-check` fails a boot that leaves less than two pages.
