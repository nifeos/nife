---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 345. Five crates whose doctests the host gate never runs

Filed 2026-09-03 as an unnumbered proposal by the milestone 247 sweep,
from milestone 68's block; numbered 2026-09-19 by milestone 433. **Premise re-checked 2026-09-19 and
it holds.** `xtask/src/main.rs`'s host test invocation still excludes `user_mode_runtime`,
`hot_swap_contract`, `virtio`, `supervision_contract` and `system_initializer` by name (alongside
`kernel`, `components` and `fixtures`). The comment above it still carries the whole account. The
exclusions are `user_mode_runtime` and everything that depends on it, because `--exclude` removes a
package from the test selection and not from the dependency graph. No crate has been split.

No decision is owed and nothing is missing. The split is ordinary refactoring inside
crates this tree owns, and the gate that would prove it is the one already running.

**In brief.** `hot_swap_contract`, `virtio`, `supervision_contract` and `system_initializer` each take an
unconditional dependency on `user_mode_runtime`, which is EL0 syscall `asm!` and cannot compile for the host.
`--exclude` removes a package from the test selection but not from the dependency graph, so all four
are excluded by name in `xtask`, along with `user_mode_runtime` itself. Split each one so the pure logic lives
where the host can build it and the syscall half is what depends on `user_mode_runtime`. Then the host pass
runs their tests and their doctests instead of skipping them.

## Why this matters

Milestone 68 exists to be the gate that keeps documented examples honest. Inside that gate sit five
crates whose examples nothing ever compiles, so an example there can go stale and no check will say
so. That is the exact failure the milestone was built against, living in the milestone's own
blind spot.

The tree has already paid for this class of gap once, and the block records the bill. When five
crates went missing from the host selection by milestone 51 (wall-clock time, the `date` command, and an NTP service), `filesystem_contract`, `compositor`,
`video_terminal`, `bitmap_font` and `grant_plan` carried **82 host tests the gate never ran**. All
82 passed when they were finally run, which is the point: nothing failed, so nobody noticed, and a
gate that quietly covers less than it claims is worse than no gate because it is trusted.

There is a second reason, and it is the reason the exclusion list exists at all. On 2026-08-03 those
unconditional dependencies broke the host build on x86_64 and nobody saw it, because CI had moved to
`ubuntu-24.04-arm` the same day and an aarch64 host builds `user_mode_runtime` by accident. A stranger with a
clean x86_64 checkout found it eleven days later, on milestone 117's first run, which is principle 3
failing in the only way it can be observed. `script/lint` now derives the exclusion set from `cargo
metadata` so the next crate to take a `user_mode_runtime` dependency breaks the gate rather than the host
build. That is a tripwire on the growth of the problem, not a fix for it.

## What it would take

Five crates, each a separate piece of work, and they are not equal. `hot_swap_contract` and
`supervision_contract` are protocol crates whose pure half is message layout and whose syscall half is
the `CALL`, which is the cleanest shape. `virtio` and `system_initializer` carry more. The measure
of success is mechanical and already automated: a crate leaves the `--exclude` list in
`xtask/src/main.rs`, `script/lint`'s derived-set check agrees, and the host pass runs its doctests.

## Where it came from

Milestone 68 (code-quality gates: one lint policy, and the lints that lost)'s block: *"Split the pure half from the syscall half in `user_rt`, `hot_swap_contract`,
`virtio`, `supervision_contract` and `system_initializer`. Each takes an unconditional `user_rt`
dependency, so the host test selection excludes it and nothing in CI ever runs its doctests. That is
five crates whose examples can rot unnoticed inside the gate milestone 68 exists to be."*

`user_rt` is `user_mode_runtime` since 2026-09-13 (milestone 285). The quotation keeps the name
milestone 68 wrote; everywhere else on this page the crate is spelled the way it is spelled today.

The exclusion and its history are commented at the host-test invocation in `xtask/src/main.rs`.

## Index row

`hot_swap_contract`, `virtio`, `supervision_contract` and `system_initializer` each take an
unconditional dependency on `user_mode_runtime`, which is EL0 syscall `asm!` and cannot compile for
the host, so all four are excluded from the host pass by name along with `user_mode_runtime` itself.
Splitting each so the pure logic lives where the host can build it and the syscall half is what
depends on `user_mode_runtime` lets the host pass run their tests and their doctests instead of
skipping them. Milestone 68 exists to be the gate that keeps documented examples honest, and inside
that gate sit five crates whose examples nothing ever compiles, which is the milestone's own failure
living in its own blind spot. The tree has paid for this class once already. When five crates went
missing from the host selection by milestone 51 they carried 82 host tests the gate never ran, and
all 82 passed when finally run. That is the point: nothing failed and nobody noticed. The
same unconditional dependencies broke the x86_64 host build on 2026-08-03 and a stranger found it
eleven days later. Success is mechanical: a crate leaves the `--exclude` list, `script/lint`'s
derived-set check agrees, and the host pass runs its doctests.
