---
status: REFUSED
raised: 2026-09-02
refused_by: 257
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: radon (the work is radon's own outlet and its network path)
needs_person: yes
---
# 224. Nothing can power-cycle radon, so a hung soak needs a person

Refused by calef on 2026-09-04 (UTC). With the four options below in front of him and the diagnosis
re-measured, calef chose manual power. Milestone 257 (boot radon over the network) is what made that the cheaper
answer: it took away the card, which was the larger cost of a bench session, and left remote power
buying only the hung-board case. This is the soak's remaining gap after milestone 221 (the soak
never crosses cores, so build the hook that makes it).

Every option left below needs a person at the router or at the outlet, in the second sense of
HARDWARE that `notes/roadmap.md` names, the one that never discharges by waiting. Reading the access
point's configuration, moving the board's outlet, putting patagonia's radio on the plug's network
and writing one firewall rule are all hands rather than decisions, and no lane can do any of them.

In brief. **radon**'s power is on a Kasa KP303 strip. calef wired it on 2026-09-01, and outlet
2 is the board while another outlet feeds an external drive that must never be switched off.
`python-kasa` and a `vf2-power` wrapper are installed on patagonia, and the wrapper addresses the
board's outlet by alias with no way for a caller to name a different one. An index is right
until a firmware update reorders the children.

None of it can reach the plug. Measured on 2026-09-01 from both patagonia and cordoba, by
broadcast and by unicast to every host on the subnet, with both the legacy and the newer discovery
protocols: zero replies. ARP for the plug's address is `incomplete` from wired and wireless
alike, while five other clients on the same subnet answer normally. The vendor app controls it,
which it can do through the vendor's cloud.

So the plug is deliberately isolated from other clients on the network it is joined to.

## Revisit

- **Condition.** The fatal risk 5 soak hangs radon, which is the case manual power costs an evening on. A fresh choice among the four options below is calef's on that day.

## Why it matters now rather than as a convenience

Fatal risk 5 (it cannot be made reliable on multicore, and the bugs appear only on silicon) wants
sustained runs, and after milestone 221 there are two experiments to sustain rather than one. A
soak that never crashes needs no power cycle, so this does not block a first run. A soak that
hangs is exactly the outcome the experiment exists to produce, and that outcome currently ends
with a board sitting hung until somebody walks to it.

`script/board-console` reads and never writes, so a boot that lands at U-Boot's prompt cannot be
rescued either.

## The options, none chosen, and the first is not free

- Turn off client isolation on the network the plug is joined to. One toggle, and it is a real
  security control: the isolation is what stops a compromised IoT device reaching cordoba, which
  holds the family's backups. Weakening it network-wide to control one plug is a larger change than
  it looks, and this block declines to recommend it.
- Move the board's outlet to a plug that is already reachable, leaving the KP303 alone. Avoids
  touching the strip an external drive depends on, and avoids weakening anything.
- Accept manual power and treat a hung soak as a bench event. Honest, and it costs an evening
  every time the experiment produces its most interesting result.

## Re-measured 2026-09-04, and the diagnosis got sharper

The 2026-09-01 finding reproduces exactly. From patagonia, on the evening radon was first put on
ethernet: zero Kasa replies, by broadcast on both of patagonia's interfaces and by unicast to
all 254 addresses on the subnet, using the legacy port-9999 protocol. The plug is not reachable and
this is not a transient.

**Two facts this block did not have.**

The subnet is `192.168.8.0/24` and the gateway is `94:83:c4:be:26:de` at `192.168.8.1`, which is
GL.iNet. That address is GL.iNet's factory default LAN and their guest network is isolated by
design, so "client isolation" stops being an inference from a symptom and becomes a named feature of
a specific product. Nobody has still read the configuration, so the BUGS entry below stands, but the
guess is now a much better one.

patagonia is already dual-homed, and the second interface is redundant. `en0` is Wi-Fi
(`192.168.8.216`) and `en9` is a USB ethernet adapter (`192.168.8.206`), both on the same subnet.
Kasa plugs are Wi-Fi only, so the plug is on some SSID this machine is not on.

## Two options this block did not list, recorded for whoever revisits it

Both are cheaper than the network-wide toggle this block declined to recommend, and neither was
available to it because both depend on the two facts above.

- Put patagonia's Wi-Fi on the network the plug is on, and leave the main LAN on the wired
  adapter. Nothing changes on the router, no isolation is weakened anywhere, and **cordoba is not
  involved**, which is the whole of this block's stated objection to the first option. The cost is
  real and is patagonia's rather than the network's: the dev machine becomes reachable from the IoT
  network. Reversible in one click.
- One firewall rule rather than a global toggle. This block's first option is written as
  "turn off client isolation", and an accept rule from one source to one destination is a materially
  different security decision from disabling the control network-wide. GL.iNet may not expose
  per-host exceptions above the LuCI layer, which is what would have to be checked.

## Why it was still declined, and this is the honest version

**Not because the options are bad.** calef chose manual power on 2026-09-04 with all four in front
of him. What changed the calculus that evening is that milestone 257 removed the larger tax: the
card, not the power button, was what made a bench session expensive, and network boot takes that
away without touching anyone's network. Remote power buys the *hung board* case, which fatal risk
5's soak produces and ordinary work does not.

So the cost of staying manual is now narrower than this block priced it, and it is still exactly
what this block said: an evening every time the soak produces its most interesting result.

## BUGS

- This block cannot verify its own diagnosis. Client isolation is inferred from the symptom, a
  device that answers its vendor's cloud while being invisible to ARP from two machines on its
  subnet, and from the absence of any other explanation. Nobody has read the access point's
  configuration.
- **Remote power is not remote recovery.** A board wedged before U-Boot hands off is fixed by a power
  cycle; a board that needs a different card is not. Milestone 257 narrows this: with the payload
  served over TFTP the "different card" case mostly becomes "different file on patagonia", so what
  remains genuinely card-bound is a change to the boot script itself.
- Nothing here was tried against the plug's own network, because nobody has said which one that
  is. Every measurement in this block is a negative taken from networks the plug is not on, which is
  consistent with the diagnosis and proves less than it appears to.

## Index row

calef chose manual power 2026-09-04 after the diagnosis was re-measured: still zero Kasa replies
from either of patagonia's interfaces, and the gateway is GL.iNet, whose guest network isolates by
design. Milestone 257 removed the larger tax (the card), so staying manual costs less than this
block first priced it
