---
status: BUILT
raised: 2026-09-21
built: 2026-09-21
---
# 554. A good upgrade sticks: what marks a trial boot successful

*(Renumbered from 543 by the integrator on 2026-09-22: the maintainer minted 543 for a promoted proposal while this lane was running, which is the collision AGENTS.md predicts for anything global to the tree. Number provisional until the merge queue lands it; 542 is the highest claimed
on a branch in flight.)* Built 2026-09-21 by the lane `abboot/confirm-a-trial-boot`, on calef's
launch of the proposal `design/roadmap/proposals/nothing-marks-a-trial-boot-successful.md` the same
day. **Based on the rung 2b branch `abboot/tries-and-priority`, which is not on `main` yet**, so its
milestone block is named here in prose rather than cited.

**This block answers that proposal and does not retire it**, which is deliberate rather than an
oversight. Retiring it means editing milestone 525's block, whose `**Proposed.**` follow-on row and
`BUGS` both name the proposal file and are both now stale. That block belongs to a lane still
working. For the integrator at merge: delete
`design/roadmap/proposals/nothing-marks-a-trial-boot-successful.md`, and change milestone 525 (a bad upgrade cannot brick the machine: two boot slots, tries and priority)'s two rows to
point here. `script/roadmap --check` fails on the deletion until the second half is done.

This is the other half of rung 2b of milestone 198 (a package manager, and the trivial install that
makes a second customer possible). The rollback half built the chooser that spends tries and falls
back. This built the thing that says a boot worked.

## The problem, stated so a stranger can check it

`crates/boot_slot` has a `successful` bit and a `State::confirmed`. The installer sets the bit on
the slot it writes, and nothing on a running machine ever set it again. So an upgrade written
into the spare slot, which boots perfectly and runs for a week, was abandoned the moment its tries
ran out. The machine quietly returned to the older image.

It failed safe, which is why the rollback rung shipped without it. It also meant **upgrades did not
stick**, which is not a system anybody would run: it fails days after the upgrade and looks like
something else.

## What was built

| | |
|---|---|
| `boot_slot::cmdline` | the token that carries the slot number from the chooser into the running system, and the three shapes it was weighed against |
| `uefi_loader::handoff::cmdline` | writes it, beside the screen, on **every** machine rather than only one with a framebuffer |
| `arch::x86_64::machine` and `memory::boot_slot` | one reader for the boot command line, and the fact kept for the rest of the boot |
| `installer`'s `ROLE_CONFIRM` | reads the table, sets the bit on one entry, writes both copies back. No entropy endpoint, no boot file |
| `user::install_service::confirm` | the criterion, the endowment, and the ordering that makes the write safe |
| `cargo xtask confirm-boot` | three boots that watch a good upgrade be tried, confirm itself, and still be chosen with no tries left |

## The decision: what counts as "this boot worked"

A promise rather than a mechanism, and the reason it deserves a section is that both directions of
error are real. Confirm too early and a machine happily keeps an upgrade that comes up and
cannot do its job. That is the failure the whole feature exists to prevent. Confirm too late
and nothing ever confirms on a machine that boots to a prompt and waits for a person. That is what
this system is today, so every upgrade reverts. The bias has to be late.

The criterion: the filesystem server mounted the installed disk and reported ready, and the
progenitor is built and measured and about to run. That covers everything between power-on and
the last thing this machine can check without a person. It covers the kernel and its self-tests,
userspace, the NVMe controller and its ring-3 server, the block server, the filesystem, and the
boot program's measurement against the trust root.

The proposal recommended one row stronger, *filesystem mounted and the shell reachable*, and the
shell half is not reached. That is not a cost saving and is not claimed as one: it is the ordering
below. The confirming program needs the disk, and the filesystem server holds the disk. The one
window in which both are true and nothing is concurrent is between the mount and the progenitor's
first instruction. Reaching a prompt is past it.

The Boot Loader Specification's counting leaves this call to the operating system, and ChromeOS's
update engine sets the bit from userspace. Both do so for the same reason: the bootloader cannot
know and the system can.

### What a boot that satisfies it can still be wrong about

The honest half, and it is repeated in `crates/boot_slot`'s `BUGS` where a reader meets the feature:

- Anything nothing exercises at boot: the network stack, the compositor, a driver for a device
  the boot path does not touch. An upgrade that breaks only those is confirmed and kept.
- The shell, for the reason above. The filesystem is mounted and the progenitor is built, and
  nothing has typed anything. A regression between there and a prompt survives.
- Anything that fails after the first few seconds: a leak, a wedge under load, a filesystem that
  mounts and then corrupts. This is a statement about coming up, not about running.

## The authority to write a live partition table

`ROLE_CONFIRM` is spawned with four capabilities and nothing else: a report endpoint, the disk's
`blk` endpoint, the page it shares with that disk's server, and a budget for the one page it maps
itself.

No entropy endpoint, which is what makes it narrow rather than merely small. A partition table
carries unique ids, so a process that cannot draw one cannot write a table anything would read back
as new. The only table it can write is the table it read with the nine bits `boot_slot` owns
changed. No boot file, so it cannot rewrite the image it is vouching for. No console, no
filesystem, no network.

The gap, named rather than papered over: a `blk` endpoint is the whole disk, because nothing in
`filesystem_protocol::blk` bounds a client to a block range. So this process *could* write anywhere
on that disk. It is the same limitation `installer`'s own `BUGS` records for the filesystem server,
and it is one wire field away from fixed. It is why this is a boot-path call rather than an
authority any program can ask for.

## What serializes the write, and the answer is an ordering

**Nothing enforces it, and that is stated as the exception it is.** One NVMe server has one transfer
region, shared by every client of its endpoint, and a client stages bytes into that region before it
calls. Two clients staging at once would corrupt each other; no lock, lease or range check prevents
it.

What makes this write safe is where it is: after the filesystem server's ready report and before the
progenitor's first instruction. In that window the filesystem server is blocked in receive with no
client that could wake it. The kernel boot path is single-threaded and is the only thing that can
hand its endpoint to anybody.

That is rung three of `AGENTS.md`'s ladder where a higher rung exists, and **it is a foot gun**. The
day something spawns a second disk client before the progenitor runs, or this call moves after it,
the ordering silently stops holding. The symptom is a corrupted partition table on somebody's
installed machine. The mechanism that would make it hold is a transfer region per client, or a `blk`
endpoint bounded to a block range; both are wire decisions.

## The write itself

Idempotent twice over. A slot already successful is returned without the disk being written at
all. An installed machine boots its confirmed slot every day of its life. A write on every one of
those boots is a write that can be interrupted on every one of them. And setting a bit
that is set writes the same bytes, so the cheap check is an optimization rather than the property.

Safe to interrupt. The bits that change are nine, inside one `u64`, inside one 128-byte entry,
inside one 512-byte logical block. So the entry on the disk is always either the old one or the new
one and never a third thing. The four-write sequence that puts the array and the two headers back
goes backup first. At every instant at least one complete self-consistent copy of the table
is on the disk. The worst outcome of an interrupted confirmation is a slot that is not confirmed.
That is the state the machine was in a moment earlier, and it rolls back safely.

## The gate

`cargo xtask confirm-boot`, the exact negative of `rollback-boot`. Install, and stage a good upgrade
into slot 1 with one try. Boot 2 chooses it, spends the try, comes up, and confirms. Boot 3
chooses it again, on a slot with no tries left.

Boot 3 is the whole assertion, and the single try is what makes it one rather than a convenience. A
slot with tries left would be chosen by priority alone. The gate would then pass on a machine whose
confirmation did nothing at all. `State::bootable` refuses a slot with no tries unless the successful
bit is set. So a machine that chooses it has read a bit something on boot 2 wrote, across two
separate QEMU processes with nothing between them but the disk.

## BUGS

- `boot-slot=` is a value two programs agree on and is committed to. The chooser spends a try
  *before* handing off, so inferring the slot from the table afterwards names the wrong one on
  exactly the trial boot a confirmation exists for; `boot_slot`'s docs say so.
- **The criterion does not reach a shell**, which is the row the proposal recommended. See above:
  it is the disk-sharing ordering and not an effort argument.
- Nothing serializes the confirming write against the filesystem server except where it sits.
  See above.
- The interrupted-confirmation case is argued and untested. Nothing in this tree cuts power to a
  QEMU between two block writes, which is `rollback-boot`'s own caveat for the decrement.
- The chooser's own table write still goes primary-first, so it has a window in which neither
  copy parses. The confirming write does not. Making the two agree is the chooser's lane.
- Nothing reads the backup table when the primary fails to parse. The backup-first write order
  above buys a recovery that no reader in this tree performs yet; a foreign tool can do it.
- **The upgrade is staged by the gate and not by nife**, because nothing in this tree upgrades a
  running machine. The format is exercised end to end; the program that will do it is not.
- x86_64 only (DECISIONS §19 (architectural parity is a tenet; the targets are aarch64, riscv64, and x86_64)), because the
  chooser is, and the device-tree architectures have no installed disk to choose slots on. A scope
  note rather than a portability claim; the parameter the number would arrive on is already on
  every architecture's `hand_over`.

## Follow-on

- **Milestone 573.** The sharing is milestone 573 (two programs share one disk's transfer region,
  and only an ordering keeps them apart),
  `design/roadmap/0573-two-programs-share-one-disks-transfer-region.md`: a `blk`
  endpoint bounded to a block range, or a transfer region per client. Either turns the ordering
  above from a written record into a mechanism, and the bounded endpoint is the same wire question
  `installer`'s `BUGS` already names for the filesystem server. Neither is a lane's to decide.
- **Recorded.** The chooser's write order, in `uefi_loader/src/chooser.rs`'s `BUGS`: four ranges
  primary-first, where backup-first would cost nothing and leave one good copy at every instant.
- **Recorded.** A reader that falls back to the backup table, in the same `BUGS`. Without it the
  backup-first order buys recovery only for tools that are not ours.
- **Milestone 198.** The upgrader: the program that writes the spare slot and puts it on trial. It
  was held on this block and is no longer held on it.

## Index row

A running machine can now say that its boot worked, so an upgrade that comes up is kept instead of
being abandoned when its tries run out. The chooser writes the slot number onto the kernel's command
line, because a running system cannot work out which slot started it. (The try is spent before the
handoff, so the slot that booted is exactly the one `select` no longer names.) A confined
process holding the disk and no source of randomness sets one attribute bit once the filesystem
server has mounted that disk. What that criterion can still be wrong about is written next to it.
`cargo xtask confirm-boot` is `rollback-boot`'s exact negative: a good upgrade is tried, confirms
itself, and is still chosen on the next boot with no tries left.
