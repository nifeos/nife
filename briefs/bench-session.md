Run a bench session on a real board (radon, xenon or argon) from a lane, safely, while calef is at
the bench. You are a developer lane. The maintainer relays everything between you and calef. Do the
work; stop only where this brief says stop.

*(Name provisional. calef names things.)*

Why this brief exists. The board notes are written for a maintainer beside the board with calef. A
lane cannot flip a switch or type at U-Boot, and it cannot see the desk. This brief is the list of
things a lane would otherwise have to guess, with the guesses turned into answers.

## Ready to run: take the top entry for the board calef names

calef cannot schedule bench time ahead. He says "radon is ready" (or xenon, or argon) whenever he
happens to be at the desk. **A lane told "X is ready" takes the top entry below for that board with
no further briefing**, claims its milestone, and follows this brief and the linked procedure.
Whoever finishes an entry moves it off this list in the same pull request.

| rank | board | milestone | procedure | calef's hands |
|---|---|---|---|---|
| 1 | radon | milestone 168 (a multi-tasking workload benchmark), fatal risk 4 | `notes/job-mix.md`, "The next bench evening on radon, start to finish" | **one power cycle of radon's outlet per boot, at least five boots**, until the reset fix below lands |
| 2 | xenon | 261, the NVMe driver leaves the kernel, and then the soak on xenon (fatal risks 6, then 5) | `notes/risk-6-bench-evening.md` (pull request #1275); if that has not landed, `design/roadmap/0261-el0-nvme-on-xenon.md`. The first thing the boot reads is xenon's DMAR table, which decides whether the earlier x86 confinement results stand | about 30 minutes at xenon: the one-time firmware **Data Wipe** of the internal NVMe (261's "What calef has to do"), which cannot be undone; a monitor and a USB keyboard (xenon halts at POST without one); the FAT32 stick, staged by the lane with `cargo xtask disk-throughput --stage-only`, copying `target/esp-disk-throughput/EFI/BOOT/BOOTX64.EFI` and **not** `target/esp`; a phone for photos of the screen; optionally a Linux live stick for the `fio` comparison |
| 3 | argon | awaiting the board (2026-10-06: the seller shipped a TK1, which is going back; `notes/bench-runbook.md`). Then 127, first light, then 225's soak on argon | `design/roadmap/0127-the-sel4-machine.md` and `notes/bench-runbook.md`, "argon, and why it is last" | everything: argon has never booted nife, so cabling, media and power are all his |

**The first step of each session is a watched reset, because a board that can reset itself turns
every later boot from calef's hands into a command.**

- radon. Today it cannot. On 2026-09-04 an SBI SRST cold reboot stopped at `cannot read pmic
  power register` and never came back (milestone 249 (the boot lottery is sampled by a person
  walking to the board), "The bench answered it, 2026-09-04"). Pull request #1279 re-read that log:
  no reset happened; OpenSBI's reboot is an I2C write to the PMIC, and it hangs because U-Boot gated
  that bus's clocks before the handoff. Nothing on patagonia can reach radon's outlet (milestone 224
  (nothing can power-cycle radon, so a hung soak needs a person)). Once the lane re-enabling
  those I2C clocks before the reset has landed, the first step on radon is one reset, watched
  through to a second `U-Boot SPL` banner, `payload came from net`, and `soak-test: started`. If it
  works, milestone 168's boots need no plug cycles, and the calef's-hands cell above shrinks to the
  first power-on. Until it lands, do not try it: the outcome is known, and it costs a power cycle.
- xenon. The first step is one kernel-initiated reset, the `script/soak-test --reboot` path (pull request #1279, not yet on `main` when this was written),
  watched to a full boot. The firmware is set to halt at POST on warnings, so a reset that lands on
  a warning waits for a keypress: say so rather than calling it a hang. While calef is at the
  machine, ask him to press Ctrl-P at POST and record whether Intel AMT/MEBx appears
  (`design/roadmap/0653-xenon-may-carry-amt.md`); AMT would be remote power.
- argon, once the TX1 arrives (2026-10-06). First light comes first. The step after it is a PSCI system reset over `smc`, watched
  the same way.

The radon soak (milestone 225) is not on this list because it ran on 2026-09-25. Another radon soak
buys a second draw of the placement lottery, which is worth something, but entry 1 answers a fatal
risk that has no data yet.

## The one rule: ask before you change hardware state, and never guess at hardware

Send the maintainer a one-line message naming the action, and wait for the go, before any of these:

- Power: any request to switch the board's own outlet. **Never any other outlet, under any
  circumstances, and never a USB hub: drives that must not lose power or their connection share
  the desk.** The maintainer reads which outlet is which from the lab record kept off-tree, and
  names it in the go;
- Media: writing a card, a stick, or a disk, including `script/board-image --card`;
- The network boot server: starting or stopping `script/board-netboot`, or changing what it
  serves;
- The serial port: opening it, after checking nobody holds it (`lsof /dev/cu.usbmodem*`). Two
  readers split the byte stream silently.

Reading an open log and building on the host need no go.

**Anything physical is calef's**: a cable, a button, a DIP switch, a card in or out, typing at a
U-Boot prompt. Say what you need in one sentence and keep polling while you wait. Do not infer the
state of the desk. Ask.

## Step 1: claim, then build on the host

    script/claim milestone/<N>-<slug> --new-work    # --debt-paydown if the milestone repays debt

Build the image from the worktree, never from the main checkout: the maintainer may have deleted
the main checkout's `target/` that day.

    script/board-image --soak --tftp        # or --job-mix, --bench: the milestone's own flag

For xenon the image is a single UEFI file instead:

    cargo xtask uefi-image --features soak_test   # then copy target/esp/EFI/BOOT/BOOTX64.EFI
                                                  # to \EFI\BOOT\BOOTX64.EFI on a FAT32 stick

`script/board-image` must print `SEALED` and exit 0. `NOT SEALED` is a real failure on every build,
including `--soak`, `--job-mix` and `--bench`: the pair would halt at `MEASURED BOOT REFUSED`. Stop
and report it. Until milestone 563 (a seal check that reads bytes cannot see a check that was
dropped) merged on 2026-09-26, those three builds skipped measured boot and the warning was a
false alarm. That is why older notes and bench logs say to ignore it.

## Step 2: rehearse under QEMU, with the same tree

    script/soak-test --arch riscv64 --for 45s      # radon's architecture

This is the baseline a red on the board gets compared against. A defect only counts as silicon-only
if the same tree does not reproduce it here (fatal risk 5). In a fresh worktree with no
`target/nifefs.img`, this command builds the disk image first.

## Step 3: serve, watch, then ask for power

With the maintainer's go:

    lsof -i UDP:69; lsof /dev/cu.usbmodem*                  # both must be free
    script/board-netboot                                      # from the worktree, backgrounded
    script/board-console --port /dev/cu.usbmodem<id> --board radon \
        --for <duration> --until none --log <worktree>/target/bench/radon-<what>-<UTC stamp>.log

Start both with `nohup ... &` so they outlive the lane itself, not only one tool call. On
2026-09-25 the lane was stopped twice by usage limits during an 8-hour soak, and the detached
watcher logged straight through both. Write the log under your own worktree's `target/`: not the
main checkout's (the maintainer may delete it), not a session scratch directory (it goes when the
session does), and not `bench/` (the log needs cleaning first, step 6). Start the console before power, so the boot is captured. An empty log
at this point means the board is off or halted, which is what you want before a power cycle.

Then ask: "power-cycle radon now". calef does it from the Kasa app.

## Step 4: read the first minute before calef leaves

In order, and report each to the maintainer:

1. `nife: payload came from net`. If it says `card`, stop: the board is running an older image.
   The `tftp server is` line above it names the address the card expected. If that address moved,
   the fix is typed at the U-Boot prompt, which is calef's hands.
2. The workload's start line (for the soak: `4 groups ... (24 user threads) on 4 online core(s)`).
3. For a soak: **`wakerate` about `100 * harts`** (about 400 on radon), and `crossings` rising
   between beats. After the first census settles (one drift event in the first beat or two, then
   `drifted=0`), note crossings per second. A slow draw (about 0.5/s on radon, seen 2026-09-03)
   is a different experiment from a fast one (about 50 to 186/s). Report it and let the maintainer
   put a redraw to calef.

Only then tell the maintainer calef can walk away.

## Step 5: poll until it ends, and never end your turn while it runs

Block in one call at a time, up to ten minutes each. A loop that returns early when the console
exits or when a beat shows a nonzero `refused`, `mismatch` or `stalled` does the job. **Ending the
turn to "wait for a notification" kills the lane.**

`script/board-console` exit statuses: `0` beat for the whole watch, `1` a failure announced, `2`
went quiet (three missed beats: a hang, which needs calef for a power cycle), `3` ended early.

## Step 6: capture, clean, record

Read `t=`, never the beat count, as the duration: radon's beat is about 5.05 s, not 5, because each
beat's 5 s is counted from the end of the previous beat's sampling and printing (`watch` in
`kernel/src/soak.rs`).

    LC_ALL=C tr -cd '\11\12\15\40-\176' < <scratch log> > bench/<board>-<UTC date>/<what>.log

The console writes bytes that are not UTF-8 under sustained output, and `grep` then silently
reports nothing. Read raw logs with `LC_ALL=C grep -a`.

Record the run in the milestone's own block and its note, and add a row to
`notes/multicore-defect-curve.md`'s exposure table if the workload was a soak. A red first gets the
same image under QEMU (step 2) before anyone writes "silicon-only".

## What to do about the failures that are known, and stop on anything else

| You see | Do |
|---|---|
| `NOT SEALED` on any build | stop and report: it is real since milestone 563 merged, step 1 |
| `soak-test: no heartbeat was seen` in 0.2 s under QEMU | a missing `nifefs.img` on a tree older than 2026-09-25; rebase |
| `payload came from card` | stop, report the `tftp server is` line |
| console exit 2 (quiet) | report the last 50 lines; calef power-cycles only after the log is saved |
| the board needs a reboot | radon cannot reboot itself; that is a power cycle, which is calef's |
| anything else | stop, report, keep the console running |

## BUGS

- Every radon power cycle is a person. Milestone 224 records calef's choice to stay manual, and
  249 records that the self-reboot route hangs in firmware. A hung soak therefore holds the board
  until calef is next at the desk.
- The plug state is invisible to a lane. Nothing reports whether radon's outlet is on, so an empty
  log cannot tell a board that is off from one that died before its UART came up.
- The TFTP address is read off patagonia at card-write time. The card written on 2026-09-16
  asks for the address patagonia's USB ethernet adapter had then. Its Wi-Fi address has moved
  since, and the boot survived only because the adapter's address did not move.
