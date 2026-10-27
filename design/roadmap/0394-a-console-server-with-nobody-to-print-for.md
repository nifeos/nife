---
status: SUPERSEDED
raised: 2026-09-13
superseded_by: 890
milestone_dependencies: none
decision_dependencies: 179
machine_requirements: none
specific_machine: none
needs_person: no
---
# 394. The boot-time console server comes up with no client and nothing notices

Overtaken by commit `bcf864dc7` (2026-10-20, milestone 890 (riscv64 and x86_64 join the shared boot path)): under the ruling on #2447 the tour boots
went, and `kernel/src/user/console_service.rs` with them, so no console server starts without a
client. §179 records the outcome. The text below is the block as filed.

Filed 2026-09-13 as an unnumbered proposal, left behind by
`maintainer/delete-narrator`, which deleted the narrator on calef's ruling and left the server that
existed to print for it running; numbered 2026-09-19 by milestone 433's drain of the proposal pile.
Premise re-read against the tree on 2026-09-19 and still true: the tour arm of
`kernel/src/main.rs` still runs `user::initial_ramdisk().map(|_| user::console_service::start())` (line 1768
now, 1676 when this was written). `kernel/src/user/console_service.rs` still carries the
`#[expect(dead_code, reason = "no client since the narrator went; see milestone 267's block")]` on
`Console`, whose own doc comment says the `expect` is this question made visible and should be
deleted when the question is answered. *(Number provisional until the merge queue lands it.)*

The decision is
§179 (whether the tour boot keeps starting a console server that has no client). It was written up
2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote
down)'s slice-c lane because this gate named no section and the ask lived only
in the prose below, addressed to one person. calef's, and specifically because the answer is not
"delete it": this is infrastructure rather than a demonstration, and the lane that found it was told
to report rather than decide. Nothing is blocked meanwhile; the boot is correct either way.

One record item that lane found and did not fix, because it is downstream of the answer: the
`#[expect(dead_code)]`'s `reason` string names *milestone 267's block*, which forwards here, so the
code is two hops from the question rather than one. When this is answered the `expect` goes, and
§179 is what its replacement should cite if anything survives it.

In brief. On a tour boot (no `shell`, no `initboot`), `kernel/src/main.rs:1676` still runs
`user::initial_ramdisk().map(|_| user::console_service::start())`. That spawns `components/src/console.rs`, a real
UART driver at EL0 holding the PL011's registers, which blocks on `receive(REQUEST)` forever because
nothing in the boot holds a capability naming its endpoint. Its only client was the narrator.

The compiler agrees, which is why this is not a reading. With `spawn_client` gone, every field
of `console_service::Console` (`request`, `reply`, `shared_phys`) is written by `start` and read by
nobody; `rustc` reports *"fields `request`, `reply`, and `shared_phys` are never read"*. The handle
`start` returns is the wiring a client would need, and there is no client. The `#[expect(dead_code)]`
on that struct is this proposal made visible in the code, and it should be deleted when this is
answered rather than left as the answer.

## What it costs today

Not correctness, and saying so plainly matters because the cheap read is that this is a leak. It is
not. The server blocks in a rendezvous rather than spinning, so it burns no CPU; `no_leaked_threads`
does not police it because the tour boot is not a test boot.

What it does cost is real but small. One spawned process with its own address space, one zeroed
frame for a shared page nobody writes, two rendezvous objects, and a second mapping of the UART's
registers handed to a program that will never use them. That last one is the item worth a
decision. This tree's whole argument is that authority is granted deliberately; a device mapping
that exists because of a program deleted four days earlier is the opposite of deliberate, whatever
its runtime cost.

## The options, with what each one actually changes

**Leave it.** Zero work, and it keeps `console.rs` exercised on the default boot in the weak sense
that it is loaded, relocated and entered. The honest version of this option is that the exercise is
worth very little: nothing checks the server did anything, which is the same criticism that deleted
the narrator.

**Stop starting it on the tour boot.** Delete the `map` line and let `console_service::start` and
its module go with it. `components/src/console.rs` stays, because it has consumers that have nothing
to do with this path: `crates/system_initializer` loads it (`lib.rs:744`), `fixtures/src/hello.rs:571`
builds it as init's print server, `measured_boot_tests.rs` measures it, and `swapper.rs` names it in
a dependency check. The interactive boot reaches it through `boot_via_progenitor`
(`kernel/src/main.rs:1904`), never through `console_service`. So this option deletes a kernel module
and one boot line, and deletes no program.

**Give it a client again.** Only if something is actually wanted on the tour boot that has to print
from EL0 through a driver at EL0. Nothing is today. Listed because it is the option that says the
server was infrastructure rather than scaffolding, and somebody should say that out loud if it is
true rather than letting the answer be decided by which is less work.

## Why the lane did not pick one

Two reasons, and the first is the binding one. It was not authorized: calef ruled the narrator
deleted, and `console_service::start` plus `components/src/console.rs` are not the narrator. Deleting
infrastructure on the momentum of a demonstration's deletion is exactly the sweep AGENTS.md's blind-
`sed` scar is about.

**And the second option touches a boot path.** `kernel/src/main.rs`'s tour block is the file
milestone 266 landed first to make safe to open, and the three boot arms diverge there. That is not
a fork a lane should take on its own initiative on the strength of a `dead_code` warning.

## Index row

On a tour boot the kernel still spawns `components/src/console.rs`, a real UART driver at EL0
holding the PL011's registers, which blocks on `receive(REQUEST)` for ever because nothing in that boot
holds a capability naming its endpoint. Its only client was the narrator, deleted on 2026-09-13. The
compiler agrees rather than a reader inferring it, since every field of `console_service::Console`
is written by `start` and read by nobody, and the `#[expect(dead_code)]` on that struct is this
question made visible in the code. It costs no correctness and no CPU, because the server blocks in
a rendezvous, and saying that plainly matters, because the cheap read is that this is a leak. What
it does cost is one process with its own address space, one zeroed frame nobody writes, two
rendezvous objects, and a second mapping of the UART's registers handed to a program that will never
use them. That is the item worth a decision. This tree's whole argument is that authority is
granted deliberately, and a device mapping that exists because of a program deleted four days
earlier is the opposite of deliberate. Three options, each with what it actually changes. Leave it,
and accept that the exercise is worth very little. Stop starting it on the tour boot, which deletes
a kernel module and one boot line and deletes no program, because `components/src/console.rs` has
four consumers that have nothing to do with this path. Or give it a client again, which is the
option that says the server was infrastructure rather than scaffolding and wants somebody to say so
out loud. It is an architect's because it is not the narrator, and because the second option touches
the boot path where the three arms diverge.
