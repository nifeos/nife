# The second FLUSH that never returned (resolved 2026-09-19)

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.*

An appendix to [notes/framebuffer-contract.md](../framebuffer-contract.md), moved there on
2026-10-11 (UTC) under §212 (a prose budget). It is the record of milestone 177 (wire the graphical
terminal stack into the real interactive boot)'s hang, carried unexplained
for three weeks, and the boot evidence that resolved it.

RESOLVED 2026-09-19 (milestone 177): the second `FLUSH` on the real boot never returned because
nobody received the driver's `FLUSHED` report. Found 2026-08-27 by milestone 177's boot-wiring lane
and carried here, unexplained, for three weeks. The hypothesis recorded at the time (the driver stuck
waiting on its own completion interrupt) was wrong, and so was the reading of the data point below.

What the boot did: `kernel::user::boot_graphical_terminal` received `UP` from the driver and `TERM_UP`
from the terminal and stopped. But the terminal paints its blank grid, which is a first `FLUSH`,
*before* it sends `TERM_UP`, so by then the driver had already replied to that flush and was blocked
sending `FLUSHED`. The terminal's second `FLUSH` (the banner) queued on the display endpoint behind a
driver that would never `RECEIVE` again.

The evidence, read from the boot rather than inferred. A kernel thread dumped every thread twenty
seconds into an aarch64 `--features shell` boot with a GPU and a keyboard, with the endpoints named
first (`display_ep=0xa`, `driver_report=0xb`):

```
tid=0x0009 state=Blocked ... wait=0xb/Sender          <- gpu_driver, in SEND on its report endpoint
tid=0x000a state=Blocked ... wait=0xa/Reply           <- display_terminal, CALLing the driver
ep=0x000a senders=1 receivers=0 pending=0
ep=0x000b senders=1 receivers=0 pending=0
```

The same thread then received on `0xb` and got `[0xd150002, <digest>, 0x4d9a0, ..]`, which is
`FLUSHED` with the surface's pixel count. A second dump ten seconds later showed the driver back
in `RECEIVE` on `0xa` and the terminal in `RECEIVE` on its own endpoint. The queued flush had been
served.
No interrupt, fence or used-ring index was involved.

The keyboard had the identical hang one step later. `keyboard_driver` sends `KEYBOARD_UP` before
its first `WAIT`, and the boot discarded the report endpoint `keyboard_service::start_direct`
returned, so with the display fixed the prompt appeared and a key pressed with `sendkey` still never
echoed. `start_direct` now takes that report itself and no longer returns the endpoint, so no caller
can drop it again. The serial source (`input`) sends no report, which is why
`--graphical-serial` went green with the `FLUSHED` fix alone.

The data point from the second driver, reread (milestone 400, 2026-09-19): `framebuffer_driver`
serves the same contract to the same `display_terminal` over the real boot's wiring and every flush
returns. That was read as pointing at `gpu_driver`'s interrupt handling. The real difference is that
`framebuffer_driver` sends `UP` and nothing after it: it has no `FLUSHED` witness to block on.
