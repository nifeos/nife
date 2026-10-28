---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-28
ratified_by: calef
---

# 161. Which subset counts as running Vaultwarden

Ruled 2026-10-28 (UTC): Definition of done: Vaultwarden running on a board or host at a Tailscale
address. A phone on the same tailnet runs the Bitwarden client, connects over Tailscale to a real
certificate (Let's Encrypt, DNS-01 challenge, no open port), logs in, syncs its vault, writes an entry
back and downloads an attachment. It receives a live update when another client changes an item.
TLS, concurrency and the async runtime are therefore required; SQLite and threads are already built.

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s lane, which found milestone 66 (somebody else's real application, running here) gated on
`DECISION` with no decision anywhere a reader can open. The ask is the block's own and has been
written in its `BUGS` section since it was filed: *"'Runs Vaultwarden' is not one bit... Which
subset counts should be decided before the work starts, or the goalposts will move to wherever the
effort lands."* *(Section number provisional until the merge queue lands it.)*

## What is being decided

The definition of done for the largest single item on this roadmap, before any of it is built.
Not the sequence, not the estimate: the sentence that gets published when it works.

## Why this is a decision and not a scoping note

It is a fact that leaves the machine. AGENTS.md's irreversible category names exactly this:
*"A published claim, a benchmark number a stranger quotes."* "nife runs Vaultwarden" is a sentence
other people will repeat, and it cannot be narrowed afterwards without looking like a retraction.
Deciding it late means deciding it from whatever the effort produced, which is the failure the
block predicted in its own words.

And this tree has already been bitten by the milder version. Milestone 66's own gap table said TCP
listen and accept were missing for three weeks after the gate line had been corrected to say they
were not. That happened because a status was fixed where somebody looked and left wrong where they
did not. A definition of done nobody wrote is the same defect with nothing to correct against.

## What the tree already does in the analogous case

**Benchmarks and comparisons here carry their caveat in the claim itself.** AGENTS.md: *"State what
each number means and where it is not apples-to-apples: the map 'tie' (zeroing-bound) and the spawn
'lighter object than a Unix process' caveats are the standard."* So the precedent is not to pick a
generous definition and footnote it; it is to write the claim with its qualification attached, and
an honest tie recorded plainly is worth more than an overclaimed win.

Milestone 123's demonstration shape is the other precedent: somebody else's software running
narrow, with a negative control. Under it, "it started" is never the claim; "it ran confined and
was refused when it reached further" is.

## Whether the premise is true, measured 2026-09-19

The block's gap table is the measurement and one row of it has already moved once:

- TCP listen and accept: built. `OP_LISTEN` and `OP_ACCEPT` have been on the wire since
  milestone 107 and are bound into the `std` PAL by milestone 64, under
  [§25](0025-socket-identity.md) (a socket id in phase one). The remaining limit is **concurrency,
  not the contract**: the backlog is one connection deep.
- `std::thread`: 4 of 6 PAL functions answer `Unsupported`.
- `std::fs`: 32 of 54 answer `Unsupported`.
- async runtime: none. Rocket wants tokio: timers, wakers, a reactor.
- TLS: none. The `rustls`-versus-confined-OpenSSL fork is milestone 387's, and server-side TLS
  is this block's.
- SQLite: a C library, so the [§31](0031-foreign-language-seam.md) seam plus real filesystem
  locking.

So the honest statement of the distance is that every row is milestone-sized on its own.

## The options

| | the claim | what it requires | what it concedes |
|---|---|---|---|
| **A** | **It serves one client over plain HTTP, SQLite on a real filesystem, single-threaded.** | `std::fs` deep enough for SQLite, the C seam, one connection at a time. | Nobody would run it. The claim needs "single-connection, no TLS" attached every time it is said. |
| **B** | **It serves concurrent clients over plain HTTP behind a TLS terminator.** | A concurrency model (userspace threads or a select-shaped wait) and an async runtime. | The TLS story is somebody else's box, which is exactly how most people deploy Vaultwarden, so the concession is smaller than it reads. |
| **C** | **It serves concurrent clients over TLS that nife terminates.** | Everything in B, plus milestone 387's fork answered and a crypto surface this tree does not have. | Nothing. It is also the furthest away by a wide margin. |
| **D** | **It runs confined, and what it cannot reach is the result.** | Grant it a directory and a listening socket and record what it asks for next. | It is not a "runs it" claim at all. It is a different and more interesting experiment, which the block itself says. |

Recommendation: B as the claim, with D as the deliverable that comes first. B is the smallest
definition a person could actually use, which is the only bar that is not chosen for convenience,
and it puts the TLS fork where it already lives (milestone 387) instead of importing it. D costs
almost nothing once the program starts at all, produces the finding this project exists to produce,
and gives a publishable result on the way to B rather than only at the end.

The honest note on effort, in the words AGENTS.md asks for: A is cheaper than B and is not
recommended, so cost is not deciding this. C is rejected on distance rather than on merit, and that
*is* an effort argument, stated so it can be weighed as one.

## How reversible it is

**The claim is not; the sequence is.** Changing which milestone comes first costs a briefing.
Changing "nife runs Vaultwarden" after a stranger has quoted it costs credibility, which is the
asset the benchmark discipline exists to protect.

## What is blocked until this is answered

Milestone 66's scoping, which is its first honest deliverable. Nothing is blocked on it today,
because 66 also waits on milestone 64 and every row above is unscheduled. That makes this a good
decision to take early and a cheap one to take late, which is the argument for taking it now while
nobody is mid-effort and the answer cannot be shaped by sunk cost.
