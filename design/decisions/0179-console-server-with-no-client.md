---
status: PROPOSED
raised: 2026-09-19
---

# 179. Whether the tour boot keeps starting a console server that has no client

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 394 (the boot-time console server comes up with no client and nothing notices)'s
`DECISION` gate naming no section. The ask lived only inside that block, and one rung further out:
`kernel/src/user/console_service.rs`'s `#[expect(dead_code)]` reason names *milestone 267's block*,
which forwards to 394, which addresses a paragraph to one person. *(Section number provisional until
the merge queue lands it.)*

## What is being decided

On a tour boot, `kernel/src/main.rs:1881` runs
`user::initial_ramdisk().map(|_| user::console_service::start())`. That spawns `components/src/console.rs`, a
real UART driver at EL0 holding the PL011's registers, which then blocks on `recv(REQUEST)` for
ever, because nothing on that boot holds a capability naming its endpoint. Its only client was the
narrator, deleted 2026-09-13 on calef's ruling (milestone 267).

One question: does that line stay, go, or get a client?

## Is the premise true

Yes, on both halves. *(Line citations in this section are against `main` at `508333ebe`.)*

- `kernel/src/main.rs:1881` still carries the `map` line, in the tour arm.
- `console_service.rs:20` still carries `#[expect(dead_code, reason = "no client since the narrator
  went; see milestone 267's block")]` on `Console`, whose three fields `start` writes and nobody
  reads.

The doc comment above that attribute says in its own words that the `expect` is this question made
visible and should be deleted when the question is answered. So the compiler is holding the
question open, which is why this is not a reading somebody could dispute.

## What this tree already does in the analogous case

Two analogues, and they point opposite ways, which is the reason this is worth a section rather
than a sweep.

Authority granted to a program that cannot use it gets taken back. §41 is the standing shape: a
device is revoked by taking the endpoint back, and §13 and §16 reclaim what a dead builder held. The
tree's whole argument is that authority is granted deliberately.

**Infrastructure is not deleted on a demonstration's momentum.** §85 draws the line between what is
evidence and what is product, and AGENTS.md's blind-`sed` scar is the standing warning about sweeps
that carry further than the ruling behind them. `components/src/console.rs` is not the narrator: it
has four consumers that have nothing to do with this path (`crates/system_initializer/src/lib.rs:868`
loads it by name, `fixtures/src/hello.rs:303` builds it as init's print server,
`measured_boot_tests.rs` measures it, `components/src/swapper.rs:252` names it in a dependency
check), and the interactive boot reaches it through `boot_via_progenitor`, never through
`console_service`.

## The options, and what each one actually changes

| | what | what it changes |
|---|---|---|
| **A** | leave it | nothing. The `expect` stays, and it keeps saying a question is open that has been answered by not answering it. |
| **B** | stop starting it on the tour boot | deletes one line in `main.rs`'s tour arm and the `console_service` module with it. Deletes **no program**: `components/src/console.rs` keeps its four consumers and the interactive boot. |
| **C** | give it a client again | says out loud that the server is infrastructure on this boot rather than scaffolding, which nothing currently wants. |

## What each option costs, measured rather than asserted

It costs no correctness and no CPU today, and saying that plainly matters, because the cheap
read is that this is a leak and it is not. The server blocks in a rendezvous rather than spinning,
and `no_leaked_threads` does not police it because the tour boot is not a test boot.

What it does cost is one spawned process with its own address space, one zeroed frame for a shared
page nobody writes, two rendezvous objects, and **a second mapping of the UART's registers handed to
a program that will never use them**. That last item is the one worth a decision, and it is the
tree's own argument turned on itself: a device mapping that exists because of a program deleted six
days earlier is the opposite of deliberate, whatever its runtime cost.

B is minutes of work. C is unbounded, because nothing has named what would print.

## Recommendation

B, and the reason is the mapping rather than the line count. §41's posture is that authority is
handed over for a reason and taken back when the reason goes; A keeps a device grant alive for a
reason that no longer exists, and calls it exercise. The honest version of A's "it keeps `console.rs`
exercised" is that nothing checks the server did anything, which is the same criticism that deleted
the narrator.

C is the option that should be chosen deliberately if it is true, rather than allowed to lose by
default: somebody should say the server was infrastructure on this boot, and then say what it
prints. Nobody has.

## Would we still choose B if both cost the same

Yes. B is also the cheaper option and that is worth stating in those words, but the argument does
not rest on it: A costs nothing to build and is still the option this tree's own vocabulary
refuses, because it leaves a grant standing with no grantee.

## How reversible, and who has acted on it

High, and nobody outside this tree has acted. B is one line and one kernel module; re-adding the
call is the same edit backwards. Nothing two programs agree on changes, no wire format, no syscall.
This is the category AGENTS.md says to decide quickly.

## What is blocked until this is answered

Nothing. The boot is correct under any of the three. What is owed is the `#[expect(dead_code)]`,
which its own doc comment says to delete when this is answered, and whose `reason` string should
then cite this section rather than milestone 267's block.
