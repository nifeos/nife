# QEMU

## What it is

QEMU is a computer made of software. It simulates a whole machine: a CPU that fetches
and executes real instructions one at a time, some RAM, and a set of devices.

We need it because the kernel is not a program that runs on macOS. It has no OS
underneath it, because it *is* the OS. It expects to be the first thing running on a
machine, to own all of RAM, and to talk to hardware by writing values to specific
physical memory addresses. There is no `main()` for macOS to call, no `malloc`, no files.
You cannot type `./kernel` and have anything sensible happen.

So the kernel needs a computer. QEMU is that computer. It loads the kernel binary into
simulated memory, points the simulated CPU at the first instruction, and lets go. From
the kernel's point of view it has woken up on bare metal and is alone in the universe.
It cannot tell the difference.

## Why not develop on real hardware

We do, now: radon (a VisionFive 2, riscv64) boots nife from a card, and xenon (a Dell
OptiPlex, x86_64) is the x86 target (milestones 16a and 87; see [target-hardware.md](target-hardware.md)).
QEMU is still the default rather than the fallback. Compare the loops:

| | Real hardware | QEMU |
|---|---|---|
| Iteration | Build, copy to SD card, move card, power-cycle, watch a serial cable | `cargo run`, ~1 second |
| Debugger | Needs special JTAG hardware | Built in (GDB stub) |
| Automated tests | Basically impossible | It's just a process with an exit code |
| Cost of a bug | Reflash the card | Press up-arrow |

## The `virt` machine

`-M virt` tells QEMU which computer to pretend to be. It can imitate many real boards,
including a Raspberry Pi. But `virt` is a machine that does not physically exist. The
QEMU developers invented it as a deliberately clean, well-documented, standards-following
ARM board.

Real hardware is full of quirks, undocumented registers, and errata. `virt` has a serial
port at exactly `0x0900_0000`, a standard ARM interrupt controller, and virtio devices,
all at fixed documented addresses. Nothing is weird. It is the machine you would design
if your goal was for a person to be able to learn on it.

That is exactly why we start here and treat the Pi as a later port. The Pi will teach us
what real hardware is like. `virt` lets us learn what a kernel is first, without the two
lessons tangled together.

## The serial port

The kernel's first output device is a serial port: ancient, and beautifully dumb. Write a
byte to a magic memory address and that byte goes out a wire, one bit at a time. No
graphics, no fonts, no buffering. It is the simplest way a computer can say anything at
all, which is why it is the first thing every kernel learns to do.

QEMU wires the simulated serial port straight to your terminal. When the kernel writes a
byte to `0x0900_0000`, a character appears in your shell.

## Flags we use, and why

| Flag | Meaning |
|---|---|
| `-M virt` | Be the fictional clean ARM board (above) |
| `-cpu cortex-a72` | Pretend to be this specific ARM core (the one in a Pi 4) |
| `-nographic` | No emulated display window. Wire the serial port to this terminal |
| `-semihosting` | Let the guest ask the host to do things, e.g. "exit with code 0". This is how our test harness reports pass/fail |
| `-kernel <file>` | Load this ELF and jump to its entry point |
| `-s -S` | Open a GDB stub on port 1234 and freeze the CPU until a debugger attaches |

## Emulation vs. virtualization

QEMU *emulates* by default: it reads each guest instruction and simulates its effect in
software. Slower, but it can pretend to be hardware you don't own, and it can stop the
world for a debugger.

A VM product (Parallels, VMware) *virtualizes*: guest instructions run natively on the
real CPU with hardware assist. Much faster, but the guest must match the host
architecture. QEMU can do this too (via KVM on Linux, HVF on macOS), but we don't need
the speed and we do want the debuggability.

## Three ways QEMU will burn your laptop

All three bit us for real. The first two cost 729% of CPU over one day of development. The
third cost milestone 127's EL2 lane time twice in one session, and caught another lane again on
2026-09-02, which is what minted milestone 226.

### 1. An idle kernel is not idle

`arch::halt()` is `loop { wfi }`. It was `loop { wfe }`, and the difference is enormous:

| Instruction | Waits for | What QEMU does | Host CPU |
|---|---|---|---|
| `wfe` | an **event** (an `sev` from another core, a lock release) | treats it as a hint and keeps executing the loop | **99.7%** |
| `wfi` | an **interrupt** | halts the vCPU; **the host thread sleeps** | **0.0%** |

A halted kernel using `wfe` pins a host core forever. Use `wfi` for idling. It is also the
semantically correct instruction: we are not waiting for a sibling core to signal us, we are
idling until something interrupts us.

### 2. QEMU swallows SIGALRM

The obvious way to bound a run on macOS (which has no `timeout(1)`) is:

```sh
perl -e 'alarm 10; exec @ARGV' qemu-system-aarch64 ...     # DOES NOT WORK
```

**QEMU installs its own `SIGALRM` handler** (it uses timers internally), so the alarm is
swallowed and the process runs forever. Every "bounded" run leaks a QEMU.

QEMU *does* honor SIGTERM. Use `helpers/qemu-bounded.sh <seconds> <cmd...>`, which
starts a detached killer that survives a pipeline whose reader (`head`) exits early. That
last part matters: `qemu ... | head -20` leaves QEMU alive, because `head` closing the pipe
does not kill a process that has stopped writing.

### 3. The bound is not the only way a run ends

Found by milestone 127's EL2 lane, twice in one session, and fixed by milestone 226.

The killer above used to have exactly one reason to fire, which was the bound expiring. That
bounds a run which is allowed to finish. It does nothing for a run whose wrapper is killed:
a dead session, a `pkill -f` at a harness, a closed terminal. The wrapper goes, QEMU is
inherited by pid 1, and it runs forever.

The bill arrives on a later run, in a different shape, which is what made it expensive. The
orphan holds the write lock on whatever disk image it was given, and the next boot fails with

```
qemu-system-aarch64: -device virtio-blk-device,drive=d0: Failed to get "write" lock
Is another process using the image [target/nifefs-blank.img]?
```

That names a file. Nothing in it points at a process, so it reads as a bug in whatever the run was
testing. A lane that has read AGENTS.md's warning about leaked emulators can still lose an hour to
it because the symptom does not look like that warning.

`helpers/qemu-bounded.sh` now gives the killer two more reasons to fire. It polls its parent
once a second and kills the child as soon as the wrapper is gone, which covers a SIGKILLed
wrapper and a dead session, neither of which runs a trap anywhere. And it traps SIGTERM and
SIGHUP, so the one process that knows the child's pid does not take that knowledge with it
when `pkill -f` sweeps the wrapper and the killer together.

What is left, and it is not fixable on macOS: a SIGKILL to the killer itself. SIGKILL is not
trappable and macOS has no `prctl(PR_SET_PDEATHSIG)`, so a supervisor shot in the head cannot
hand off. For that case the script does the other thing instead of preventing it. When the command
fails, it runs `lsof` over the image paths in the command line. It prints the pid, ppid, start time
and command of whoever holds one for writing. A reader is not reported, and that filter is what
keeps the message quiet on a green run: the three test legs share one read-only OVMF firmware file.
**Walk the parent chain up before killing what it
names**: a QEMU whose parent is a live harness is somebody's gate in flight, not a leak.

`helpers/qemu-bounded-selftest.sh` checks all of this against a real emulator, including that
`perl`'s alarm is still swallowed. It is in no gate; run it if you touch the bounding script.

**It signals the child's whole tree, because a runner's child is not always QEMU.** The aarch64
and riscv64 runners end in `exec qemu-system-...`, so their child *is* QEMU. The x86_64 runner
deliberately does not `exec` (its own comment: it has to turn `isa-debug-exit`'s odd status back
into 0 afterwards), so its child is a shell with QEMU beneath it, and `cargo xtask shell` is cargo,
then xtask, then the runner. Signaling `$CHILD` alone kills the shell or cargo on time and leaves
QEMU re-parented to launchd, halted and holding a core's worth of memory. So
`helpers/qemu-bounded.sh` collects `$CHILD`'s descendants with `pgrep -P` before signaling anything,
because a killed parent re-parents its children and they cannot be found afterwards, and it sends
TERM, then KILL, to all of them. `helpers/qemu-bounded-selftest.sh` case 9 holds that. The x86_64
runner has not been rerun against it, because this Mac has no x86 emulator lane handy; it is the
same shape as case 9. After bounding an x86_64 run, `pgrep -l qemu-system-x86` and walk the parent
chain, and treat a PPID of 1 as yours.

### 4. Killing a harness does not kill its children

The evidence for the rule in [qemu-hygiene](skills/qemu-hygiene/SKILL.md) that a session checks for
leaked emulators afterwards.

Checking `pgrep` is not sufficient after you kill a harness, and on 2026-08-02 it took four
attempts to notice. Killing a loop script does not kill its descendants: `pkill -f hunt-...` left
`cargo xtask test` running, which kept starting fresh QEMUs. So every check honestly reported "no
qemu" and the next command found one holding `target/nifefs.img`, which then failed unrelated test
runs with `Failed to get "write" lock` and looked like a bug in the code under test.

**And the check runs in both directions** (2026-08-15): before killing a "leaked" QEMU, walk
`ps -o pid,ppid` UP from it too. A QEMU whose parent chain ends in a live harness is somebody's gate
in flight, not a leak. The maintainer killed a lane's mid-suite emulator this way and the lane's run
failed for a reason no one could see from inside it.

Two habits fix it. **Ask who holds the file, not whether a process matches a name**:
`lsof target/nifefs.img` names the holder even when your pattern does not. And **kill the tree at its
root**: walk `ps -o pid,ppid,command` up to the harness and kill that, or the loop simply starts
another child.

**A kernel does not exit.** That is the root of it: `cargo test` terminates because the test
build asks the host to exit via [semihosting](semihosting.md), but a normal boot halts
forever, by design, exactly like real hardware. So every interactive run must be bounded, and
`pgrep -x qemu-system-aarch64` is worth checking after a session.

---

*Add to this file as new QEMU concepts come up.*

## BUGS

- On macOS the pinned version is not available, and building it by hand has a trap. `.qemu-version`
  pins the version CI and Linux build (`script/ci-qemu`); Homebrew ships only its current release
  and `script/ci-qemu` refuses to run on macOS, so a Mac runs whatever Homebrew has and
  `script/qemu-check` warns. Milestone 117's sixth stranger run built the pinned 11.0.2 by hand and
  found that `script/ci-qemu`'s configure line does not link on this SDK: `hw/display/apple-gfx.m`
  needs `--disable-cocoa --disable-pvg`. On 2026-10-10 11.1.1 was built here that way for a
  same-tree comparison, into a scratch prefix, and configure also needed `--python=` pointed at
  Homebrew's `python3` (its `libexec/bin`). Apple's 3.9 cannot install QEMU's own Python
  package editable and configure stops in `mkvenv`. And a QEMU installed into `$HOME/.cache/nife-qemu` is honored by
  `helpers/qemu-path.sh` on macOS too, for every checkout on the account, which is how a build meant
  for one clone changes the emulator under every other lane.
- `cargo xtask uefi-boot`'s screen read is a handshake, per milestone 445 (the screen check stops
  sampling and starts asking). The check reads the guest's framebuffer through QEMU's monitor to
  assert the claim of milestone 243 (a machine with no serial port has no way to say anything, and
  no gate can read it). Sampling (poll every 50 ms and hope a dump lands while the tour is up) is
  not enough. On a busy machine a full `script/test` run read zero rows and reported *"the tour was
  never readable on the screen"*. The same leg run alone a minute later read 56. The message sent
  the reader after the loader's `LocateProtocol`, the pixel order, the stride and the
  mapping, none of which was wrong.

  So the kernel is *asked* to stop. `screen-hold` on the boot command line makes
  `kernel::console::yield_screen` announce `boot_ladder::SCREEN_HELD` on the serial line and wait up
  to ten seconds for a byte back before it clears the screen. So the dump is taken against a state
  that is not going anywhere. Three runs at a one-minute load average of 17, 32 and 38 on an
  eight-core Mac each read exactly 95 rows. Two things carry beyond it:

  - **The tour is taller than the screen, so it scrolls**, which is why the marker is the tour's
    *tail*. Held still, the OVMF console's first row is the middle of the firmware memory map.
  - A spinning guest starves the emulator's own monitor. A wait that polls the UART with
    `spin_loop` makes every `screendump` taken during the hold come back a file that will not
    decode, and parking the core with `wait_for_interrupt` between polls fixes it. Remember it for
    anything else in this tree that busy-waits inside a guest while a host is trying to talk to
    QEMU.
- A process that leaves the tree escapes the bound. The killer finds what to signal by walking
  `pgrep -P` down from its child at the moment it fires. A descendant that double-forks or calls
  `setsid` to reparent itself to pid 1 before then is no longer under the child, and is not
  signaled. Nothing in this tree's QEMU paths does that today. A process group would catch it,
  but a background job in its own group cannot read the terminal, and interactive `cargo xtask
  shell` runs need exactly that.
- **A kernel's serial log is binary to `grep`.** The test logs carry the guest's control bytes, so
  `grep FAILED log` says `Binary file log matches` or nothing, rather than the line. Use `grep -a`.

- **An aarch64 guest's SeqCst store followed by a SeqCst load can reorder under TCG.** Armv8 orders
  an `STLR` before a later `LDAR`, and Rust's aarch64 SeqCst relies on it. QEMU 11.1.2 translates
  `STLR` as a barrier then the store, and `LDAR` as the load then a barrier, so nothing sits between
  the two (`target/arm/tcg/translate-a64.c`, `trans_STLR` and `trans_LDAR`). Store buffering with
  SeqCst on both accesses was seen on an aarch64 host and on an x86_64 one; a `fence(SeqCst)` between
  them holds. A correct Dekker-style handshake can therefore fail in QEMU and never on silicon.
  riscv64 guests are unaffected. Measured in
  [host-reordering-under-tcg](benchmarks/host-reordering-under-tcg.md); not yet reported upstream.
