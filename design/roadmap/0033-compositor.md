---
status: BUILT
raised: 2026-07-29
built: 2026-07-29
---
# 33. A compositor: one screen, mutually distrusting clients

**In brief.** Built (2026-07-29), both ISAs, rung two of the display ladder: `compositor`
multiplexing one screen among three clients, each holding a capability to its own surface. Software
composition honouring a damage rectangle; input routed by capability over the terminal contract's
`OP_BYTES`; enumeration and screenshots as read-only mappings rather than verbs. No new system call and
no new method. notes/compositor.md, DECISIONS §33 (the compositor's authority is memory, not messages)

Why it matters. the canonical multiplexer of one device among distrusting clients, and the thesis at
its sharpest. A client is *proved* unable to reach its neighbor's pixels even when handed the exact
address of them. The compositor holds no authorization code because the authority is a mapping
rather than a message. It also found the kernel's one missing primitive (no wait-any), recorded as a fork

## Follow-on

- **Milestone 151.** The kernel's one missing primitive this milestone found: a component that must
  distinguish more than one class of sender has exactly one blocking wait point, so it must be more
  than one process or carry authority outside its messages. DECISIONS §101 answered it with
  notification objects rather than wait-any, and 151 is that kernel build.
- **Decision.** The fork itself, with both candidate shapes and what each would buy the compositor,
  is written up in
  `design/decisions/0033-compositor-authority.md`. The buys are per-client endpoints for unforgeable
  identity, a served screenshot instead of a tearing read-only mapping, and input delivery that is
  not a blocking `CALL`.

## Index row

the canonical multiplexer of one device among mutually distrusting clients
