---
name: qemu-hygiene
description: >-
  QEMU hygiene in nife: load before running QEMU by hand (a demo, a boot, an interactive run) and
  after any session that ran it. Every interactive or demo run is bounded with
  helpers/qemu-bounded.sh, since timeout(1) is absent on macOS and perl alarm does not work on QEMU.
  Covers why halt() uses wfi, and how to find and kill a leaked QEMU without killing someone else's
  gate (pgrep -l qemu, walk the parent chain, lsof target/nifefs.img).
---

# Never leave QEMU running

Moved whole from `AGENTS.md` on 2026-10-06 (UTC) by lane/agents-md-skills, wording unchanged apart
from heading levels and link paths, so where it says "this file" it means `AGENTS.md`. Its name was
ratified by calef on 2026-10-06 (UTC); notes/skills/README.md records it.

## Never leave QEMU running

A nife kernel that has finished its work calls `arch::halt()`, which is `loop { wfi }`. It never
exits, so QEMU never exits either unless something kills it or the kernel asks the host to terminate
via semihosting (which only the test build does). Two consequences:

1. Every interactive or demo QEMU run must be bounded, with `helpers/qemu-bounded.sh <seconds>
   <cmd...>`. `timeout(1)` does not exist on macOS, and `perl -e 'alarm N; exec @ARGV'` does not
   work on QEMU: QEMU installs its own `SIGALRM` handler and swallows the alarm, so the process runs
   forever.
2. `halt()` must use `wfi`, not `wfe`. QEMU implements `wfi` as a real vCPU halt and the host thread
   sleeps; it merely spins on `wfe`, burning 99.7% of a host core. With `wfi` it is 0.0%.

After any session that ran QEMU, check `pgrep -l qemu` and clean up (`-l` rather than `-x
qemu-system-aarch64`, because it matches every architecture's binary). Three rules for that cleanup, and
[`notes/qemu.md`](https://github.com/partsfoundry/record/blob/main/notes/qemu.md) has the four attempts it took to learn them:

- Killing a harness does not kill its children, so a `pgrep` that reports nothing can still be
  followed by a QEMU holding `target/nifefs.img`. Kill the tree at its root: walk `ps -o
  pid,ppid,command` up to the harness and kill that.
- The check runs in both directions. Before killing a "leaked" QEMU, walk `ps -o pid,ppid` UP from
  it: a QEMU whose parent chain ends in a live harness is somebody's gate in flight, not a leak.
- Ask who holds the file, not whether a process matches a name: `lsof target/nifefs.img` names the
  holder even when your pattern does not.
