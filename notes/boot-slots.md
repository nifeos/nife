# Two boot slots, so a bad upgrade cannot brick the machine

Milestone 198 (a package manager, and the trivial install that makes a second customer possible),
rung 2b, built 2026-09-21 on calef's ruling the same day: *"Yes, write the tries and priority
attributes in 2b."* [Installing nife onto a disk](installing.md) is rung 2a and this builds
directly on it; read it first if you have not.

The promise, and it is the whole page in one sentence: an installed nife machine that is handed a
boot image which does not come up goes back to the previous one by itself, with nobody at the
console.

## Running it

```console
$ cargo xtask rollback-boot
--- the stick installs itself onto an empty NVMe disk ---
  install     : DONE. Remove the installation medium and reboot.

rollback-boot: slot 1 written from target/esp-install/EFI/BOOT/BOOTX64.EFI, on trial
               priority 3, 1 try; slot 0 stays at priority 2, confirmed

--- boot 2 of 3: the machine tries the upgrade, and the upgrade never comes up ---
uefi_loader: boot slots on one disk, 2 of them
uefi_loader: starting boot slot 1

rollback-boot: killed at the handoff, which is what a hang looks like to the disk

--- boot 3 of 3: nobody touched anything ---
uefi_loader: boot slots on one disk, 2 of them
uefi_loader: starting boot slot 0
uefi_loader: started from boot slot 0
$ wc made-on-target
  1 10 57
rollback-boot: slot 1 is priority 3, 0 tries, not successful
rollback-boot: PASS
```

And the disk it leaves behind can be read by anybody's tool, because the layout is the ordinary one:

```console
$ sgdisk -p target/nife-install.img
Number  Start (sector)    End (sector)  Size       Code  Name
   1            2048          784383   381.9 MiB   EC5C  nife data
   2          784384          915455   64.0 MiB    1163  nife slot 0
   3          915456         1046527   64.0 MiB    1163  nife slot 1
   4         1046528         2097118   512.0 MiB   EF00  nife boot
```

## What is on the disk

| | |
|---|---|
| nife data | a RedoxFS volume, first on the disk for [rung 2a's reason](installing.md) |
| nife slot 0, nife slot 1 | one boot image each, raw, behind a 4096-byte header |
| nife boot | a FAT32 EFI system partition holding `\EFI\BOOT\BOOTX64.EFI`, which is **the chooser** |

There are therefore three copies of the boot image on a freshly installed disk, and the third
one is not waste. The chooser is what the firmware starts, and it is a complete nife image, so a
machine whose slots are both unbootable still has something known-good to fall back to. That falls
out of the chooser and the image being the same binary and is the answer to "what if everything is
broken".

## The state, and why it lives where it does

Three fields per slot, in the **GPT partition entry's attribute bits**:

| bits | field | |
|---|---|---|
| 48-51 | priority | 0 to 15. **0 means never boot this slot** |
| 52-55 | tries | boot attempts left before the slot is given up on |
| 56 | successful | this image has booted and been confirmed |

Those are ChromeOS's own positions, so `cgpt show` reads a nife disk, and the design is ChromeOS's
whole. What is nife's is the answer to where the selector runs, and it is the correction this rung
turned on.

**A maintainer told calef that firmware would do the selecting. That is wrong here.** UEFI 2.11
section 5.3.3 says of attribute bits 48 to 63, verbatim:

> Reserved for GUID specific use. The use of these bits will vary depending on the
> `PartitionTypeGUID`. Only the owner of the `PartitionTypeGUID` is allowed to modify these bits.
> They must be preserved if Bits 0-47 are modified.

So the specification tells generic firmware to stay out of those bits, and generic firmware does.
ChromeOS gets firmware-level selection because **depthcharge is a coreboot payload that replaces
UEFI**: its firmware is a program Google wrote. We have UEFI, so the first code of ours that runs is
the selector, and the first code of ours that runs is `\EFI\BOOT\BOOTX64.EFI`.

That cuts both ways and both halves matter. The bits are the right place for the state, because
they are ours by specification and no other operating system's partition tool will touch them, which
a file in a filesystem cannot claim. And the selector has to be ours, which is why this rung has
a chooser in it at all.

For that to be true the slots must be partitions of a type that is ours, which is why `NIFE_BOOT`
(`1163 1EE3-E18F-4AFC-9D7F-171572635629`) exists beside §45 (a nife partition is `EC5CC08B-D749-4434-AC38-A274C50385BA`, and that never changes)'s `NIFE_DATA`. On an EFI system
partition those bits would be Microsoft's.

### The design that was evaluated and refused

UEFI `Boot####` variables with `BootOrder` and `BootNext`, which firmware genuinely does read and
which is how a Linux distribution would do this. Refused for a reason specific to this tree: rung
2a deliberately proved its boot with the firmware variable store deleted, so that "the firmware
found the file on its own" was a claim about the disk and not about leftovers in a checkout. A
rollback design that needs those variables to survive contradicts the one property that boot was
built to demonstrate. It would also put the state somewhere only firmware runtime services can
reach, which this kernel does not map.

## The crux: who decrements, and when

**The chooser, before it hands off.** Not the running system afterwards, and this is the single
decision the feature stands on.

A chooser that only read the state, leaving the booted system to mark itself good once it was up,
protects against an image that crashes visibly and not against one that hangs. A machine
that wedges before userspace would retry the same bad image on every power cycle, forever, showing
nothing to the console nobody is standing at. That is the failure that actually strands people, and
the only thing that bounds it is a decrement already on the disk before control leaves.

It costs a disk write on every trial boot, which is a real cost and is where the crash-consistency
gap below comes from. ChromeOS's firmware decrements before the launch for the same reason.

A confirmed slot spends nothing, so an ordinary boot of a settled machine writes nothing at all.

## Which failures this catches, and which it does not

The table is in `uefi_loader/src/chooser.rs` beside the code and is repeated here because it is what
somebody deciding whether to trust this needs:

| failure | caught | by what |
|---|---|---|
| the slot was never written, or its header is corrupt | yes, same boot | the header's own CRC |
| the copy into the slot died partway | yes, same boot | the image CRC in the header |
| the image is not a PE the firmware will start | yes, same boot | `LoadImage` refuses, next slot |
| the image starts and returns an error | yes, same boot | `StartImage` returns, next slot |
| **the image starts and hangs** | yes, **next** boot | the try spent before the handoff |
| the image comes up but is subtly wrong | **no** | nothing here can tell |
| power lost during the chooser's own table write | **no** | four block ranges, no journal |
| every slot is bad | the machine still boots | the chooser's own image is started |

A failure the chooser can see for itself is recovered from **within one boot**: it moves to the next
slot rather than making somebody reboot to recover from something it watched happen. A failure it
cannot see is what tries are for.

## What was proved, and by what

| claim | checked by | strength |
|---|---|---|
| the bits encode and decode as the format says | `cargo test -p boot_slot`, 15 tests | weakest: a writer against its own reader |
| the bit positions are the ones `cgpt` prints | the ChromiumOS disk-format reference, read 2026-09-21 | somebody else's document, not memory |
| an install writes four partitions with slot 0 confirmed | `cargo xtask install-boot`, plus the image read back by hand | strong: the bytes on a disk |
| an installed machine boots **through** the chooser | `cargo xtask install-boot`'s two new assertions | strong |
| **a doomed upgrade is tried and abandoned unattended** | `cargo xtask rollback-boot`, three boots | **the deliverable** |
| the decrement survives a power cycle | boot 3 of that gate never mentions slot 1 | strong: the machine was killed in between |
| any of this works on real firmware | **nothing** | rung 2b's bench half, and calef's hands |

On the fifth row, the part worth being precise about: the machine is killed at the instant of the
handoff, as soon as the chooser says it is starting slot 1. From the disk's point of view a kernel
that wedges, a driver that spins and a power cut are the same event, which is that the try was spent
and nothing came back, so this is the hang rather than a stand-in for it. And the image staged into
slot 1 is a byte-for-byte copy of the working one on purpose: garbage would be caught by the header
checksum inside one boot, which tests the cheap half.

## What marks a trial boot successful

The installer sets the bit on the slot it writes, and that is a record of an observation rather than
an assumption: the bytes going into slot 0 are the bytes the firmware started that machine with
seconds earlier. An *upgrade* is a different claim, and something on the running machine has to make
it, or a perfectly good upgrade rolls back once its tries are spent.

**The chooser tells the booted system which slot it is**, as a word on the kernel's command line
(`boot_slot::cmdline`), and it has to be told rather than work it out: the try is spent *before* the
handoff, so a slot started on its last try is no longer `is_bootable` and `select` now names the
other one. The one boot a confirmation exists for is the one the inference gets backwards.

The criterion is the filesystem server mounting the installed disk and reporting ready, with the
progenitor built and measured and about to run. That is everything between power-on and the last
thing this machine can check without a person, and the bias is deliberately late: confirming early
would keep an upgrade that comes up and cannot do its job, which is the failure the whole feature
exists to prevent.

**What it can still be wrong about**, which is the honest half: anything the boot path does not
touch (the network stack, the compositor, a driver nothing opens at boot), the shell, which is one
row stronger than this reaches, and anything that fails after the first few seconds. It is a
statement about coming up, not about running.

**The write is `system_installer`'s `ROLE_CONFIRM`**, which is `ROLE_SURVEY` with a write: the same 34
blocks, the same parser, one attribute word put back. It holds no entropy endpoint, so it cannot
draw the unique ids a new table carries and the only table it can write is the one it read; and no
boot file, so it cannot rewrite the image it is vouching for.

What keeps it from colliding with the filesystem server is an ordering, not a mechanism, and
that is worth knowing before you move the call. One block server has one transfer region shared by
every holder of its endpoint. `install_service::confirm` runs between the filesystem server's ready
report and the progenitor's first instruction, when that server is blocked in receive with no client
that could wake it. The day something spawns a second disk client before the progenitor runs, the
ordering stops holding silently. See
[milestone 573 (two programs share one disk's transfer region, and only an ordering keeps them
apart)](../design/roadmap/0573-two-programs-share-one-disks-transfer-region.md).

`cargo xtask confirm-boot` is `rollback-boot`'s exact negative: a good upgrade with one try is
tried, confirms itself, and is still chosen on boot 3 with no tries left. The single try is the
strength of the test rather than its cost: a slot with tries to spare would be chosen by priority
alone and the gate would pass on a machine whose confirmation did nothing.

## What this does not do

Nothing writes the second slot on a running machine, because there is no upgrader. The gate
stages one from the host, through the same crates the installer uses, so the format is exercised end
to end and the program that will one day do it is not.

Two disks carrying boot slots is a refusal. The chooser enumerates whole disks and gives up if
more than one has slots. Telling which disk this image was started from means walking its own
device path to the `HARDDRIVE` node, which is the same parser `place_boot_file` has wanted since
rung 2a.

**x86_64 only**, which is rung 2a's scope unchanged: without the loader handing the kernel its own
file there is nothing to install and nothing to put in a slot, and a device-tree handoff has one
initrd slot in `/chosen` and no second one. `crates/boot_slot` itself is architecture-free and
host-tested.

## One thing this found that was already broken

Every `Spawn::maps` page has cost a share of a revocation log page since `map_physical` began
recording on 2026-09-21. Four call sites were updated to pay for it; the installer's
eleven-megabyte boot-file window could not be, because it goes through `user::run` and `run` had
nowhere to put the number, and it fitted inside `AS_OVERHEAD`'s slack by luck. This rung grew the
boot image by about a megabyte and the luck ran out, as an `OutOfPageFrames` panic three stack
frames from anything that mentions memory. `user::load` now takes the number and `run_with` derives
it from `spawn.maps`, so no caller has to remember it: the same move as any other accounting that
was a comment and is now a computation.

Charging the full window would put a frame into every long-lived process's region that the region
never uses, sixty-eight of them, to pay for a window one caller has, and the frame ledger fails
that. What a caller owes is the cost above what `AS_OVERHEAD`'s margin was already providing, which is
`WINDOW_IN_OVERHEAD` and its reasoning at the arithmetic.

## See also

- [Installing nife onto a disk](installing.md): rung 2a, the disk this all sits on, and why the data
  partition comes first.
- [The boot stick, and the program that makes it](boot-stick.md): rung 1, how the file gets onto a
  stick at all.
- [Booting x86_64 from real firmware](x86-uefi-boot.md): the loader the chooser is half of.
- [The GUID partition table](globally-unique-identifier-partition-table.md): the format the
  attribute bits live in, and what the fixtures pinned.
