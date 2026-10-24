---
status: PARTIAL
raised: 2026-09-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: xenon (its firmware settings and the house network it boots from are the work)
needs_person: yes
---
# 260. Boot xenon over the network, because the stick is the tax and the router is ours

calef, 2026-09-05: *"xenon already hurts. We can control our router since it runs OpenWRT."*
Everything that does not need xenon is built, and the firmware settings and the router edit are
done. What remains is one power cycle at the bench and one photograph, calef's because it is his
machine, not because anything is unchosen. Nothing in this block asks calef to choose between
options: the step is written down in `notes/x86-uefi-boot/network-boot-xenon.md`, and CI rehearses
everything up to the real NIC on every pull request.

## Why this is not the same problem radon had, and why the answer just changed

Milestone 257 (boot radon over the network) needed no cooperation from the network at all. U-Boot takes the server address
from a script we write onto the card, so `192.168.8.216` is baked in and the house router never has
to know anything.

PXE is the opposite. The DHCP server has to advertise the next server and the boot filename, and
that is the router. When 257's lane refused `dnsmasq`, it did so for exactly this reason. Its
own words are worth keeping because they are what made this look blocked: *"a second DHCP server on
it is an outage for everyone in the building."*

That objection does not apply here and the difference is the whole reason this is startable.
This is not a second DHCP server; it is one option on the existing one. OpenWRT's DHCP *is*
`dnsmasq`, and its boot options can be scoped to a single MAC address, so xenon gets an offer
and nothing else on the house network sees any change.

## What makes it easier than radon, which is the opposite of what anyone would guess

xenon's payload is one file. `cargo xtask uefi-image` stages the loader, the kernel and the
archive into a single `BOOTX64.EFI`, 9,210,880 bytes as measured on 2026-09-05 (this block said
9,180,160 when it was minted; the number moves with every kernel change and is not a constant). So a
netboot is one TFTP transfer, where radon needs two and a boot script to sequence them. There is no
fallback branch to write, because PXE either produces the file or the firmware moves to the next
boot entry on its own.

And `script/board-netboot` is already board-agnostic. It serves a directory over TFTP and
knows nothing about RISC-V; milestone 257 built it, and the ratified name says netboot rather than a
board or a protocol precisely so it could serve this too. **It served this without a single change
to how it serves radon**, at `--root target/esp`, and the one change it did need is in BUGS below.

## What was built

### The router half, written down and then made to run

`bench/xenon-netboot/dnsmasq.conf`. Four directives, each with its reason beside it: a `dhcp-host`
tagging xenon by MAC, two `dhcp-match` lines tagging an EFI x86-64 client by option 93, and one
`dhcp-boot` requiring both tags.

It is not only a record, and that is the point. `script/netboot-rehearsal` parses this file and
answers DHCP out of it, so a typo in the configuration fails a gate on patagonia rather than at a
bench with a camera in hand. That is the same move the multicast DNS responder's configuration document made (retired
2026-09-15; notes/mdns.md): one document, read by the thing and by the thing's test. A fact that lived only in the router would be the
rung-four failure AGENTS.md names, and a reflashed router would take it with it.

No address is assigned to xenon, deliberately: `set:` with no IP is valid dnsmasq, tags the host
without removing it from the dynamic range, and so cannot collide with a lease the router has
already handed out. Nothing needs to reach xenon at a known address.

### The architecture question, answered rather than sidestepped

DHCP option 93 says what the client can execute. Scoping by MAC alone would sidestep the problem,
and the hole that leaves is worth naming: **xenon itself is a BIOS client whenever `Boot List
Option` is Legacy**, which is one setup visit away. So the offer requires the architecture tag as
well, and a client in the wrong mode gets *no* boot filename and falls through to the next boot
entry, rather than being handed 9 MB of the wrong instruction set.

Measured, `script/netboot-rehearsal --check`, in under a second and with no emulator:

| Client | option 93 | Offered |
|---|---|---|
| xenon in UEFI mode | 7 | `EFI/BOOT/BOOTX64.EFI` from 192.168.8.138 |
| xenon, firmware sending the other EFI x86-64 value | 9 | the same |
| xenon after somebody sets Boot List Option to Legacy | 0 | nothing |
| xenon asking for an HTTP boot | 16 | nothing |
| xenon on firmware too old to send option 93 | absent | nothing |
| any other machine in the house | 7 | nothing |

Only the first row can be exercised under OVMF, because OVMF is a UEFI machine and cannot be asked
to be a BIOS one. That is why the table exists as its own gate.

### The rehearsal, which is the deliverable that mattered

`script/netboot-rehearsal` boots nife the way xenon will, on patagonia, with nothing plugged in.
A synthetic ethernet on QEMU's stream `socket` netdev carries real OVMF to the real
`script/board-netboot`, serving the real `target/esp`, with the DHCP answers coming out of the
config file above. No disk of any kind is attached: the wire is the only way in.

Measured, 2026-09-05 on patagonia: `nife x86_64: boot complete, halting.` in 8.2 seconds
from QEMU start, 9,210,880 bytes transferred, blksize 1468 as EDK2 asks for it, the whole
boot tour on serial. A second run an hour later, after the suite had rebuilt the image, measured
7.9 seconds and 9,797,120 bytes, which is the same claim twice and is also the evidence for
the size not being a constant. The negative case (`--mac 02:00:00:00:00:01`) produces `PXE-E16: No valid offer
received` and no boot, which is what a house machine that is not xenon must see.

CI runs all three (the configuration check, the boot and the refused MAC) against the release image
in the `boot-check` job, as `script/ci-build netboot-rehearsal`. On an x86_64 Linux host on
2026-10-24 the release image (8,180,736 bytes) booted in 14.1 s and the whole row took 1 min 36 s.

`-netdev user` was refused rather than missed. QEMU's slirp has a DHCP server and a TFTP server
built in, and pointing them at `target/esp` PXE-boots this image in one line. That was the first
thing tried and it worked, and it is what proved the payload boots at all. It proves nothing about
our configuration or our server, because neither is in the path, and neither can be put there.
Slirp's NAT has no TFTP helper, and TFTP moves to a fresh server port after the first packet, so the
DATA reply is dropped by the very NAT that was meant to carry it.

### One defect found in `board-netboot`, and what it cost

EDK2 asks for a file twice: once to learn its size, which it does by starting a read, taking `tsize`
out of the option acknowledgement, and sending a TFTP ERROR to stop the transfer it just
started. And once to fetch it. `board-netboot` did not understand ERROR. It retried its
acknowledgement six times, held the client past its own timeout so the real request had to be sent
again, and printed `FAILED` on the one transfer of a boot that was working perfectly.

Handling it took the rehearsal from 23.4s to 8.2s and made the log true. Milestone 257 wrote
down the shape of this: a network boot can fail in a way that looks like success. This is the same
coin's other face, and it would have been read at the bench as a broken server.

### The bench procedure

`notes/x86-uefi-boot.md`, in the shape `notes/visionfive2.md`'s runbook has: the two firmware
settings with their photograph numbers, the router edit in both UCI and raw-dnsmasq form, the two
commands on patagonia. What the screen and the server terminal should each say in order, and a
failure table keyed on what a person with a camera and no shell can actually see.

It removes the co-location, which is the second reason to want it. First light is on record as
happening the way it did because *"patagonia could not be moved to the bench"*. A netboot does not
need it to be: xenon needs a cable to the house LAN and patagonia needs to be on the same LAN, from
wherever it is.

## The two firmware settings, which are calef's

Read off `notes/xenon-firmware.md`, which transcribed 70 photographs of this machine's setup UI on
2026-09-05 so that a lane would not have to walk to it. Both are on one page:

> IMG_4031, Integrated NIC. `Enable UEFI Network Stack` unticked. Disabled ( );
> Enabled (•); Enabled w/PXE ( ). So the LAN is visible to an OS but there is no UEFI PXE path.

So two changes: tick `Enable UEFI Network Stack`, and move Integrated NIC from `Enabled` to
`Enabled w/PXE`. They are calef's for the reason that note already gives about firmware
generally, that a setting changes this machine's behavior for everything else it is used for.

Nothing else in the transcription is in the way. Boot List Option is already UEFI, Secure Boot
is Disabled, legacy option ROMs are unticked, and `UEFI Boot Path Security` has no effect
while no admin password is set, and none is.

## The proof that this milestone worked

xenon boots nife with no removable media in it, photographed, since the Dell's video output is
the channel that carried first light. Anything short of that is a rehearsal, and the rehearsal is
now done: it exists, it is green, and it is what makes the bench session worth an evening.

## What this does not fix, and it is the thing that will bite next

xenon halts at POST without a keyboard. `notes/xenon-firmware.md` records `Warnings and Errors`
set to `Prompt on Warnings and Errors`, `Enable Keyboard Error Detection` ticked, and **eight
`Alert! Keyboard not found` entries in the machine's own event log** across a year. A netboot rig
whose point is an unattended power cycle runs straight into that, and the two settings that would
change it are an architect's for the same reason as the two above. **Network boot without that is a
faster bench session, not an unattended one.**

## Status, 2026-10-24 (UTC)

PARTIAL. The exit criterion is the proof above, a photograph of xenon booting nife with no removable
media, and it does not exist yet. Against the tree:

| Piece | State | Evidence |
|---|---|---|
| Router configuration, parsed and obeyed | built, gated | `bench/xenon-netboot/dnsmasq.conf`; `script/netboot-rehearsal --check`, six clients |
| PXE boot through the real server, no disk | built, gated | `script/ci-build netboot-rehearsal` in CI's `boot-check` job |
| A refused client gets no boot file | built, gated | the same row, `--mac 02:00:00:00:00:01`, ends at `PXE-E16` |
| The bench procedure | written | `notes/x86-uefi-boot/network-boot-xenon.md`, steps 3 and 4 |
| The two firmware settings | done 2026-10-04 | `notes/xenon-firmware.md`'s table; photographs owed |
| The router edit | done 2026-10-04 | below |
| xenon booted over the wire, photographed | **not done** | needs calef |

- Router edit done (calef's homelab agent, 2026-10-04): the four lines are in the router's
  `/etc/dnsmasq.conf` inside a BEGIN/END xenon-pxe block, which the GL.iNet UI does not regenerate and
  firmware upgrades preserve. Server address confirmed `192.168.8.138`, reserved by UCI entry
  `patagonia-wifi`. Backup `/root/pxe-xenon-backup-2026-10-04.tgz`; rollback:
  `ssh root@192.168.8.1 'tar -xzf /root/pxe-xenon-backup-2026-10-04.tgz -C / && /etc/init.d/dnsmasq restart'`.
  `dnsmasq --test` passes, other clients' offers are unchanged, patagonia kept `.138` on reconnect.
- Needs calef: the bench step, steps 3 and 4 of the runbook. On patagonia, on the house LAN,
  `cargo xtask uefi-image --release` and then `script/board-netboot --root target/esp`, allowing
  python3 through macOS's firewall if asked. Take every nife stick out of xenon, cable its
  motherboard ethernet to the LAN, leave the keyboard and monitor attached, and power it on (F12,
  `UEFI PXEv4`, if it does not pick PXE itself). Photograph the `nife self-test:` line and the empty
  ports, copy `board-netboot`'s `done` line, and during the boot run
  `ssh root@192.168.8.1 'logread | grep -i d8:9e:f3:74:b2:a2'`. A lane then writes the photograph
  numbers here and the rate into BUGS, and marks this block BUILT.

## BUGS

- Nothing here has run on xenon, and nothing here can tell you it will. What is proven is OVMF,
  which is EDK2, which is the same codebase Dell's firmware is built from. Xenon is at BIOS 1.27.0
  with an Intel I219-LM onboard NIC (the e1000e family) rather than QEMU's e1000, and the two
  firmware settings that make it try at all are described under Status below. The NIC's source is
  the PCI survey line in the 2026-10-04 photos, class `020000`, `00:1f.6 8086:15e3`, an Intel
  I219-LM; milestone 494 (a driver for the network card a PC actually has) and `notes/e1000e.md` say
  the same.
- Nothing in this repository has ever talked to the house router. The rehearsal implements what
  the `dnsmasq` lines *mean*; it does not prove `dnsmasq` implements them the same way, and it does
  not prove OpenWRT's UI will accept them. The `dhcp-match` half has no UCI form and has to go
  into `/etc/dnsmasq.conf` directly, which is the part of the runbook most likely to be wrong.
- The `dhcp-boot` server address is patagonia's current lease, and a lease that moves makes xenon
  TFTP into nothing with no console to say so. patagonia is at `192.168.8.138` on Wi-Fi `en0` (MAC
  `10:9f:41:d4:8c:0e`), and the pin is in place, but it is bound to macOS's private Wi-Fi address
  `9e:1a:a1:57:40:a6`, not the hardware MAC. It matches only while patagonia's Private Wi-Fi
  Address for the home network is Fixed or Off. Set to Rotating, the reservation silently stops
  matching and `.138` can move again. That condition is met: calef confirmed on 2026-10-04 (UTC)
  that the setting is Fixed. Anyone changing it owes this line an edit.
- The transfer rate is a loopback number and is not a LAN number. 9,210,880 bytes in 1.40s
  (6,429 KiB/s) is python talking to itself through a synthetic ethernet with no cable, no switch
  and no loss. radon's real measurement over TFTP on this LAN was 428 KiB/s, and nobody has measured
  a UEFI client on a real wire.
- macOS's firewall is a failure mode this rig has that radon's did not. `board-netboot` is
  python3 binding a UDP port, and the first inbound packet from a machine on the LAN is the first
  time macOS will be asked about it. It is in the runbook's failure table; it has not been hit,
  because the rehearsal is loopback.
- `script/netboot-rehearsal` answers no IPv6 at all. EDK2 tries IPv4 PXE first, so this costs
  nothing today; a firmware that preferred IPv6 would spend its retries in silence before falling
  back, and the rehearsal would look like a timeout with nothing to say about why.
- No fallback, by design, and that is a real difference from milestone 257. radon's card can
  always boot something; a xenon that fails PXE falls to the next boot entry, and the entries on that
  list are `Windows Boot Manager` and whatever removable media is present. That is somebody's
  installed Windows, which argues for leaving a stick in the machine rather than removing it.
- `board-netboot` serves one transfer at a time, and a UEFI client makes two requests per boot
  where U-Boot makes one. The probe is cancelled promptly rather than held for eighteen seconds, so
  the two do not collide; two *machines* booting at once still would.

## Follow-on

- **Outstanding.** The bench session, which is the proof and cannot be a lane: one power cycle of
  xenon with `script/board-netboot` serving, one photograph, and the real-wire transfer rate for
  this block's BUGS. Calef's; the exact step is under Status, and runbook steps 3 and 4 in
  `notes/x86-uefi-boot/network-boot-xenon.md` have what each screen should show and the failure
  table.
- **Done.** The `dhcp-match` lines' OpenWRT siting: all four lines in the router's
  `/etc/dnsmasq.conf`, written back into `bench/xenon-netboot/dnsmasq.conf`'s header and step 2 of
  the runbook.
- **Done.** The boot server's address is pinned, by the Wi-Fi reservation this block's BUGS
  describes, rather than by an ethernet MAC.
- **Recorded.** Three provisional names waiting on an architect, on `script/names --unratified`'s
  worklist rather than blocking anything: `script/netboot-rehearsal`,
  `bench/xenon-netboot/dnsmasq.conf`, and the `bench/xenon-netboot/` directory. The refusals are in
  the script's own `Name:` block, which is where a reader meets them.
- **Recorded.** The keyboard-at-POST hazard, above and in `notes/xenon-firmware.md`. It is calef's
  two settings and it belongs to the unattended-rig work rather than to this one.

## Index row

PXE needs the DHCP server to point at the boot server, which milestone 257 could avoid and this
cannot. The objection died when calef said the router is OpenWRT: not a second DHCP server, one
option on the existing one, scoped to one MAC
