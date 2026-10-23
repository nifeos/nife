# The kernel's compositor wiring

This appendix to [`notes/compositor.md`](../compositor.md) carries the design behind
`kernel/src/user/compositor_service.rs`: what the kernel hands the compositor and each client, and
why the test wiring is shaped the way it is.

## The kernel as witness

The kernel allocates every frame in a scene, so it can read each one through the direct map without
asking any process. A test checks a client's surface, the composed screen and the published window
list against values the kernel computed itself, not against numbers a process reported. The kernel
also holds one report endpoint per client. As the spawner it may know who is speaking, which is
exactly the identity the compositor does not have.

## One run for every client

Every client's control page and surface come from one contiguous allocation. That puts a client's
neighbor's pixels in the frames physically after its own grant, so the neighbor attack aims at
memory that really is there. An attack on a scattered allocation would prove only that the attacker
could not find its neighbor. `neighbour_probe_va` names the address two pages past the end of a
client's surface, past the neighbor's control page, so the test can assert on the exact faulting
address.

## The screen is one capability

The compositor and a capture client receive the screen as one `PageFrame` capability over its whole
run (§102 (a Frame names a run of pages)), and each maps it where it chooses. As one-page
`Spawn::maps` entries, the 683-page screen would overflow the 1 KiB a spawn closure may capture
(`kernel/src/thread.rs`). The window list, the input ring and each client's pages stay spawn-time
mappings, since they are few and small.

## Capture is the spawner's decision

Only `ROLE_CAPTURE` gets the screen (slot 3) and a map budget (slot 4). They are placed with
`grant_at`, because a non-focusable client's slot 2 must stay empty and a sequential fill cannot
skip it. The kernel decides from `role`. A hostile `window` binary that ignored its own role still
could not read the screen unless the spawner granted it, which is what the `ROLE_PROBE_SCREEN` test
checks. The grant is `READ` only: a program that may look at the screen may not draw on it, and
that is the difference between a screenshot tool and a second compositor. `fixtures/src/window.rs`
proves the write faults.

## A terminal is a window client

`spawn_terminal` gives `display_terminal` exactly a window client's authority: a report endpoint,
the doorbell, an input endpoint, and its control page and surface. The compositor cannot tell it from
the client that paints a coordinate pattern. Its one addition is an output page, which belongs to the
terminal contract. Its input endpoint is the endpoint it serves, because the process has one wait
point, so an application printing and the compositor typing arrive on one endpoint and are told
apart by opcode, as `line_editor` does for the serial terminal.

## The kernel as input driver

`type_bytes` writes the ring and then the tail that advertises the bytes, with a fence between.
`drain_input` in `components/src/compositor.rs` is the single reader for every writer of the ring,
the kernel stand-in included, so its fence covers them all. The doorbell `ipc_call` also orders this
producer on its own, since the compositor is blocked in `receive_cap` on the doorbell.
[`notes/memory-ordering.md`](../memory-ordering.md) has which producer relies on what.
