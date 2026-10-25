# `jig` on radon: milestone 801's riscv64 leg

*Name: provisional, minted 2026-10-24 (UTC) by `maintainer/record-2026-10-24-b`; naming is an
architect's.*

calef ruled on 2026-10-24 (UTC) that milestone 801 (packages over the internet) closes on real
silicon on three machines. This is the riscv64 leg's bench step, an appendix to
[jig.md](jig.md). The procedure's form is notes/designware-ethernet.md's "calef's bench step on
radon".

It cannot pass yet. radon's ordinary image has the NIC, a lease and `jig`. It has no entropy at
the prompt, so no resolver and no TLS. It has no file service, so nowhere to keep the index copy
or an install, and no wall clock to check a certificate's dates. The proposal
[radon's booted system runs `jig`](../../design/roadmap/proposals/radons-booted-system-runs-jig.md)
is that work, and the wall clock waits on a ruling of calef's there. Step A below runs on today's
tree and records which gap bites first; step B is the leg itself, once the proposal is built.

## Before either step

radon's card was written once with `--tftp` (milestone 257 (boot radon over the network)), so
nothing on it changes. On the Mac radon's UART goes into, in a worktree of the commit under test:

```sh
pgrep -l qemu                                   # nothing of yours should be running
git log -1 --format=%h                          # goes in the Results row
cargo xtask std-exerciser                       # builds jig; an archive without it is refused
```

In a second terminal, the TFTP server, left running: `script/board-netboot`. The Ethernet cable
goes in `ethernet@16030000`'s port, the one U-Boot netboots over, on a LAN whose DHCP hands out a
name server and reaches the internet.

## Step A: what today's image says (two minutes, any radon evening)

```sh
NIFE_INDEX_SOURCES=basalt script/board-image --tftp            # must print SEALED
mkdir -p bench/radon-$(date -u +%F)
script/board-console --board radon --port /dev/cu.usbmodem<id> --for 5m --until none \
    --log bench/radon-$(date -u +%F)/801-jig-today.log
```

Power-cycle radon at its own outlet (never another outlet, and never a USB hub; which is which is
kept off-tree by the maintainers). At `$`, type `jig update`, then `jig install ioping`. Each line
it prints is the record; the proposal predicts a refusal at the resolver, the TLS client or the
index store, and the log says which. `reboot` puts the board back for the next step.

## Step B: the leg, once the proposal is built

The build gains whatever switch the proposal's RAM disk takes; until it exists, this is the shape:

```sh
NIFE_INDEX_SOURCES=basalt script/board-image --tftp <the RAM disk's switch>   # SEALED
script/board-console --board radon --port /dev/cu.usbmodem<id> --for 10m --until none \
    --log bench/radon-$(date -u +%F)/801-jig.log
```

Power-cycle radon. Before `$`, the boot must say `entropy service up`, `name resolver up` and
`directory grants get a bound badge`; without all three, stop and keep the log. Then type:

```text
jig update
jig install ioping
cd packages
ioping -v
```

| line | means |
|---|---|
| `index updated: N packages for riscv64, from basalt.nifeos.org over TLS` | the index came by name over pinned TLS |
| `fetched ioping-1.3-riscv64 from ... over TLS`, then `fetched and installed; generation N is live` | 801's riscv64 leg is met |
| `ioping 1.3` | the installed program runs |
| `index unreachable at basalt.nifeos.org: not resolved: ...` | the resolver failed; the parenthesis names its step |
| `index unreachable at basalt.nifeos.org: <other>` | resolved, then the connection or TLS failed; a certificate date error is the clock |
| `passed over https://...: <why>` | the Release location was refused; `jig` falls back to Pages' `targets/` |
| `this jig holds no index store` | no file service reached `jig` |

basalt's index lists `ioping-1.3-riscv64` beside the other two architectures (checked 2026-10-24
UTC); `jig list` after `update` shows it on the board.

## What the run proves, and what it does not

It proves the fetch end to end on riscv64 silicon: a name resolved through the lease's server, TLS
pinned to ISRG Root X1 on radon's own NIC, the digest check, and the progenitor installing on the
index copy's word. It does not prove that radon keeps an install. The RAM disk is gone at the next
power cycle or `reboot`, so a second boot starts with no generation. radon's persistent disk stays
§260's USB drive. 801's riscv64 leg is the fetch, so this is enough to close it; a later
boot-to-boot claim belongs to the USB drive's milestones.

## Recording

`LC_ALL=C tr -cd '\11\12\15\40-\176'` over the log and nothing else, then commit it under
`bench/radon-<date>/` with a README naming the commit and the image's features. A Results row goes
here and a line in milestone 801's block.

### Results

| date (UTC) | commit | step | last line | verdict |
|---|---|---|---|---|

## BUGS

- Step B depends on a switch that does not exist yet, and its exact spelling comes with the
  proposal's build.
- The DesignWare port moves 14.3 Mbit/s (milestone 53), so a large package takes a while; `ioping`
  is small.
