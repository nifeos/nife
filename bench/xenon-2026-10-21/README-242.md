# xenon, 2026-10-21 (UTC): milestone 242's keyboard, without the monitor

The bench step of milestone 242 (USB host and HID, because on commodity hardware the keyboard is
not a UART), on xenon. A USB keyboard (`c0f4:08f5`, low speed) was plugged directly into xenon's
first rear port; the boot stick was in the front port. Each image was booted from the stick with
F12 at POST. The console was read by a raw reader that sends nothing and cleaned with
`briefs/bench-session.md` step 6's `tr`. Nothing was typed on the UART during these boots: every
`echo hi` below was typed by calef on the USB keyboard.

| file | UTC | image | what it shows |
|---|---|---|---|
| `242-screen-panic.log` | ~23:00 | `main` at `5ee9b4afb`, monitor attached (HDMI) | `usb : no keyboard yet (0 of 26 ports in use)`, then the hand-over panics: `the framebuffer driver did not come up (0xdead000000000002)` (pull request #2528) |
| `242-xhci-probe-before.log` | ~23:47 | pull request #2529's probe on the driver as on `main`, no monitor | the same `0 of 26` line; the probe shows the firmware left ports 1, 4 and 9 connected and the scan ran before the ports re-detected after the controller reset. `echo hi` on the keyboard echoed and ran: the late attach works |
| `242-xhci-probe-settle.log` | ~23:51 | #2529's probe with its 100 ms settle before the scan, no monitor | `usb : a keyboard on port 4 (low speed, c0f4:08f5); its keys reach the shell`, and `echo hi` on the keyboard echoed and ran |

The second and third images are the probe builds of #2529 (debug, `--features
xhci_port_probe`), not `main`. Their SHA-256s, checked on the stick before each boot: before
`d4f09c39070d47574fca57b3337e30137d86337b6b1c2beadc589b1b229b0974`, settle
`60602b3733e7a6af9b42953b0a81ad68a6acfa6fe555395e4ad8afea14371fc5`.

What this shows against the block's steps: step 2's success line, from the settle build; step 3's
`hello`-style echo, on the UART log only. Step 3's criterion is the monitor, and the monitor boot
waits on #2528. Steps 4 (replug) and 5 (serial cable pulled) did not run.
