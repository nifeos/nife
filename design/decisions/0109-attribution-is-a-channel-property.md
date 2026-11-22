---
status: DECIDED
raised: 2026-08-22
decided: 2026-08-22
ratified_by: calef
---

# 109. Attribution is a property of a channel, not of a capability

calef, 2026-08-22, on milestone 49 (what identity is for once it stops being authority)'s own named fork (worked out in
conversation, not yet built): channel.

## The question

Milestone 49's "In brief" names Unix's uid as doing four jobs at once, three already answered here
structurally. The fourth, attribution ("who did this?"), has no mechanism at all. The fork the
milestone names to settle first: does attribution become a property of a capability, or a property
of a channel? As a property of a capability, an invocation carries a stamped origin (seL4's badge
mechanism). As a property of a channel, a server logs which endpoint a request arrived on,
established once when that endpoint was handed out.

## What else was considered, and why it loses

**Badged capabilities are not a new idea being weighed for the first time.** This exact fork has
come up three times already in this tree, independently, under different names, and lost every
time:

- **The compositor** (`notes/compositor.md`): every client shares one endpoint with no sender
  identity. Rather than badge it, the message is made to carry nothing and every per-client fact
  lives in per-client memory instead. "There are no badged capabilities here (DECISIONS §26.5
  records that decision and what would bring it back)."
- The FS server / directory confinement (`notes/dir-capability.md`): a shared endpoint's handle
  table is per-server, not per-client, so rights on a handle are not confinement. Badging is named
  as seL4's answer and explicitly not taken; the fix is a whole separate caretaker process, its own
  address space.
- **The fault endpoint** (`notes/supervision.md`): no badge is needed because the kernel is the only
  sender on this path. "seL4 solves the general untrusted-sender case with badged capabilities; that
  machinery returns as its own decision if a supervision endpoint ever needs trustworthy identity
  from userspace senders."

No badge field exists anywhere in the ABI today (checked directly: `crates/moho_abi`, `crates/capability`).
In each prior case the actual requirement was confinement, not identity. A badge answers neither
question a shared endpoint actually raised. It tells a receiver who called, never whether the
message content can be trusted, which is what compositor's and dir-capability's fixes both needed.

## Why seL4 built it and this tree has not

seL4 is a general-purpose foundation for downstream systems it does not control the shape of. Some
of those legitimately want one cheap, shared endpoint serving many, perhaps many thousands of,
distinguishable clients (a resource manager fielding requests from many VMs, for instance). Minting
a badge is nearly free; minting a kernel object or a process per client is not, at that scale. This
tree has never had that scale problem. Every server built so far serves a small, fixed number of
principals. Per-principal objects have been cheap enough that the tree reached for them three times
running rather than reach for a badge.

## The decision

**Channel.** A server that wants to know who is asking gives each principal its own endpoint,
established once (at login, at spawn, wherever the principal is created), and logs which one a
request arrived on. This is not a new mechanism; it is the pattern this tree already uses
everywhere identity-shaped information has mattered, generalized rather than invented.

It composes with milestone 152 for free. 152's durable per-user sessions already give every
downstream capability a traceable per-user origin: once a user's session exists, every service it
reaches through session-derived capabilities is reachable through a channel unique to that user.
Attribution at user granularity, which is what audit actually wants, falls out of 152's own shape
with no additional mechanism.

## What this costs, and what it does not

Channel: no kernel change, reuses proven machinery (§26's fault endpoint, caretaker
supervision, `SINK_BIT`-shaped delegation). The cost lands on server structure (one endpoint or
region per principal) rather than on the kernel.

Badging would touch the IPC fast path (the single most gated, most measured surface in this kernel:
`fastpath-footprint`, milestone 132 (the fast path's footprint)'s L1i-sized budget) for the first
time in the project. It would also need new, proven kernel semantics for how a badge interacts with
capability derivation, copying and revocation. Real cost, for a feature with no current consumer.

Where the channel model's own cost eventually bites, named rather than assumed away: each durable
session, as milestone 152 designs it, plausibly owns at least one kernel region (`MAX_REGIONS = 256`,
`kernel/src/untyped.rs`, a system-wide concurrently-live cap), plus whatever caretakers it holds
long-term. A rough estimate, not a measurement: on the order of 50-70 concurrently-durable sessions
before the region table is a real constraint, against a system footprint of its own already
consuming some of that budget. `MAX_REGIONS` has been raised once already (16 to 256,
`notes/heap.md`) and raising it again is the first, cheap response if this tree ever approaches that
number, long before badging would be the right call.

Comparative context, corrected from an earlier draft of this decision that anchored the wrong
number. nife is a general-purpose operating system; file serving (Time Machine) is one workload
on it, not the one this fork's headroom should be measured against. An earlier pass here cited
Windows Home/Pro's 5/20 concurrent-SMB-connection cap and macOS's 5-simultaneous-user fast-switching
limit, both of which are real, sourced numbers, but they measure a specific, deliberately
product-throttled consumer feature, not what a general-purpose multi-user OS actually supports.
Neither Windows nor macOS caps how many *accounts* can hold durable, persistent per-user state (a
crontab, a systemd `--user` unit with `loginctl linger`) anywhere near 5 or 20. That model is
resource-bound, not artificially capped. Unix cron has run one crontab per account with no
built-in ceiling since the 1970s. Shared systems have run that for accounts numbering in the
hundreds as ordinary practice, though this note does not have a single precise citation for a
figure and should not be read as claiming one.

seL4 itself supplies no comparison at all, and that is worth being precise about too. seL4 has no
user, login, or account concept anywhere in the kernel; badging distinguishes *senders on a shared
endpoint*, for whatever a downstream system builds (VMs in a hypervisor, components in a CAmkES
system), never "logged-in end users" as such. Citing seL4 as having solved "how many users" would be
citing an answer it does not give. The honest version of the earlier reasoning (§2 (primary target),
§3 (use the crate ecosystem) above) is narrower and still holds. seL4 supports cheap many-sender
fan-out for whatever a downstream system needs it for. This tree has never needed that, and every
prior case here needed confinement rather than identity regardless.

**So the number that actually matters is `MAX_REGIONS` itself, not a borrowed desktop-OS throttle.**
The 50-70-concurrently-durable-session estimate above is the real headroom question for a tree that
means to be general-purpose, and it is closer than "comfortably distant" once measured against
cron's actual, uncapped norm rather than a file-sharing product limit. The right framing is not
"this will never matter" but "raising `MAX_REGIONS` again, the same cheap move already made once
(`notes/heap.md`, 16 to 256), is a normal, expected action for this tree to take as durable sessions
are actually built and measured, not a distant hypothetical to defer indefinitely."
[Windows connection limits (Windows OS Hub)](https://woshub.com/max-concurrent-connections-limit-windows/),
[macOS user account limits (Twocanoes)](https://twocanoes.com/knowledge-base/how-many-users-on-macos/).

## What this does not decide

How a login service hands out per-principal channels, and the login service itself, are
milestone 49's own build, not decided here. This settles only which shape attribution takes once
that exists.

## What it unblocks

Milestone 49's own gate is now clear: nothing further blocks starting the login service or the
channel-shaped attribution logging. Milestone 152 (durable delegation), gated on 49, inherits a
settled answer to how attribution composes with its own design.
