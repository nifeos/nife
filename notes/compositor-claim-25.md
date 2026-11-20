# Confinement claim 25, attacked part by part

Milestone 719 (compositor confinement claim 25), 2026-10-03 UTC. Row 25 of
[confinement-claims.md](confinement-claims.md) says a client cannot reach its neighbor's pixels or
read the screen. The outsider pass of milestone 633 (an outside agent attacks the confinement claim)
left it alone, because a userspace server enforces it and the kernel does not.

Row 25 cited §66 (a refusal is a non-zero status, and not the same one an error gets), the shell's
decision. The compositor's is §33 (the compositor's authority is memory, not messages), and the row
now says so.

## What the one test proved, and what it could see

`compositor_tests::a_client_holds_no_capability_for_its_neighbours_pixels_or_the_screen` proves four
things in one body. One recorded patch reaches one of them. A count of "one of four" was fair, and a
read of each part shows it was also generous.

| Part | What the original could see |
|---|---|
| 1. The input slot is empty (`NoSuchSlot`) | Reachable and first, with no patch recorded. |
| 2. A write at the neighbor's first pixel page faults | The one patched part. A write cannot tell "unmapped" from "mapped read-only", so a read-only exposure of the neighbor's pixels left it green. |
| 3. The victim's pixels are unchanged | Behind the fault wait, so it can fire only after part 2's assertion already has. |
| 4. A client cannot read the screen | The peeper never asked for a mapping, so a screen capability leaked to every client left it green. A grant alone maps nothing. |

## One test and one patch per part

A patch is recorded per test, so each part became a test. Every patch was applied by hand, the one
test run, and the result read: red on aarch64, 2026-10-03. None was replayed on riscv64 or x86_64,
and `script/falsifications --sweep system_tests` was not run over the package.

| Part | Attack | Outcome | What turns it red |
|---|---|---|---|
| 1 | Receive on the slot a non-focusable client was not granted. | Held. | A write-only capability in slot 2 gives `NotPermitted` (-3). |
| 2 | Read the neighbor's control page, first pixel page and last pixel page, one attacker each. | Held. | The neighbor's first pixel frame mapped read-only. |
| 3 | Commit nine lying damage rectangles, then compare the victim's surface and control page, and the screen. | Held for reach, broke for availability, fixed. | Two patches, below. |
| 4 | Ask for the screen mapping as a capture client would, then read. | Held. | The screen capability and its map budget leaked to a window client. |

On the part 2 patch the original test stays green. Its probe is a write, a write to a read-only page
faults at the same address, and its digests move only if a write lands. That is the hole the new test
closes.

The tests are `a_client_with_no_input_grant_finds_nothing_in_the_input_slot`,
`a_client_cannot_read_any_page_of_its_neighbours_it_can_name`,
`a_lying_damage_rectangle_changes_nothing_of_its_neighbours`,
`a_lying_damage_rectangle_cannot_stop_the_compositor_or_misdraw_the_screen` and
`a_client_granted_no_screen_capability_cannot_map_or_read_the_screen`. The patches sit beside the
original's in `system_tests/falsifications/`, and each opens with what it does and does not reach.

## The one thing that broke, and what it was not

`Rect::right` and `Rect::bottom` were `x + w as i32`. The damage rectangle comes straight out of a
client's control page and reaches them before any clip, so `(i32::MAX, 0, 1, 1)` overflows. In a debug
build, which every test leg runs, the compositor panics. One doorbell serves every client, so one
lying client takes every window down.

The host showed the panic first. On the machine, with the fix reverted, the liar's next `CALL` is
answered with an error and it reports `0xDEAD_..04`.

It is not an escape. A release build wraps, the clip to the surface still bounds the result, and no
lie reached a pixel outside the liar's own window. The host test
`a_lying_damage_rectangle_neither_panics_nor_leaves_its_surface` checks that for seven origins crossed
with seven extents against every window in the scene. The claim's sentence held. What broke is
availability, which [compositor.md](compositor.md) already says the rung does not defend. It also
corrects a sentence there: "cannot be worse" was true of reads and false of the process surviving.
The fix is saturating arithmetic in `crates/compositor`, with no wire-format or system call change.

## What part 3's patches reach

The control-page test is red under a compositor that writes `STATUS` into the last client's control
page. The compositor can write every client's memory, so the claim is that the address it writes never
depends on what a client said. The victim's surface digest in that test stays equal under the patch, so
the surface half is not shown able to fail. The compositor writes only control pages today, and a patch
that made it write a surface has not been recorded.

The screen test is red under the reverted arithmetic. Its screen comparison runs only when the liar
finishes, so it has not been shown to fail on its own either.

## Attacks not built, and why

- **Teardown races.** A client that dies mid-commit leaves the compositor reading frames that stay
  allocated: the kernel owns every client frame for the compositor's lifetime and nothing frees one.
  That is a reason the attack cannot work today, read from `compositor_service::start`, and not a
  test. It stops holding once clients can be torn down and their frames reused.
- **Badges and buffer grants.** The protocol has neither. Its two verbs are content-free, and every
  per-client fact lives in memory that one client alone maps. The attack surface is the control page.
- **Slot reuse.** See the limit below.

## BUGS

- A spawned client's surface is not scrubbed. `Wiring::spawn_client` maps a slot's frames as they
  are, so a client spawned into a slot a previous client used would see that client's last pixels
  until it painted. Read from the code, not run. Nothing respawns a client today and the frames are
  zeroed once at `start`, so it is a limit of the wiring. It becomes a hole the day a window can be
  closed and its slot reused. Also recorded beside `spawn_client`.
- The patches are recorded on aarch64 only.
