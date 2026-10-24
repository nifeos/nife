# Booting over the network, in detail

An appendix to [notes/visionfive2.md](../visionfive2.md), which keeps the two commands. This file
holds the design of the TFTP path from milestone 257 (boot radon over the network) and the measurements behind it.

## Why it exists

Milestone 257 built it on 2026-09-05, after the path was proved by hand at the prompt on
2026-09-04. The paragraph this replaces predicted the day it would matter, and was right. The
2026-09-04 bench session wrote the card six times, once per boot, because a comparison of two builds
has to interleave them.

It also got one thing wrong, corrected by calef the same evening. The server is on patagonia, not
cordoba. radon's UART goes into patagonia, and patagonia is where the images are built. So serving
from there means there is no copy step at all: build, power cycle, watch, on one machine.

## The card is still underneath

That is the point rather than a hedge. The generated script tries `dhcp` and two `tftpboot`
transfers. It falls back to `load` from the card's own copy when any of it fails: no cable, no hub,
no lease, and the board still boots something. A card that can be bricked by an unplugged cable
would be a worse rig than the one being replaced. `netretry` is set to `no` first. So a network that
is not there fails in seconds rather than retrying while nobody is watching.

## Nothing is ever assembled from two places

The kernel and the archive are one measured pair. So a transfer that gets the kernel and loses the
archive falls all the way back, and takes *both* from the card. Half a pair halts at
`MEASURED BOOT REFUSED`. That is the gate working, and it would be working on a fault we built.

## No server address is written down in this tree

That is on purpose. `192.168.8.216` was true on the evening the path was proved. Nobody checks it
afterwards, and a DHCP lease can move it: exactly the defect class of milestone 256 (x86_64 places PCI BARs in a hardcoded window). So
`cargo xtask board-script --tftp` reads the address off the machine writing the card, at the moment
it writes it. The script echoes it at boot:

```
nife: tftp server is 192.168.8.216, setenv nife_boot_server to point somewhere else
```

A console log therefore says what a card expects before anything depends on it. When the address
has moved, the fix is one line at the prompt and no card reader:

```
StarFive # setenv nife_boot_server 192.168.8.42
StarFive # source ${scriptaddr}
```

## The script's shape, and its two settings

The network-first script is the dullest shape that can express a fallback: one state variable, `if
cmd; then` and `fi`, nesting never deeper than two, and no `else`. U-Boot's parser is a cut-down
hush, and nobody has watched this script run on the board. A chain of guards costs a few more lines
than nested branches and puts fewer parser features between the card and a board that boots. A host
test holds the shape; another runs the script under `/bin/sh` with the U-Boot verbs stubbed, which
proves the control flow but not that hush accepts it.

`autoload no` is required because bare `dhcp` in U-Boot means "get an address and TFTP the bootfile
the DHCP server names". On a home network that server is the router. Without the setting, radon
fetched from the router on 2026-09-05, reported `TFTP server died`, and `dhcp` returned failure. The
whole network branch was skipped and the card fallback fired, so a bug looked like a clean fallback.
No host test could catch it, because every test stubs `dhcp`.

`netretry no` makes a missing network fail in seconds instead of retrying while nobody is watching.
That is the difference between a fallback and a hang.

## It reads interfaces, not the routing table

Not a connected UDP socket whose local address the kernel picks from the route, because the default
route can belong to a VPN interface, and every probe would then answer a CGNAT address the board
has no path to. Interfaces are enumerated instead, and anything outside RFC 1918 is dropped.

patagonia has two addresses on the bench LAN: `en0` at `.216` and a USB adapter at `.206`. Either
serves equally well, because the server binds every interface. The first is taken, and both are
printed so `--server` can pick the other.

## Measured on 2026-09-04, over the wire

The transfer moved 282,624 bytes of kernel in 1.4 s and 9,044,480 bytes of archive in 20.6 s.
That is 428 KiB/s and about 6,200 round trips. It is slower than a card read and very much faster
than a walk to the bench.

And that boot was a control nobody asked for. The image served was the padded E3 build. So it is a
fourth reading of that condition, taken through a completely different load path: DHCP, ARP and
TFTP instead of a FAT read. `ipc_rtt` read 4311, `call_reply` 5089 and `ipc_rtt_el0` 124917. Every
one is inside the card-booted cluster of three. How the kernel arrives does not perturb what it
measures. That was the one thing that could have made this workflow useless for the bench work it
exists to serve.

## Why `script/board-netboot` and not dnsmasq

dnsmasq is somebody else's tested code and is not in the shipping graph. That is a real argument,
and it is why the decision was made rather than assumed (§46 (thin primitives or whole subsystems)). It lost on one point.
dnsmasq is a DHCP server that also does TFTP. This LAN is a family's house network with a router
already handing out leases. A second DHCP server on it is an outage for everyone in the building.
`--port=0 --enable-tftp` with no `--dhcp-range` is safe, but only as long as every future invocation
stays right. A tool that cannot speak DHCP at all cannot get that wrong.

python3 is also already what ten `script/` entry points are written in. So this asks nothing new of
anybody's machine, while `brew install dnsmasq` does. Port 69 binds without root on patagonia
(checked 2026-09-04, rechecked 2026-09-05), so there is no `sudo` in the runbook.
