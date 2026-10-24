# The bench: booting xenon over the network, with no stick at all

*Name: ratified 2026-10-18 (calef, "Yes"). Kept: the directory names a boot path, not a machine, so
the stem keeps its machine.*

An appendix to [notes/x86-uefi-boot.md](../x86-uefi-boot.md), which keeps the summary. This is the
procedure milestone 260 (boot xenon over the network) built, for the machine of milestone 87 (the
x86_64 bare-metal machine), written to be followed rather than interpreted.

It was built by milestone 260, rehearsed on patagonia, and never yet run on xenon. Everything below
except the two firmware settings and the router edit has been exercised end to end under OVMF
(`script/netboot-rehearsal`). It ran against the same `script/board-netboot` that will serve xenon,
with the DHCP answers parsed out of the same `bench/xenon-netboot/dnsmasq.conf` calef is about to
apply.

## Why bother

Copying one file to a stick already works. There are two reasons, and the second is the one that
grows.

- The copy is the tax, and it is paid per boot. The stick procedure is eject, walk, insert, power on,
  and then walk back and do it again for the next build. Milestone 257 (boot radon over the network)
  measured what that costs on the other board. The 2026-09-04 radon session wrote the card six times,
  once per boot, because comparing two builds means interleaving them.
- It removes the co-location. First light is on record as happening because the video output was the
  only channel: *"patagonia could not be moved to the bench"*. A netboot does not need it to be.
  xenon needs an ethernet cable to the house LAN, and the serving Mac needs to be on the same LAN,
  from wherever it is.

## What you need

- xenon, on the house LAN by cable into the motherboard's ethernet. Not the Wi-Fi card in
  Slot2_M.2: `Integrated NIC` switches the LOM and nothing else, and only the LOM can PXE.
- The serving Mac on the same LAN, and its address. `script/board-netboot` prints every private
  address this machine has. That is a list of candidates rather than an answer; see the failure
  table.
- The house router, once, to take the DHCP lines. Nothing in this repository has ever talked to it.
- A monitor on xenon, exactly as for the stick procedure, because the Dell's serial port does not
  carry firmware output, and the firmware is the half that is new here.

## Step 1: the two firmware settings, which are calef's

Both are on one page, `System Configuration → Integrated NIC` (IMG_4031 in
`notes/xenon-firmware.md`). Today it reads `Enable UEFI Network Stack` unticked, with the radio on
Enabled.

1. Tick `Enable UEFI Network Stack`. Without it the firmware has no UEFI networking at all, so there
   is nothing for a PXE boot option to be built on.
2. Move the radio from `Enabled` to `Enabled w/PXE`. This is what puts `UEFI PXEv4` on the boot list.

They are calef's for the reason `notes/xenon-firmware.md` gives about firmware generally: a setting
changes this machine's behavior for everything else it is used for. Edit that note when you change
them, because the next lane reads it instead of walking to the machine.

Nothing else in the transcription is in the way. Boot List Option is already UEFI, Secure Boot is
Disabled, legacy option ROMs are unticked, and `UEFI Boot Path Security` has no effect while no admin
password is set.

## Step 2: the router, once

`bench/xenon-netboot/dnsmasq.conf` is the whole of it, each line explained. `dhcp-boot` names
patagonia at `.138` (reserved 2026-10-04; was `.216`).

On OpenWRT, through UCI (survives a reboot):

```console
# uci add dhcp host
# uci set dhcp.@host[-1].name='xenon'
# uci set dhcp.@host[-1].mac='D8:9E:F3:74:B2:A2'
# uci set dhcp.@host[-1].tag='xenon'
# uci add dhcp boot
# uci set dhcp.@boot[-1].filename='EFI/BOOT/BOOTX64.EFI'
# uci set dhcp.@boot[-1].serveraddress='192.168.8.138'
# uci set dhcp.@boot[-1].servername='patagonia'
# uci set dhcp.@boot[-1].networkid='xenon'
# uci commit dhcp
# /etc/init.d/dnsmasq restart
```

The architecture match has no UCI form, so the two `dhcp-match` lines go in a file dnsmasq reads
directly. On OpenWRT that is `/etc/dnsmasq.conf`, which the UCI-generated configuration includes:

```
dhcp-match=set:efi-x86-64,option:client-arch,7
dhcp-match=set:efi-x86-64,option:client-arch,9
```

The `dhcp-boot` above then needs `tag:efi-x86-64` as well, and UCI's `networkid` takes only one of
them. If that turns out to be awkward, write all four lines into `/etc/dnsmasq.conf` and skip UCI
entirely. The config file in this tree is already in that form, and it is the form the rehearsal
parses and proves.

Do not skip the architecture match to save the trouble. It is what stops this configuration handing
9 MB of x86-64 UEFI image to a machine that is in Legacy boot mode. That includes xenon itself, the
moment anyone sets `Boot List Option` back to Legacy. `script/netboot-rehearsal --check` lists all
six cases and takes under a second.

## Step 3: serve, and boot

On the serving Mac, two commands in two terminals:

```console
$ cargo xtask uefi-image                        # every time the kernel changes
$ script/board-netboot --root target/esp        # in the other terminal, while you work
```

Then power xenon on. There is no third step and no copy. Every later boot is `cargo xtask uefi-image`
and a power cycle. The file the firmware fetches is the same file the stick procedure copies, at the
same path, staged once.

## What you should see, in order

1. On xenon's video output, the firmware's own PXE progress, which is EDK2's and reads exactly as it
   does under OVMF:

   ```text
   >>Start PXE over IPv4.
     Station IP address is 192.168.8.NN
     NBP filename is EFI/BOOT/BOOTX64.EFI
     NBP filesize is 9210880 Bytes
    Downloading NBP file...
     NBP file downloaded successfully.
   ```

   The byte count is not a constant, and is not worth checking against. It is whatever
   `cargo xtask uefi-image` last wrote, and it moves with every kernel and archive change. Two runs an
   hour apart on 2026-09-05 measured 9,210,880 and 9,797,120. What matters is that the number the
   firmware prints and the number `board-netboot` prints are the same number.

2. In the `board-netboot` terminal, two requests and not one, in this order:

   ```text
   board-netboot: 192.168.8.NN <- EFI/BOOT/BOOTX64.EFI (9210880 bytes, blksize 1468)
   board-netboot: EFI/BOOT/BOOTX64.EFI cancelled by the client after the option acknowledgement (a UEFI client's file-size probe does exactly this)
   board-netboot: 192.168.8.NN <- EFI/BOOT/BOOTX64.EFI (9210880 bytes, blksize 1468)
   board-netboot: EFI/BOOT/BOOTX64.EFI done, 9210880 bytes in N.NNs
   ```

   `blksize 1468` is EDK2's own ask. It is what a full-size ethernet frame holds once the IP, UDP and
   TFTP headers are out of it. A client that negotiated 512 instead would still work, and would take
   about three times as long.

   The cancellation line is normal, and is the boot working. EDK2 asks for the file twice: once to
   learn its size, which it does by starting a read and then stopping it, and once to fetch it.

3. Then `nife uefi_loader: milestone 87` on the video output, and the boot tour after it, identical
   to the stick path from here on. The loader carries the kernel and the archive inside itself, so
   nothing after the transfer knows or cares where the image came from.

## If it does not

| Symptom | Most likely cause, in order |
|---|---|
| No `>>Start PXE over IPv4` on the screen at all | The two firmware settings did not take, or took on the wrong machine. `Enabled w/PXE` is what creates the boot entry; `Enable UEFI Network Stack` is what lets it exist. Check `F12`'s one-time boot menu for a `UEFI PXEv4` entry: no entry means step 1 |
| `PXE-E16: No valid offer received` | The router is answering, and answering with no boot file. Either the MAC in `dhcp-host` is wrong (read it off `General → System Information`, `LOM MAC Address`), or the `dhcp-match` tag is not being applied and the `tag:efi-x86-64` requirement is failing. This is the same output `script/netboot-rehearsal --mac <not-xenon>` produces, deliberately |
| `PXE-E18: Server response timeout` | The offer arrived and the transfer did not. The server's address in `dhcp-boot` is stale or is an address xenon cannot route to (a Tailscale address, or the wrong side of a Wi-Fi/ethernet split); or `board-netboot` is not running; or macOS's firewall is dropping inbound UDP to python3, which is the failure this rig has that radon's did not, because radon's traffic was on a port macOS had already been asked about |
| `board-netboot` says `NOT FOUND` with a name in it | The filename in `dhcp-boot` and the layout under `--root` disagree. It is a path relative to the served directory, so `--root target/esp` wants `EFI/BOOT/BOOTX64.EFI` and nothing else |
| Transfer completes, then nothing | The firmware executed the image and the loader's video output is the next thing. If the screen is blank, that is milestone 87's territory rather than this one, and a stick will reproduce it without the network in the way |
| The machine sits at POST waiting for a keypress | `Alert! Keyboard not found`, which this rig runs straight into and does not fix. See below |

## What this does not fix, and it is the thing that will bite next

xenon halts at POST without a keyboard. `notes/xenon-firmware.md` records `Warnings and Errors` at
`Prompt on Warnings and Errors`, `Enable Keyboard Error Detection` ticked, and eight
`Alert! Keyboard not found` entries in the machine's own event log across a year. A rig whose point
is an unattended power cycle runs into that. The two settings that would change it are calef's, for
the same reason as the two above. Network boot without them is a faster bench session, not an
unattended one.
