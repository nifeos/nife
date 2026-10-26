# The host-load line: the harness says whether the host was loaded, 2026-08-18

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Moved here on 2026-09-24 and tightened; pull request #1211's first
commit has the text verbatim.)*

## The harness now says whether the host was loaded (2026-08-18)

The main page's diagnostic sorts the family, and the thing that failed cannot apply that diagnostic.
A guest knows it was late. It cannot know that eleven other emulators were on the same eight cores.
Until this was built, the reader had to think of `uptime` unprompted, and the run that asked for it
did not. The third stranger run of milestone 117 (the stranger test) spent about an hour on a defect
that was not there. Other lanes held the laptop at a one-minute load average of 45 to 63, 2 of its
13 aarch64 legs were red, and nothing in any transcript said so.

So `xtask` samples the load and prints it, and only when a leg goes red. `HostLoad` in
`xtask/src/scanout.rs` reads `uptime` every five seconds for the length of an emulated leg. It runs
from the poll loop the scanout referee already runs, and both kernel legs report on failure:

```
host load (aarch64): 1-minute average 35.69 / 36.24 / 36.79 (min/mean/peak over 2 samples), on 8 cores
  4.6x oversubscribed at the peak. A timing assertion that failed above may be measuring this
  machine rather than this kernel; `script/icount` asserts the timer claims in instructions, which
  nothing the host does can move. See notes/load-sensitive-assertions.md.
```

Four things about it are deliberate.

- It suggests, and says so. A loaded host does not make a failure spurious, and a quiet one does
  not make it real; what the line removes is the guessing.
- It reports min, mean and peak, which is `script/repeat-under-load`'s vocabulary on purpose. The
  peak earns its keep: a suite runs for minutes, and the one-minute average has decayed by the time
  the verdict is in.
- It gives the core count beside the figure. A load average without one is a number the reader
  must look something up to use.
- It prints only on a red leg, since readers learn to skip a diagnostic that appears on every run.

The parse is `script/repeat-under-load`'s `load_now` in Rust, with a host test pinning both formats.
Development is macOS (`load averages: 4.14 4.86 4.29`) and CI is `ubuntu-24.04-arm`
(`load average: 0.50, 0.40, 0.30`). The shape not in front of the author is the one that would
break silently, reporting "unavailable" on every CI run.

## BUGS

- It samples only while a leg is running. A host that thrashed during the build and went quiet for
  the boot produces an honest, unhelpful line.
- It says nothing about which assertion failed. The TCG leg's transcript goes straight through to
  the terminal and `xtask` never sees it, so the line lands after the leg rather than beside the
  panic.
- A load average is a coarse instrument in the first place. It counts runnable threads, not
  contention for the one core QEMU happens to be on.
