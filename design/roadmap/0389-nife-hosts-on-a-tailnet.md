---
status: NOT-STARTED
raised: 2026-09-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 389. Getting nife hosts onto the tailnet, which mostly does not need nife

Filed 2026-09-05 as an unnumbered proposal after calef asked whether a
milestone covered Tailscale; numbered 2026-09-19 by milestone 433's drain of the proposal pile.
Premise re-read against the tree on 2026-09-19 and still true: nothing in this repository
implements or depends on `tailscale`, `wireguard`, `headscale`, `curve25519` or `chacha20`. The
five places Tailscale is named are all the same finding recorded below (the CGNAT default-route
hazard, in `xtask/src/main.rs`, milestone 257 (boot radon over the network)'s block,
`notes/visionfive2.md` and
`notes/x86-uefi-boot.md`). Of the two things this sat behind, the resolver is built (milestone 384
(in a capability system the resolver is a grant), with its grant and protocol in §252 (a resolver
grant is one zone per client badge)). TLS is still absent (milestone 387 (a TLS stack)).
*(Number provisional until the merge queue lands it.)*

The reason is the finding rather than a formality. The near-term answer needs no
nife work at all, so nothing here waits on anybody.

## This question came from the lab, which almost none of them do

AGENTS.md ranks work by the shortest path to a system a customer runs, and that path has been
vacant since 2026-08-30. This is not a workload and does not fill it. It is worth marking anyway,
because it is the first requirement in weeks that arrived from the environment the machines actually
live in rather than from the roadmap's own logic, and the ranking function is supposed to notice
that difference.

## The near-term answer, quoted rather than recalled

From `tailscale.com/kb/1019/subnets`, read 2026-09-05:

> Subnet routers let you extend your Tailscale network (known as a tailnet) to include **devices
> that don't or can't run the Tailscale client**.

So a machine already on the tailnet advertises the route covering the bench local area network, and
every tailnet device reaches radon and xenon. Zero nife work, and it is Tailscale's designed answer
for this case rather than a workaround.

patagonia is already a tailnet node, measured the same day: `utun6` at `100.75.22.70`, and that
interface owns the default route. It is also on the bench local area network at `192.168.8.216`, so
it could advertise `192.168.8.0/24` today. cordoba is the better choice if it is on the tailnet,
because patagonia sleeps and cordoba is the always-on box, and a subnet router that is asleep is a
subnet that is gone.

## The hazard this already caused, before any nife work existed

A Tailscale default route makes "what is my own address" ambiguous, and it cost milestone 257 a
correct implementation. That lane's first server-discovery asked the routing table which address to
advertise, by connecting a UDP socket and reading the local address. On patagonia that answers
`100.75.22.70` every time, because the default route is the tailnet, and **that is a CGNAT address
radon has no path to**. A card written that evening would have silently fallen back to its own SD
copy forever, and a session would have been measuring a stale kernel while believing otherwise.

The lane found it by measurement and replaced the discovery with interface enumeration that drops
anything outside RFC 1918. Recorded here because the next thing that needs to know its own address
will hit the same wall, and because it is evidence about what joining a tailnet does to a machine
rather than a Tailscale defect.

## A native client is the wrong target, and their docs say why

`tailscaled` is Go, so a client means a Go runtime on nife, which is a project rather than a
milestone. Even on Linux it wants a tunnel device:

> Tailscale works on Linux systems using a device driver called `/dev/net/tun`... However, not all
> Linux systems support `/dev/net/tun`.

And the mode that exists for machines without one degrades to something weaker than membership:

> `tailscaled` functions as a SOCKS5 or HTTP proxy which other processes in the container can
> connect through.

That is "nife processes can egress", not "nife hosts are on the tailnet". The coordination protocol
is also not a stable public API; Headscale reimplements it by tracking their client, which is a
maintenance relationship this project should not take on for a convenience.

## The interesting middle is WireGuard, not Tailscale

Tailscale's data plane is WireGuard: Curve25519, ChaCha20-Poly1305, BLAKE2s, over UDP. That is
bounded and well specified where the control plane is neither, and
[§46](../decisions/0046-dependency-rule.md) puts the crypto squarely on the take side. It would put
a nife host on the lab network cryptographically with no coordination plane at all.

And it is the third instance of one pattern in a single evening, which is what makes it worth
writing down rather than filing as networking work:

| ambient on Unix | a capability here |
|---|---|
| `/etc/resolv.conf`: any program resolves any name | a resolver grant that answers for one domain (milestone 384) |
| `/etc/ssl/certs`: any program verifies against all 121 authorities | a trust store granted as the roots one peer chains to (milestone 387) |
| a tailnet: joining puts **every process on the box** on the whole network | a peer grant: a program given a tunnel to one peer cannot reach the rest of the tailnet |

The third row is the one a Tailscale user would feel. Tailscale's own ACLs are enforced at the
tailnet's edges, per device; nothing on the device stops one process from using another's
reachability. A capability system can put that boundary inside the machine, and that is a real
difference rather than a restatement.

## What it sits behind

Everything else does. The crypto surface is `argon2`, `subtle` and `aes`; there is no DNS at all
(MagicDNS is a resolver); and there is no TLS. The subnet router removes the whole near-term need,
which is why this is a proposal with no milestone attached.

## What would turn it into one

- A nife host that must be reachable **when the subnet router is not**, which is a real gap the
  moment anything depends on a board being up while cordoba is not.
- Or the confined-peer demonstration being wanted for its own sake, which is
  [§145](../decisions/0145-compartmentalization-at-process-cost.md)'s question rather than this
  one's.

## BUGS

- Nobody has checked whether cordoba is on the tailnet, and the subnet-router recommendation
  rests on it. patagonia is, measured; cordoba is assumed.
- **No WireGuard implementation has been evaluated.** Rust ones exist and none has been read against
  this tree's `no_std` userspace or §46's test, so "take it" is a direction rather than a choice.
- **The subnet router is not free of tradeoffs and this proposal does not price them.** Advertising
  a route puts the whole bench local area network on the tailnet, not just the boards, and whether
  that is wanted is calef's network decision rather than a nife one.
- **This is not a customer**, and saying it came from the lab should not be read as filling the
  ranking function's vacancy. It is a lab convenience with real value and no workload behind it.

## Index row

calef asked whether a milestone covered Tailscale, since the lab depends on it for several things
and nife hosts will need to be reachable. The finding is that the near-term answer needs no nife
work at all: a subnet router is Tailscale's own designed answer for devices that cannot run the
client. A machine already on the tailnet advertises the route covering the bench local area network,
and every tailnet device reaches radon and xenon. cordoba is the right one to do it. Patagonia
sleeps, and a subnet router that is asleep is a subnet that is gone. A native client is the wrong
target and their documentation says why: `tailscaled` is Go, it wants `/dev/net/tun`, and the mode
for machines without one degrades to a SOCKS5 proxy, which is "nife processes can egress" rather
than "nife hosts are on the tailnet". The coordination protocol is also not a stable public API. The
interesting middle is WireGuard, which is Tailscale's data plane and is bounded and well specified
where the control plane is neither. This block also carries the hazard joining a tailnet already
caused: a default route through the tailnet makes "what is my own address" answer with a CGNAT
address nothing on the bench local area network can reach, which cost milestone 257 a correct server
discovery. And it carries the third instance of one pattern in a single evening, beside milestones
384 and 387. Joining a tailnet puts every process on the box on the whole network, where a peer
grant would put that boundary inside the machine. It is a lab convenience with real value and no
workload behind it, and it does not fill the ranking function's vacancy.
