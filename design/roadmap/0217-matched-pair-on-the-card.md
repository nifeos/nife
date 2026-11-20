---
status: BUILT
raised: 2026-09-01
built: 2026-09-02
---
# 217. The card carries a kernel and an archive from different builds, and the gate is the only thing that noticed

Built.

In brief. The VisionFive 2 boots, starts a user program at U-mode, and then halts:

```
userspace  : a program ran at U-mode and made 0 system calls (yield/yield/exit via ecall)

MEASURED BOOT REFUSED: 'init' is not what this kernel image was built against
  expected sha256 d63054330191625f54db33110bdf3882d6f81bc5c97877f32f574c2e8cc798b1
  measured sha256 dfd6a05381ab613cfc6cbcdb007ed4c67316e60229347f87e8b05be7643dbc11
halting rather than handing the archive to init.
```

That is the gate working and it should be read as a success, in the same way DECISIONS §14's
verified core is a success when it refuses. The kernel on the card was built against a different
userspace archive than the one on the card, so it declines to hand the archive to `init`. Nothing is
wrong with either file; they are from different builds.

## Why the pair drifted, which is the part worth fixing

`script/board-image` builds a matched pair correctly, and its own comment records the scar that
taught it to. On 2026-08-15 it built kernel-first, produced a kernel vouching for the *previous*
archive, and the board refused the pair at the bench (`MEASURED BOOT REFUSED`, boot 12). The build
order was fixed then.

**What was not fixed is the card-prep instructions the script prints**, and they are how the pair
comes apart afterwards. Step 3 copies two files:

```
cp target/board/nife-vf2.img /Volumes/NIFE/
mkdir -p /Volumes/NIFE/extlinux
cp target/board/extlinux/extlinux.conf /Volumes/NIFE/extlinux/
```

The archive is mentioned only in the build output, as *"the userspace archive; optional third
file on the card"*. **It is not optional and calling it optional is the defect.** A person who
follows the printed steps exactly copies a new kernel over an old archive, which is precisely the
2026-08-15 failure reintroduced one layer out: the script stopped producing a mismatched pair and
started instructing one.

## What it needs

- The printed card-prep steps copy all three files, with the archive no longer described as
  optional.
- A word about why, at the place the reader meets it. The kernel measures the archive and
  refuses a mismatch, so the two files travel together or the board halts.
- Consider making the mismatch impossible to express rather than documented. The strongest form
  available here is that the script copies the files itself when given a target volume, so a human
  cannot copy one without the other. That is rung one of AGENTS.md's ladder against this block's
  rung three. It is in tension with the script's deliberate refusal to run anything destructive
  ("`dd` to a device is a decision the person at the bench makes"), and that tension is the design
  question this milestone should answer rather than assume. Copying files onto a mounted
  filesystem is not `dd` to a raw device, and the two may not deserve the same caution.

## What it got, 2026-09-02

**The printed steps no longer instruct a mismatch, and the stronger form was available**, so both
halves of "What it needs" landed rather than the documented one alone.

`script/board-image --card /Volumes/NIFE` copies the kernel, the archive and (milestone 218's) boot
script onto a card as one act, after every build step has succeeded, so a failed build cannot leave
a card holding half a pair. Without `--card` the printed steps say to run it rather than listing
three `cp` commands a reader can do two of.

### The design question, answered rather than assumed

This block set the script's deliberate refusal to run anything destructive against rung one of
AGENTS.md's ladder and asked which wins. **Copying wins, and the boundary is not "does it write to
a disk".**

Formatting names a *whole device*. `diskutil eraseDisk /dev/diskN` and `dd` destroy everything on
whatever that name resolves to, the name is easy to get wrong. The person at the bench is the
one looking at it. Copying names a *mounted filesystem the operator already chose and mounted*, and
its worst outcome is three unexpected files somewhere. Those are different acts, and the original
refusal was written about the first one. The header comment now says so explicitly instead of
leaving the distinction to be re-derived.

The argument that decides it is that only the script can make the set indivisible. A printed step
is rung four whatever it says, and this milestone exists because rung four failed. The steps were
correct about the kernel and silent about the archive, and a card came apart. Prose warning a
reader to copy three files is the same rung as the prose that told them to copy two.

The refusals stay narrow rather than clever. `--card` requires an existing directory and declines
`/`; it does not try to prove the path is really a memory card, because a check that guesses at
intent fails the person with an unusual mount and protects nobody else.

### The one deletion, and why it is content-matched

`--card` removes `<card>/extlinux/extlinux.conf`, and only that path, and only when the file
contains a `label nife` line. An extlinux config on this board does not fail, it hangs U-Boot
before the kernel runs (milestone 218), so a stale one left beside a working boot script would
quietly undo that fix, and "somebody will notice" is rung zero. Matching the content first is what
keeps this from being a script that deletes a stranger's boot configuration: an extlinux.conf that
is not one of ours is left alone, which was checked both ways on the host.

### Proven, and where

On the host, against a directory standing in for a card: the three files arrive together, a nife
extlinux config is removed and a Debian-looking one is not, a nonexistent path is refused before
the build rather than after it. What no host check covers is a real card's filesystem, which is
`cp` into FAT32 and is the least surprising thing here.

## BUGS

- **This block does not fix any card.** Re-flashing is a bench action and the pair on the card is
  stale until somebody does it, and radon was powered down on the day this landed.
- `--card` has only ever written to a directory on a Mac's own disk. Nothing here has touched a
  real microSD card, and the `sync` it issues afterwards is the ordinary defense against pulling a
  card too early rather than a tested one.
- Nothing here checks a card after the fact. A tool that reads a mounted volume and reports
  whether its kernel and archive match would catch this before a power cycle rather than after, and
  is not in scope. `--card` narrows who needs it rather than removing the need: a card written by
  any other means is still unverifiable without booting it.
- The measured-boot refusal is only as good as the manifest. This milestone treats the gate as
  correct because it fired on a real mismatch; it makes no claim about what the gate would miss.

## Follow-on

- **Recorded.** `design/roadmap/0217-matched-pair-on-the-card.md` BUGS: this block does not fix any
  card. Re-flashing is a bench action, and radon's pair is stale until somebody does it.
- **Recorded.** `design/roadmap/0217-matched-pair-on-the-card.md` BUGS: the card option has only ever
  written to a directory on a Mac's own disk. Nothing here has touched a real microSD card, and the
  `sync` it issues afterwards is the ordinary defense against pulling a card too early rather than a
  tested one.
- **Recorded.** `design/roadmap/0217-matched-pair-on-the-card.md` BUGS: the measured-boot refusal is
  treated as correct because it fired on a real mismatch, and this milestone makes no claim about
  what that gate would miss.
- **Refused.** Having the card option prove that the path it was given is really a memory card. The
  refusals stay narrow: it requires an existing directory and declines the filesystem root, because
  a check that guesses at intent fails the person with an unusual mount and protects nobody else.
- **Milestone 332.** A tool that reads a mounted card
  and reports whether its kernel and its archive match. `--card` narrows who needs one rather than
  removing the need: a card written by any other means stays unverifiable without booting it, so a
  mismatch is found after a power cycle at the bench instead of before one.

## Index row

`script/board-image --card <dir>` copies the kernel, the archive and the boot script onto a
mounted card as one act, so the pair cannot be separated by following the printed steps, which is
how it came apart. The block's design question was answered rather than assumed. Formatting names
a whole device and stays the operator's, copying names a filesystem they already mounted, and only
the script can make a set indivisible, because a printed instruction is the same rung as the
printed instruction that failed. It also removes a stale nife `extlinux.conf`, matched by content
so a stranger's is left alone, since one on this board hangs U-Boot before the kernel runs (218)
