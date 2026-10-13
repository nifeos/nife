---
status: DECIDED
raised: 2026-10-07
decided: 2026-10-07
ratified_by: calef
---

# 256. A server that keeps windows for many clients scopes each by the caller's badge

*Raised at PR #1798, milestone 800 (a non-Anthropic model attacks the confinement claim).*

## The rulings

Two rulings, both calef's, both made on PR #1798 on 2026-10-07 (UTC).

The first, about 03:50Z, was the fix shape for the network stack's socket capture. That pass had
booted the capture on all three ISAs. The shape: per-caller windows keyed by the caller's badge,
`name_resolver`'s exact shape, refusals silent, nothing folded in. Whether refusals should also
become answerable was left open as a separate, smaller call about observability.

The second, about 15:03Z, is the lane's decision 2, option (i). Per-caller scoping is a written
rule for every multi-client window server. The fix lane audits the remaining ones, `system_log`'s
reader windows first.

At 15:04Z calef superseded the first ruling for `net_stack` alone: each socket is its own
capability, §255 (each socket is its own capability). That replaced how the stack names a socket. It
did not withdraw the second ruling, which is about every other server, and this section records it.

## The rule

Some servers map pages from more than one client and later read or write them for a client. Such a
server finds a request's page by the badge on the capability the request arrived on. The kernel
stamps that badge. The server never finds the page by an index, an offset or an id the client wrote
in the request word.

- The badge is the scope. A client can name only windows registered under its own badge, so it
  cannot reach another client's page by guessing a number.
- A second registration at a window already taken is refused, not overwritten. The refusal may be
  silent, as `name_resolver`'s is. Under the rule every failed registration is the caller's own, so
  silence hides nothing from anyone else.
- A client that should be able to hand its window on needs §255's stronger form: the window, or the
  object it belongs to, is a capability of its own.

## What it rules out

- A window table indexed by a client-chosen integer. This was `net_stack`'s defect, milestone 649
  (every client of a network stack shares its socket numbers).
- An offset in a request word that the server adds to a base without bounding it. This was the swap
  demonstrator's deputy, found by milestone 633 (an outside agent attacks the confinement claim)'s
  third pass and fixed in `swap_protocol::log_put`. Bounding the offset closes the reach; it does not
  scope the page to a caller, so the audit below reads it again.

## Prior art

- In this tree: `name_resolver` keys its windows by the badge's grant index (§252 (a resolver grant
  is one zone per client badge)). The file service binds a directory per badge (§230 (badged
  endpoint capabilities)). `system_log`'s reader windows are registered per badge by the spawner.
- seL4's badged endpoints exist so a server can tell its clients apart without trusting what they
  write (seL4 reference manual, recalled).

## The audit

Read by milestone 649's lane and recorded in §255: `system_log`, the file service and
`name_resolver` already scope by badge, and `net_stack` was the one that did not. The rest is
milestone 823 (every multi-client window server is audited for caller scoping). At the
merge a grep found servers that receive client pages and mention windows but not badges. None has
been read against this rule: `compositor`, `console`, the two keyboard drivers and the swap
demonstrator.

## Refused

- Leaving it as `net_stack`'s fix alone. The defect class is a server's bookkeeping, and the kernel's
  gates never see it, so no kernel test catches the next instance. A written rule is what the next
  server's author reads.
- Answerable refusals as part of the rule. calef kept them a separate call, still open. They help a
  client see why it failed and add nothing to the security property.
