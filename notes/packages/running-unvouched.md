# Running bytes nobody vouched for: §219's gate D2

*Built 2026-09-26 (UTC) by milestone 198 (a package manager) rung 3a's D2 lane. Names provisional.
An appendix to [packages.md](../packages.md).*

DECISIONS §219 (how the shell names an installed program to the spawner) ruled that running
unvouched bytes takes a capability the owner gives a session. An unvouched child gets what the
caller delegated, plus the read-only clock and configuration pages. Never the process domain,
the network, entropy, the file service, or anything else the progenitor holds.

## The shape, and why it is not a token

§219's limitation 2 found the constraint in the tree. The capability is granted without `GRANT`,
so its holder cannot pass it on, and the kernel refuses to `SEND_CAPABILITY` a capability that lacks
`GRANT`. So it cannot ride on a spawn request. What a holder of a `WRITE`-only endpoint can do is
send on it.

So D2 is one rendezvous, the run-unvouched endpoint. The progenitor holds the only `READ` on it.
A session holds `WRITE` at slot 62 (`grant_plan::spawnproto::RUN_UNVOUCHED_SLOT`, just below the
reserved fault slot). An image request that claims it sets `RUN_UNVOUCHED_BIT` and sends one word
on that endpoint as its last message. The progenitor takes that word with a `RECEIVE` while serving
the request. Only a holder can send there, so arriving is the proof.

The kernel is untouched. There is no new syscall and no new method; the bit is on a userspace word
and the proof is an ordinary `SEND`.

Considered and refused:

- A capability attached to the spawn request. The kernel refuses it without `GRANT`, and granting
  `GRANT` lets the session lend it, which §219 recommends against.
- A second spawn endpoint that runs misses, served beside the first. The progenitor has one
  thread and no way to wait on two endpoints, and badges are a kernel change.
- Sending the word before the request. The progenitor receives on the run-unvouched endpoint only
  while serving a claiming request, so an early word would be taken by whichever claim came next,
  possibly another session's.

## Who holds it

| Holder | Rights | Placed by |
|---|---|---|
| the progenitor | full (it retyped it), receives on it | itself, after `login` is built |
| `login` | `WRITE`, `GRANT` at slot 62 | the progenitor, into `login`'s unstarted thread |
| a session `login` builds for a listed identity | `WRITE`, the sixth capability of an `OK` | `login` (`login_contract::RUN_UNVOUCHED_FOLLOWS`) |
| the boot prompt | `WRITE` at slot 62 | the progenitor, one call |

The boot prompt's grant is one call in `crates/system_initializer`. calef ruled on 2026-09-26 that
the boot prompt is the owner's console and keeps it (§221 (the boot prompt is the owner's
console)). Deleting that call makes the gate refuse as before, and that was run (below). A `login`
session gets it only when the owner lists its identity ([vouching.md](vouching.md)).

The progenitor retypes the run-unvouched endpoint after `login`'s build, not before, because that
build is where the progenitor's table peaks. Held across the peak, the endpoint would have taken the
last free slot. Measured by `script/swish-check` on aarch64 and riscv64: 23 of 24 before and after.
The x86_64 leg's gauge reads the hand-over, not the peak, so it says nothing here.

## What an unvouched child holds

`grant_plan::UNVOUCHED_MANIFEST` is the ruling as one value, with a host test saying so. For a
native child: slot 0 its output, slot 1 the clock page, slot 2 the configuration page. The shell
binds an image line against `uptime`'s manifest before it knows the verdict, so the line can
delegate the output and nothing else yet.

`caps <path>` previews it. The shell hashes the file as the progenitor will and looks the digest
up in the live generation:

```
$ caps installed/unvouched
  installed/unvouched would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 1  page      clock    read-only; unvouched bytes may read the time
    cap 2  page      config   read-only; and the configuration page
    provenance: unvouched (digest sha256:82f73b2e...)
    runs on this session's capability to run unvouched bytes (slot 62)
```

A vouched file prints `provenance: vouched by activation generation N`. A session without D2 is
told the run would be refused.

## Milestone 202's claim, and breaking it

The claim milestone 202 (every confinement test is a ritual until somebody breaks the confinement)
was owed: an unvouched child holds no capability the caller did not delegate, beyond the two
pages. The fixture is `installed/unvouched`, `unreachable_network_witness` stripped so no table
vouches for it. It asks the process domain, entropy and the network with real requests, then lists
every slot the kernel says it holds. `script/swish-check` wants three refusals and `slots held: 0 1 2`.

Each break was run on aarch64 before the test was committed, and each turned the line red for its
own reason:

| Break in `spawn_service` | What the child reported |
|---|---|
| endow the process domain to an unvouched child | `domain: REACHED`, `slots held: 0 1 2 7` |
| endow entropy | `entropy: REACHED`, `slots held: 0 1 2 9` |
| endow the network | `network: REACHED`, `slots held: 0 1 2 10` |
| remove the boot prompt's grant | `caps` said it would not run; the run was refused |

`login`'s half is a kernel test, `login_tests::login_hands_a_listed_session_the_run_unvouched_capability_and_it_cannot_be_passed_on`.
A session's copy reaches the endpoint `login` was given, and its `SEND_CAPABILITY` is refused. Delegating
it with `GRANT` turned that test red: the client's delegation arrived in place of its report.

## BUGS

- A session that sets the bit without holding the capability hangs the progenitor, which waits for
  a word nobody can send. Every promised message in the spawn protocol has this exposure
  (`spawnproto`'s BUGS). Today only the boot prompt holds the spawn endpoint.
- A holder can send its word early on purpose and let another session's claim through. That is a
  proxy, which no capability system prevents.
- Only one session exercises D2. No session `login` builds holds a spawn endpoint, so a listed
  session's sixth capability is delivered and proven, not used.
- The refused path has no permanent gate. The boot prompt holds D2, so no line at the prompt can be
  refused for want of it; the removal falsification above is the only run of it.
- A removed program is not unrunnable at a prompt that holds D2. It loses its vouch and runs with
  the unvouched endowment. `caps` shows which.
- The census counts capabilities, not mappings. A page mapped into the child with no capability
  behind it would not appear in `slots held`.
- An unvouched line cannot carry `--mem`, an input or a directory, because the shell binds it as
  `uptime` before it knows the verdict.
