# The console fixtures, and which of them a board actually printed

Two directories, and the split is the point: **`captured/` is bytes off the wire, `synthetic/` is
text written by hand.** The distinction was a paragraph in this file for the first half of a day
and is now a directory, because a fixture that looks like a capture and is not one is the shape of
the fabricated block quote this tree carried for twelve days, and a claim in a README is rung four
where a path is rung three.

## `captured/`

**Raw bytes from the VisionFive 2, 115200 8N1, on 2026-09-01**, taken by calef with the board on
his desk. CRLF and control bytes are preserved exactly as they arrived, including the NUL run just
before our banner and the `\b` backspaces U-Boot's autoboot countdown overwrites itself with.
Nothing has been cleaned up; cleaning it would remove the parts a recogniser has to survive.

- **`vf2-2026-09-01-userspace.log`** is the **full success**: the measured-boot gate passes, `init`
  loads `worker` from the archive and builds it as a child, the child's IPC round-trips, and a
  userspace driver comes up holding an `Irq` cap. This is what a working board looks like.
- **`vf2-2026-09-01-manual-boot.log`** is a successful boot with **no archive on the card**, so the
  tour runs to its end and `init` never builds anything. Kept beside the one above because the pair
  is the only thing that shows what `userspace_ran` distinguishes.
- **`vf2-2026-09-01-measured-boot-refused.log`** is the kernel **halting at the trust boundary**,
  because the card's kernel and archive came from different builds:

  ```
  MEASURED BOOT REFUSED: 'init' is not what this kernel image was built against
    expected sha256 d63054330191625f54db33110bdf3882d6f81bc5c97877f32f574c2e8cc798b1
    measured sha256 dfd6a05381ab613cfc6cbcdb007ed4c67316e60229347f87e8b05be7643dbc11
  halting rather than handing the archive to init.
  ```

  **This is the gate working, not a crash**, and it is the subtlest fixture here. It contains
  `Starting kernel ...`, the whole nife banner, and most of a tour before it refuses, so anything
  keying on the banner calls it a success. It is why the watcher has a settle window.
- **`vf2-2026-09-01-extlinux-refused.log`** is the **extlinux** path from power-on, and it fails:

  ```
  Moving Image from 0x40200000 to 0x80200000, end=802ff000
  Device tree not found or missing FDT support
  ### ERROR ### Please RESET the board ###
  ```

  That is exactly the caveat `notes/visionfive2.md` records about U-Boot's fallback DTB addresses.
  It is the third outcome, and the reason the recogniser has one: U-Boot refusing before the kernel
  ever ran looks nothing like a hang and must not be reported as one.

**And three captures that are not off a board at all**, kept in this directory anyway because the
distinction this directory draws is *machine-printed against hand-written*, not *silicon against
emulator*. All three are the soak command's output on the RISC-V `virt` machine, and all three are
unedited:

- **`qemu-2026-09-01-riscv64-soak.log`** is milestone 219's workload announcing itself and beating,
  from before the placement census existed. Its beats are what `progress::observe_soak_beat` is
  asserted against. Taken on the development Mac with `script/soak`, the command's name at the time.
- **`qemu-2026-09-03-riscv64-soak-census.log`** is the same thing with milestone 240's census in it,
  taken by milestone 249's lane, also with `script/soak`. Two of the four cores hold three grinders
  between them and the settled arrangement has **one** clean core, at 18,963 round trips a second,
  which is the low end of the same spread radon shows.
- **`qemu-2026-09-14-riscv64-soak-test.log`** is `script/soak-test --arch riscv64 --for 30s` on the
  day milestone 297 renamed the command, taken in the lane's own Linux worktree. It is post-221, so
  it is the only one of the three that carries `wakes=` and a census block that changes between the
  spawn lottery and the settled arrangement.
- **`qemu-2026-10-05-aarch64-soak-test.log`** is `script/soak-test --for 25s --log <file>` on
  patagonia, aarch64 `virt` with four cores, captured by the lane for milestone 225 (run the soak
  on radon, argon and xenon) because argon is
  aarch64 and no aarch64 soak capture existed. It is what `exposure`'s tests read, and the reason
  that reader splits boots at the banner: on aarch64 the `smp: 4 core(s) online` line comes
  twenty lines *before* `nife machine:`, so a split at the machine line loses the core count.

**And one capture of the job-mix sweep** (milestone 324 part 2), also machine-printed and also not
off a board:

- **`qemu-2026-09-19-aarch64-job-mix-medians.log`** is a `--features job_mix` kernel on the aarch64
  `virt` machine with four cores, taken with `helpers/qemu-bounded.sh` around
  `helpers/qemu-runner-aarch64.sh` so that the capture is the guest's console and nothing else. It
  runs the whole sweep: six points, 126 subruns, seven `job-mix-kind:` lines under each point, and
  `job-mix: done`. It is what turned the sweep's markers from constants somebody wrote into text a
  kernel printed, and it is where the 4.0-second longest-subrun figure in `script/job-mix`'s `BUGS`
  comes from (249,234,771 ticks on a 62.5 MHz counter).

  **It replaced a capture taken the same day, and the replacement is the point** (2026-09-19).
  `qemu-2026-09-19-aarch64-job-mix.log` was taken from a kernel from before milestone 168 changed
  the sweep's statistic, and the recogniser's parser had been written against it. When 168 landed,
  the kernel's point line stopped carrying `ticks=` and `jpm=` and the parser read nothing, while
  the fixture and the parser went on agreeing with each other. **Two things made from the same
  stale source do not check each other**, and that is the argument for re-capturing rather than
  editing: this file is bytes off a current kernel, so the next rename breaks a test instead of
  going quiet. The old capture is deleted rather than kept beside it, because nothing reads it and
  a second transcript of a different mix invites a comparison that is not valid.

  **The wedge case has no fixture of its own and does not need one.** The test that proves a
  stalled sweep is distinguishable truncates *this* file after its third point, which is the same
  construction `synthetic/vf2-handoff-hang.log` uses one directory over and is honest for the same
  reason: every byte before the cut is a byte a machine printed. The end-to-end proof is separate
  and was run rather than written: `cargo xtask job-mix --quiet-after 1s` exits 2 and
  `--for 30s` exits 3. Both were re-run against this capture's kernel; the window that used to
  suffice (`--for 12s`) was against a sweep that finished in 28 seconds and this one takes 165.

  **No sweep has been watched on a board.** radon has never run one this tool read, which is
  milestone 168's own HARDWARE gate.

**The first two predate the marker rename and are left exactly as they arrived** (milestone 297,
2026-09-14: `soak:` became `soak-test:` and `soak-census:` became `soak-test-census:` when
`script/soak` became `script/soak-test`). A capture is a record, so the respelling happens at read
time in `board_console::respell_pre_297_markers` and never on disk; that function's block says why
the parser does not simply match both spellings, and why the third capture above is what makes that
affordable. Rewriting the marker inside a `captured/` file would be the fabricated transcript
`design/naming/rename-where-names-hide.md` records, and this directory exists to make that
impossible to do by accident.

**Both board captures show a degraded U-Boot environment on this card, and that is not a defect in
our payload.** `*** Warning - bad CRC, using default environment`, then several
`** Invalid partition 3 **` / `Couldn't find partition mmc 1:3` / `Can't set block device`
complaints, and `## Error: "boot2" not defined`, before it finds `mmc 1:1` and gets on with it. The
board boots through all of it. Nobody should read those lines as something we caused, and whether
the environment is worth repairing is somebody else's milestone.

**What the captures settled.** Every marker in the recogniser had been quoted from documentation
and none from a machine, and this is where that got checked. `U-Boot SPL`, `OpenSBI v`, the U-Boot
banner with a version where `SPL` would be, `StarFive #`, `Starting kernel ...`, `Moving Image
from` and `nife on ` were all correct as written. Two things were missing and are now in: the
`### ERROR ### Please RESET the board ###` refusal, and `nife: the capability core runs on `, which
is a stronger success signal than the banner because it means the whole boot tour ran.

## `synthetic/`

**Written by hand from documented markers**, because no capture of either case exists. Each line is
either a marker quoted from `notes/visionfive2.md`'s failure-triage ladder, a line quoted from this
tree's own source (`notes/trusted-init.md` for the measured-boot refusal), or filler in the shape
vendor firmware prints so that a chunk boundary lands somewhere other than on a marker. No test
asserts on the filler.

- **`vf2-bad-magic.log`**: U-Boot rejecting the payload's header (triage ladder row three).
- **`vf2-handoff-hang.log`**: the kernel starting, saying a few lines, and stopping before the tour.
  **This is the one outcome with no real sample and the one the tool exists for**, since a hang is
  what a multicore defect looks like from the far end of a serial cable. Constructed from the real
  captures' own opening lines, truncated. If risk 5 ever produces one at a bench, capture it: a
  real hang is worth more than every other fixture here.

These prove the recogniser finds the markers **it was told about**, in a stream that behaves like a
stream. They prove nothing about whether those markers are the text the board prints. If either
case ever occurs at the bench, capture it and move the file.
