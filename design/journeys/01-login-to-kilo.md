---
status: DELIVERED
raised: 2026-08-26
---
# 1. Boot to a login prompt, into `swish`, to editing a file with `kilo`

calef, 2026-08-26: boot to a login prompt, enter a username and password, land in a `swish` prompt
on a terminal worth using, and create and edit a file with `kilo`. The first journey tracked here,
and the reason the directory exists: no single milestone was blocked when this was asked, but
tracing the whole story end to end found a real, unowned gap (milestone 177) that scanning the
roadmap milestone by milestone never surfaced.

`swish` itself is not a step below: it already exists and is already what the real interactive boot
spawns unconditionally today (`crates/system_initializer::boot`), so nothing on this journey is
waiting on it. The gaps are everywhere else.

| step | milestone | decision | what this step needs |
|---|---|---|---|
| 1 | 29 | | the framebuffer contract, bitmap font, VT engine, and virtio keyboard driver: proven, but only under the test harness |
| 2 | 33 | | the compositor: multiplexing the screen, proven under the test harness |
| 3 | 177 | | attach the GPU/keyboard devices to the real interactive boot and swap `console`/`input` for `display_terminal`/`compositor`; on x86_64 this also needs a real interactive-boot entry point first, which does not exist on that architecture at all yet |
| 4 | 49 | 120 | wire `login` into the real interactive boot; §120's amendment of 2026-08-26 grants the QEMU-only virtio-rng stopgap, and the piece is `BUILT` (2026-08-27) |
| 5 | 169 | | `kilo`: the raw-keystroke input primitive nife's terminal layer does not have today, and the editor itself |
| 6 | 142 | | grow the terminal past its current 18x8 test-harness size to something a person would actually sit at: increments 1-2 only (grow the scanout to 924x344, building the already-`DECIDED` §102; then the 132x43 grid, scrollback, UTF-8 and arrow keys). **Built** (`milestone/142-terminal-size`), retargeted 2026-08-27 from an intermediate 182x90 grid at a 1280x720 scanout that reasoned about a *future* cell size instead of the 7x8 one actually shipping. **Complete, with no decision blocking either increment.** Milestone 142's remaining increments (font atlas, blending, rich attributes, palette) are a typography quality axis this journey's own bar does not require and are not tracked here. |

Steps 3 through 6 are independent of each other; none is a prerequisite for either of the others
(traced directly: `kilo`'s raw-input primitive sits at the `DECISIONS §21` line-discipline contract
level, which both the plain UART console and `display_terminal` already speak identically, so step 5
does not wait on step 3; step 4's blocker was a device grant chain unrelated to either; step 6 grows
the VT engine's own grid, provable under the same test harness milestone 29 already uses, and needs
neither the real-boot wiring step 3 does nor anything else here).

## What "done" looks like

Every step above at `BUILT`. At that point the story in the title is something a person can
actually do, not something proven in six separate test harnesses, and step 6 is what makes "a
terminal worth using" literally true rather than an 18x8 grid technically satisfying step 3 alone
(calef, 2026-08-27: "I want a full size usable terminal").
