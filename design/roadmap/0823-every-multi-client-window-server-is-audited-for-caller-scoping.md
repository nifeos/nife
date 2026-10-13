---
status: NOT-STARTED
raised: 2026-10-07
milestone_dependencies: none
decision_dependencies: 256
machine_requirements: none
specific_machine: none
needs_person: no
---
# 823. Every multi-client window server is audited for caller scoping

*(The title and slug are provisional.)*

## In brief

Some servers map pages from more than one client. Read each against §256 (a server that keeps
windows for many clients scopes each by the caller's badge). A server that passes finds a client's
window by the badge a request arrived on. One that fails gets a fix and a booted test that a second
client cannot reach the first's page. The test goes red under a replayable falsification.

## Why

Milestone 800's fourth outsider pass booted a capture in `net_stack`. A second client named the
first's window by a number it chose. The kernel's gates never saw it, so no kernel test catches the
next server built the same way. calef ruled on 2026-10-07 (UTC) that per-caller scoping binds every
such server. The audit reads the rest, `system_log`'s reader windows first.

## What is already read

Milestone 649 (every client of a network stack shares its socket numbers)'s lane read three. §255
(each socket is its own capability) records them. `system_log`, the file service and `name_resolver`
scope by badge. `net_stack` did not, and #1817 fixed it.

## What is left

A grep at minting found servers that receive client pages and mention windows but not badges. They
are candidates, not findings:

- `components/src/compositor.rs` and `components/src/console.rs`.
- `components/src/keyboard_driver.rs` and `components/src/usb_keyboard_driver.rs`.
- The swap demonstrator in `crates/swap_protocol`. Its `log_put` offset is bounded since milestone
  633's third pass, which is not the same as scoped.
- `components/src/login.rs` mentions both windows and badges, and wants a read.

A server with one client by construction is out of scope, and the audit says so per server.

## Done when

- §256's audit section lists every server that receives a client's page. Each verdict is one of
  scoped by badge, one client by construction, or fixed.
- Each fix has a booted test on every ISA that runs the server, red under a replayable falsification.

## BUGS, expected

- The candidate list came from a grep for `receive_cap`, "window" and "badge". A server that names
  its windows another way would not appear. The audit starts from who receives capabilities.

Reuse: `net_confinement_tests`'s squatter shape and `name_resolver`'s badge-keyed windows. Nothing
outside the tree applies, because the rule is about this tree's own servers.

## Index row

Every server that keeps windows for many clients is read against §256's caller-scoping rule. One
that names a window by a client-chosen number gets a fix with a booted, falsifiable test.
