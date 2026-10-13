---
status: BUILT
raised: 2026-09-30
built: 2026-10-03
milestone_dependencies: 177, 192, 600
decision_dependencies: 26, 227
machine_requirements: none
specific_machine: none
needs_person: no
---
# 632. Graphics on demand: `graphical_terminal`, launched from the swish prompt

*(Title provisional.)* calef, 2026-09-30:
*"I don't want graphics at boot and won't for a long time. Change to launching a program from the
swish prompt to get graphics; graphics is going to sit there largely unused for some time."*

The ruling reverses the boot half of milestone 177 (wire the graphical terminal stack into the real
interactive boot). Every `--features shell` boot is the minimal UART system of DECISIONS §26
(the fault endpoint: thread death becomes a message a supervisor holds). A gpu on the bus changes
only who holds the grants. The answer is the shell, until a person at the prompt delegates them
into a session. Milestone 600's mechanism survives whole; only its moment moved, from the boot to
the launch.

## What changed

- The protocol carries devices (`crates/grant_plan/src/spawnproto.rs`). `GRAPHICS_BIT` and
  `KEYBOARD_BIT` announce the gpu's four caps and the keyboard's three. They are delegated after
  the `--mem` untyped and before the machine statistics page. `SHELL_GPU_SLOT` (22) names the
  seven-slot block 22-28 in the shell. Const assertions fence it against `SHELL_CONFIG_SLOT` below
  and `RUN_UNVOUCHED_SLOT` above. The shell probes fixed numbers, and a probe is sound only for a
  slot nothing else allocates into.
- `graphical_terminal` is a program (`components/src/graphical_terminal.rs`, `Prog::GraphicalTerminal`, id 20). Its manifest declares
  nothing a line designates, so every operator on it is a refusal and a session is always a plain
  line. It has two arms, chosen by `x0`. With a keyboard, the session's own discipline answers
  cooked `OP_READLINE` and does the terminal's whole job. Without one, the program reads the boot's
  own discipline raw over the UART (`OP_RAWMODE`/`OP_READRAW`) and paints its own echo. That is the
  shell's shape under §227 (how Tab reaches the shell: the shell edits its own line), and it is
  milestone 192 (a keyboard on real silicon)'s option A at launch rather than at boot. `quit` and
  `^C` end either arm.
- The boot hands the devices to the shell and builds nothing from them
  (`crates/system_initializer`). `graphical_terminal_grants` replaces `graphical_verdict`. The console pair and
  the UART `input` driver are built unconditionally again, the pre-177 shape, and the UART is never
  freed early. The seven grants land in the shell's slots 22-28 beside the machine page, after the
  login block's peak. The progenitor keeps no copy.
- The shell refuses what it cannot launch, loudly. `graphical_terminal` at a prompt that holds no display (no
  gpu, a `login` session, every witness wiring) prints a sentence. It is never a spawn that quietly
  drops the authority the user asked for. On a launch the shell delegates the seven caps, narrowed
  to the rights the boot endowment carried. It keeps its own copies, so a session can run again.
- The launch belongs to the spawn service (`build_graphical_terminal_session`). Milestone 600 (the graphical
  terminal stack is built in userspace)'s builder lives in the `Prog::GraphicalTerminal` branch.
  It refuses anything but the exact shape: the program and the caps together, and the
  keyboard bit exactly when the boot's verdict says. Everything is carved from one 528-page job
  region. The session program is born supervised on `deaths` like any job, so its one reap sweeps
  the drivers with it. The drivers are unsupervised for `build_caretaker`'s recorded reason: a death
  message for a tid the sweep already collected would trap `job_undertaker`. Failure is `Err` all
  the way down, not a trap. A launch that cannot come up is a command that failed.

## What proves it

`script/swish-check`, on aarch64 and riscv64 (see the fold line below). Two of each leg's boots
attach a gpu. Each asserts the swish prompt on the serial console first, which is the claim that the
boot stayed minimal, then types `graphical_terminal` over the UART. It requires the session's `$ `
prompt on the screen and one keystroke echoed back. The keystroke goes by `sendkey` through the
session's keyboard in one boot and by the raw UART round trip in the other. The
first boot of every leg, on all three ISAs, attaches no gpu and types `graphical_terminal` too,
requiring the refusal sentence.

**The graphical legs are part of `swish-check`, not a job of their own** (calef's ruling,
2026-10-03 UTC: a job of its own cost a runner and about ten minutes of build per pull request for
about a minute of boots). A launch ends the boot it is typed into, so each aarch64 and riscv64 leg is three boots.
The first is the full script with no gpu, which keeps the refusal covered on all three ISAs at no
extra boot. The second runs the after-reboot script, then attaches a gpu with no keyboard (the UART
arm, the boards' configuration). The third is scriptless, with a gpu and a keyboard (the `sendkey`
arm). Both arms stay because they test different drivers and cost about thirteen seconds a boot. The
`--graphical` and `--graphical-serial` flags refuse themselves.

## BUGS

- `GRAPHICAL_TERMINAL_SESSION_PAGES` is 528, not the 464 the constants add up to, because a launch
  fails at 464. Bisected on the keyboard arm, a
  session needs 490 to 493 pages on aarch64 and 494 to 495 on riscv64. While a session runs, its
  region is 528 of the pool's 672 pages, so a `std` job and a second session are refused until it
  ends. The serial arm builds one child fewer and was not bisected separately.
- The 528-page figure was bisected on aarch64 and riscv64 only. There is no x86_64 graphical leg
  (milestone 270 (wire `virtio-gpu-pci` and `virtio-input` into the x86_64 test runner) is what it
  waits on; see the x86_64 bullet below), so x86_64's need is unmeasured and nothing gates it. When
  that leg arrives, it must re-bisect this constant.
- A session whose build fails after its first driver started may keep those pages. The failure
  path reclaims the region and the sweep wakes the half-built drivers. No leg has yet forced
  whether they exit on a swept endpoint.
- A session cannot be interrupted under §24 (interrupting the foreground process). It ends on its
  own, so a second `^C` cannot escalate and a hung session holds its region until reboot.
- The boot carries the seven device grants from kernel spawn until the shell's build. With a gpu, a keyboard, a virtio-rng and a NIC attached,
  that resting baseline is counted from the code, not measured. No gate boots all four devices.
- Measured 2026-10-03 on aarch64 and riscv64: a boot with a gpu and a keyboard peaks at 26
  capability slots and a boot with no gpu at 24. The plain legs' slot gauge reads the launch: the
  keyboard arm's launch peaks at **30** on both ISAs, with or without the NIC, and the serial arm,
  with the plain legs' NIC and virtio-rng, at 27. `CAPABILITY_TABLE_PEAK_MEASURED` in
  `kernel/src/cap.rs` holds the tree's record. Which grants sit on the peak is not traced.
- x86_64 has no `graphical_terminal` session. No virtio-gpu is wired there, the verdict answers empty, and
  `graphical_terminal` is refused with the sentence, which the plain swish-check legs assert on all three ISAs.
  The plan for the launch itself is milestone 270 (wire `virtio-gpu-pci` and `virtio-input` into the
  x86_64 test runner): the drivers are PCIe and need no new capability, so a graphical leg follows.
  **What an x86_64 graphical leg needs, read from the tree 2026-10-03 (UTC), not built.**
  (1) Runners: `helpers/qemu-uefi-x86_64.sh` and `helpers/qemu-runner-x86_64.sh` read no `NIFE_GPU`,
  `NIFE_KEYBOARD` or `NIFE_GPU_MON` and attach no `virtio-gpu-pci`. That is milestone 270 and
  nothing else. (2) The kernel side is shared. `boot_progenitor` is the one `x86_hand_over` and the
  other ISAs call, and it wires the gpu through `display_service::wire_device` and
  `pci::find_gpu_device`, which enumerate PCIe through ECAM with nothing ISA-specific. The gpu driver
  behind `intel-iommu` has never run on x86_64, and the four display tests that skip there say so.
  (3) Keystrokes: the UART arm needs no new driver. The session reads the boot's line discipline
  raw. On x86_64 that is already fed by the progenitor's `input` over a COM1 `PortRange` capability
  (milestones 299 and 505), which is how swish-check's x86_64 leg types today. The keyboard arm would be `virtio-input` over PCIe, the
  same arch-neutral driver, behind the same runner flag. (4) Screen: the OVMF runner emulates VGA
  with `-display none` and the firmware console tee (milestone 400) paints it. A `screendump` must
  name the virtio-gpu's console, as the aarch64 runner does for ramfb. (5) Re-bisect the page count
  above. Nothing here is a design fork. The firmware-screen terminal (milestone 400 (the shell on
  the firmware screen)) is untouched and still mirrors the boot console; it is the x86_64 display
  path, and milestone 624 (the x86_64 paint path stops repainting the world) is its paint path.
  Whether that console should also become a launch is calef's call, not decided here.

## Follow-on

- **Milestone 700.** Milestone 700 (a graphical terminal session can be interrupted and torn down like any job).
  `design/roadmap/0700-a-screen-session-can-be-interrupted-and-torn-down-like-any-job.md`.
  The §24 job-frame shape for `graphical_terminal`, so a second `^C` escalates and a hung session can be torn
  down. The manifest already records the cost: `interruptible: false` is a first cut, not a design.
- **Milestone 699.** Milestone 699 (a local editor for the graphical terminal session's UART arm). `design/roadmap/0699-a-local-editor-for-the-screen-sessions-uart-arm.md`. The §227
  engine behind the raw arm's reads, so backspace works before `quit` is typed.
- **Recorded.** The rest of `SWISH_CHECK_SCRIPT` against a graphical prompt is still scoped-out
  follow-on work, carried unchanged from milestone 177's own scoping.

## Index row

calef's 2026-09-30 ruling reverses milestone 177's boot half. Every boot is the minimal UART system,
and the shell holds the display devices' seven grants at slots 22-28. A new `graphical_terminal` program,
launched from the prompt, takes them by delegation. The progenitor builds the session's driver,
terminal and (with a keyboard) discipline from one supervised region. Launch proven on aarch64 and riscv64
by `script/swish-check`'s aarch64 and riscv64 legs; the refusal, on all three. Without a keyboard, a session reads the boot
discipline raw over the UART: milestone 192's option A at launch.
