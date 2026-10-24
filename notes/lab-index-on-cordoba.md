# Serving the lab's update index from cordoba: a spec for calef's homelab agent

calef ruled on 2026-10-07 (UTC), on pull request #1805: *"Start on cordoba."* The lab machines
(radon, xenon, and argon once it arrives) take their updates from an index served on the house
network by cordoba, the always-on Ubuntu box. cordoba is the homelab agent's, not nife's, so a nife
lane writes this and does nothing on cordoba. The plan it serves is
[milestone 858 (lab machines update themselves)](../design/roadmap/0858-lab-machines-update-themselves.md).
Name provisional.

## Goal and why

A lab machine whose owner has turned on automatic updates pulls its channel, installs, and restarts
what changed (§250 (an image names its distribution's package index)'s amendment of 2026-10-07).
cordoba is that channel's server for this house. It is one owner's choice of source, so nothing in
nife may assume it exists.

Two rules from the same rulings shape the job:

- CI never deploys. basalt's gate publishes a release; cordoba fetches it on its own timer. Nothing
  on GitHub logs into cordoba or pushes to it.
- cordoba mirrors and never signs. The index is a TUF repository (Fork 9), and a mirror copies it
  byte for byte, so a lab machine verifies everything against basalt's root key, not cordoba's.
  cordoba holds no signing key.

## What exists today, and what does not

Check this first: most of the content does not exist yet, and the spec is written so the server
can stand up before it does.

| piece | state on 2026-10-07 |
|---|---|
| basalt's gate | runs on each pin bump, daily at 06:17 UTC; keeps test builds as 14-day workflow artifacts |
| a published basalt TUF repository | not built (worklist item 9 in the plan) |
| the per-merge pin bump | not built |
| a nife client that fetches from anywhere but QEMU | not built: the source is compiled in as `10.0.2.9:8080` (`notes/packages.md`, BUGS) |

So step 1 below is useful now, and steps 2 and 3 wait on basalt.

## Step 1: serve a directory

- Serve `/srv/nife/lab/current/` read-only over plain HTTP on the house LAN at
  `http://<cordoba's LAN address>:8090/lab/`. The port is the homelab agent's choice; tell calef
  which. Plain HTTP is enough: every byte is verified by TUF signatures and digests (§195 (a
  reviewed recipe vouches for a package)).
- LAN only. Not on the tailnet, not through the router to the internet.
- GET and HEAD only, `Content-Length` on every reply, no directory listings, no redirects, no
  compression. nife's HTTP client is minimal, and the only server it has met answers HTTP/1.0
  (`helpers/package-http-peer`).
- `current` is a symlink to a dated directory, so a sync swaps it in one rename.
- Give cordoba a DHCP reservation on the router if it lacks one, and report the address. The lab
  machines will be given the address, not `cordoba.local`, since nife does not speak mDNS.

## Step 2: the layout it will serve

The TUF repository as basalt publishes it, unchanged. The file names are TUF's; the version in the
file name is the specification's consistent-snapshot form.

```
/lab/metadata/root.json, /lab/metadata/<N>.root.json   every root, so a client can rotate
/lab/metadata/timestamp.json
/lab/metadata/<N>.snapshot.json
/lab/metadata/<N>.targets.json                         and delegated roles beside it
/lab/targets/<sha256>.<name>                            packages and the release manifest
```

The release manifest (Fork 5) is one of the targets: it pins a kernel and the base versions tested
with it, so a mirror cannot mix releases.

## Step 3: follow basalt's passing gates

A systemd timer on cordoba, every 15 minutes:

1. Ask basalt for its newest release from a passing gate. The exact place (GitHub Releases on
   `nifeos/basalt` is the likely one) is basalt's to publish, and this step waits on it.
2. Download it into `/srv/nife/lab/<UTC timestamp>/`.
3. Verify it before serving it, as a client would: with a TUF client on cordoba (for example
   `tuftool`, from awslabs/tough, or python-tuf) against the root that calef pins on cordoba once
   basalt has one. Refuse a release whose versions go backwards.
4. Swap `current` to it. Keep the previous three directories, for comparison when a lab run fails.
5. Log one line per run: the release, its version, and pass or refused.

Do not re-sign, re-compress or rename anything. A mirror that changes a byte breaks every client.

## What the lab machines need

- To reach cordoba's address and port from the lab's LAN. radon takes `192.168.8.200` by DHCP
  (milestone 257 (boot radon over the network)); xenon's address is in the router's lease table.
- Nothing else from the homelab agent. The nife side (a configurable source, TUF verification,
  automatic updates as an owner setting) is nife's work and is listed in the plan.

## Verify

- From another machine on the LAN: `curl -sI http://<address>:<port>/lab/metadata/timestamp.json`
  returns 200 once step 3 has run, and 404 before.
- A request for a path outside `/lab/` returns 404.
- After a sync, `readlink /srv/nife/lab/current` names the newest directory, and the log line says
  pass.

## Constraints

- Never touch the lab machines, the smart plugs, or any USB hub.
- Do not disturb Immich, the backups, the LiteLLM gateway or the netboot roles cordoba already has.
- No signing key on cordoba. No credentials for GitHub beyond anonymous reads, unless basalt's
  release turns out to need a token; then ask calef.
- This file is the nife side's request. The homelab repository is the record of what was done.
