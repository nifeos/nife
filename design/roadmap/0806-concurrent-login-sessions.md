---
status: NOT-STARTED
raised: 2026-10-06
promoted_from: concurrent-login-sessions
milestone_dependencies: none
decision_dependencies: 221, 253
machine_requirements: none
specific_machine: none
needs_person: no
---
# 806. Concurrent login sessions: more than one person on the machine at once

*(All six forks ruled by calef on 2026-10-06 (UTC), on #1769. §253 (concurrent login sessions: a
greeter per terminal, authority granted at login, memory from a broker) records each ruling in his
words with the refused options. The forks below keep the argument; their recommendations are now
the rulings. The §221 amendment they build on is #1776.)*

calef asked for this on 2026-10-06 (UTC), in a maintainer session, and later the same day added why:
*"it seems like we have programs that not every user on a multiuser system should be able to run.
which means we're possibly looking at some sort of sudo escalation mechanism."* A writing-only lane
wrote it and built nothing.

Every name here is provisional: `greeter`, `console_multiplexer`, `grant_broker`, `memory_broker`,
the policy files and the gate.

Milestone 481 (real terminal multiplexing) points here. Milestone 49 (users, login, and
attribution) refused it with one condition, "somebody needing two sessions at once", and calef's
request meets it.

## Why, and its honest rank

Against principle 1: the customer path is vacant, and none of the candidates in milestone 530 (name
a customer) needs two operators. Remote login ranks higher than concurrency does. A rented machine
(§203 (capacity is rented rather than bought)) has no console a person can reach, so even its one
operator needs a network login. calef also ruled no second customer before milestones 801 and 802.

Against the fatal risks: no verdict moves. User against user is a confinement boundary missing from
`notes/confinement-claims.md`, so this enlarges risk 7 (the confinement claim is false) rather than
greening it.

## What the tree has, read on 2026-10-06 (UTC)

No person can log in on a real boot today. `crates/system_initializer` builds `login`, then deletes
its copies of the front door, and the shell "holds no capability to `login`'s front door"
(`components/src/swish.rs`). The only clients are `kernel::user::login_tests`.

A login session cannot run a program: it gets no shell and no spawn endpoint. `grant_plan::spawnproto`'s
BUGS says why: every activation verb (`install`,
`remove`, `rollback`, `vouch`) is open to whoever holds the spawn endpoint. "A session given a spawn
endpoint would be the owner too."

`login` blames its stuck-terminal bug on "no wait-any primitive", but notification objects (§101
(notification objects: async multiplexing without wait-any)) have since landed.

Reuse: `login`, the credential service, `fs_subtree_caretaker`, the system log and the spawn
presentation are the tree's. The greeter and multiplexer are new glue over them;
`getty` and `screen` lend a shape, not code. Remote login reuses `sunset` (fork 6).

## The forks

Six, all ruled. The first five shape the first slice; the sixth is a follow-on.

### Fork 1: What a session is, and who mints its terminal

**Ruled A**, calef, 2026-10-06 (UTC): "A on Fork 1". One `greeter` per terminal owns that terminal.
It reads an identity and secret, asks `login`, builds a shell from what `login` returns, supervises
it, and logs out when it dies. `login` holds no terminal, and milestone 49's single-session rule is
retired rather than widened.

This is Unix's `getty` and `login` split (from memory): the authenticator never owns the line.
Liveness falls out, because the greeter receives the shell's death on the fault endpoint (§26 (the
fault endpoint)), which fixes `login`'s stuck-terminal bug. The cost is one program and a
`login_contract` change that drops the terminal from the reply.

### Fork 2: Which programs a user may run, and how a user gets more

**Ruled B now, D as a follow-on**, calef, 2026-10-06 (UTC): "B now, D as a follow-on".

Running a binary confers nothing here; a program can do what it is granted at spawn (§208
(installing is granting)). `reboot` (#1766, a reset object) is the live example. So "who may run
`reboot`" is two questions: which capabilities a session gets at login, and how it obtains one it
was not given. The tree answers the first once already: `may-run-unvouched`, an owner-written list
`login` reads at every login (§221 (the boot prompt is the owner's console) ruling 2). It is out of
a session's reach because a session is confined to its subtree (§117 (a principal's subtree is named
by its identity)).

| option | what it is | verdict |
|---|---|---|
| A | setuid or `sudo`: a binary carries authority to whoever runs it | refused |
| B | Login-time only. To do more, log in again as an identity the policy grants more. | ruled, first slice |
| C | A `grant_broker` holds privileged capabilities and hands a session an attenuated, time-limited or single-use copy after re-authentication | refused |
| D | The broker spawns one job with the extra endowment after re-authentication and a policy check. The session never holds it. | ruled, follow-on |

A is ambient and binary-scoped: whoever names the binary gets its authority, the confused deputy
milestone 49 names in setuid. B generalizes what the tree does now: each privileged authority gets an
owner-written list at the file-service root, and `login` delivers it only to listed identities.
"Log in again as `admin` on another console" is `su` without the ambient part.

C fails because the kernel has no time-limited capability: expiry needs the broker to revoke on a
timer (§16 (object revocation)) or proxy every call, and a holder can copy a `GRANT`-able capability
first. D keeps §139 (who may read the cycle counter): authority is given at creation. The
broker re-authenticates through `login`'s credential path, checks the owner's policy, and spawns
one command with the extra endowment, which dies with the job. Every grant is one audit record.
This is polkit's shape without its root daemon.

### Fork 3: Where sessions come from first

**Ruled A, virtual terminals**, calef, 2026-10-06 (UTC). His intent, verbatim: "be able to switch
between sessions at a single keyboard. I believe early linux with text mode had this when hitting
the function keys to switch between different sessions. You would get a fresh login prompt, could
sign in, and that session would continue running and you could switch between sessions."

So: Alt+F*n* selects console *n*. Each console opens on a fresh greeter. A session on a console you
have switched away from keeps running, and its output is held for when you come back. On silicon
this needs milestone 242 (USB host and HID) for a keyboard that is not a UART; QEMU proves it first,
over the serial line, by the escape sequence a terminal sends for Alt+F*n*.

The risk is x86_64, whose console is kernel-resident (§121 (x86 port I/O)). The multiplexer has to
sit above whatever endpoint the shell reads there. Nobody has checked that it can, so it is the
first slice's step 0, before anything is built on it.

### Fork 4: Memory, CPU, and the owner

**Ruled D**, calef, 2026-10-06 (UTC): "D on Fork 4". Not a fixed per-session budget, because
"that likely means we're just limiting a single session to how much memory can be used since most
of the time there will be a single session."

What the kernel allows, read in `crates/abi` and `kernel/src/memory_region.rs`:

- A region cannot grow. `memory_region` has `MAP`, `RETYPE`, `RETYPE_OBJECT`, `SPLIT`, `DESTROY` and
  `USAGE`, and nothing that enlarges a region or shrinks one after `SPLIT`.
- A process can hold more than one region. Growth is therefore a second capability, split from the
  pool and delegated, with no kernel change.
- Regions are carved bump-only. A child freed out of order leaves a hole that returns only when every
  child above it is gone (§16's return-of-pages rule).
- The region table holds 256 regions, machine-wide (`MAX_REGIONS`).
- A running program cannot grow. Its region is fixed at spawn, and a `std` program's heap is
  `STD_REGION_PAGES` (384).

So a lone session cannot be handed everything but a reserve and shrunk later: with no shrink, a
second session would be refused. Growth comes in increments from the start.

The ruling: a `memory_broker` holds the pool left after the boot's carve. A session gets a small
first increment at login, and its shell asks for another when a split is refused. Increments grow
geometrically, so a large session uses few of the 256 slots. Interleaved growth fragments the pool,
and the gate measures how many pages that strands. The broker's request and reply are a wire format,
the expensive part.

calef added that the broker is designed so programs, not only sessions, can ask it for memory. A
growable program heap is a separate proposal, #1777 (lane/growable-heap-proposal). This slice
widens what a session can start, not what one running program can use.

The per-identity cap is an owner-written file at the root, fork 2's list pattern; no file, no cap.

The reserve, measured from the constants. The progenitor carves the boot shell's budget
(`SHELL_BUDGET_PAGES`, 1,152 pages) and the job pool (`JOBS_BUDGET_PAGES`, 672) at boot: 1,824
pages, 7.1 MiB, never `login`'s. `JOB_REGION_PAGES` (48) was bisected on x86_64 in
`script/swish-check`, so one owner job is known to fit. A revoke allocates nothing: `DESTROY`
returns pages, and §16's kill-then-retry handles a session still running. So the reserve needs no
new pages, only a broker handed what is left after the carve and an owner holding a revoke
capability on it. That reserve attaches to the owner's greeter and to the recovery shell.

CPU stays unisolated, with the delay measured (exit criterion 9).

The owner, under §221 as amended on 2026-10-06 (UTC) (#1776):

- No console is an unauthenticated owner shell. Console 1 shows a greeter like every other. The
  owner is an identity holding owner authority through fork 2's lists: the file-service root, and a
  spawn endpoint whose activation verbs are open.
- The one unauthenticated owner shell is a recovery boot, chosen on purpose. Today's boot shell is
  that shell already, so recovery costs a boot switch, not a program.
- The owner sets a password before any shell exists. `system_installer` asks at install. Otherwise
  any boot whose credential store holds no owner asks. That is state, not a first-boot flag.
- An unreadable credential store refuses to log anyone in and names the recovery boot.

The cost lands on every automated boot. `script/swish-check`, the boot checks, the QEMU suites and
the radon and xenon bench scripts type at a prompt that becomes a greeter. Test images need an owner
credential preset at build time, which must never reach a release image.

### Fork 5: Attribution and audit with two people at once

**Ruled**, calef, 2026-10-06 (UTC): "Yes on Fork 5, with the owner-credential events."

Today `login` sends one `ATTRIBUTED` record per login and `login_audit_receiver` throws it away.
Yet the system log (milestone 613 (a system log service), §242 (a system log)) already stamps each
record's `user` from the writer's badge and filters reads by it.

So `login`'s records and the greeter's logouts go to the system log, and each session's log writer
is badged with its identity. The owner-credential events are audited too: the owner password being
set, a recovery boot, and the refusal of an unreadable credential store. Milestone 480 (a server
that logs which channel a request arrived on) returns when sessions reach the spawn service, which
step 4 begins; reopening it is its own promotion. The log is memory only until milestone 687 (the
system log persists through RedoxFS), so the trail dies at reboot: a `BUGS` line, not a blocker.

### Fork 6: Remote login

**Ruled A**, calef, 2026-10-06 (UTC): "Sunset." A follow-on reuses `sunset`, a `no_std` Rust SSH
library by dropbear's author, as the remote-login server library. Its terms:

- The dependency (§46 (thin primitives or whole subsystems)) is approved on the condition that
  `sunset` builds on all three bare-metal targets, checked against §198 (the glue is ours, the
  primitives are not).
- Before any nife machine exposes SSH to a network, the follow-on passes a security gate: fuzz
  `sunset`'s packet parser and review the glue.
- dropbear through the C seam (§31 (the foreign-language seam)) is the fallback. OpenSSH was weighed
  and not chosen: its fork and uid privilege separation does not survive a port to a capability
  system.
- SSH logins go through the same `greeter` and `login` path, so forks 2 and 5 apply. Recovery stays
  local-only.
- Upstream hardening. calef, same day: "We should seriously consider contributing any hardening we
  do to sunset as a push upstream if possible." Hardening the gate finds goes upstream where it can.
  A security bug goes to `sunset`'s maintainer privately first, never as a public pull request
  first. calef approves each upstream submission, because each one leaves the machine.

It depends on this slice, milestone 649 (every client of a network stack shares its socket numbers)
and milestone 783 (the network stack seeds its random generator from the clock).

## The first slice

This milestone, shippable alone: two identities' sessions, concurrent and isolated, on the boot
console.

0. Check that a multiplexer can sit above x86_64's kernel-resident console. If not, stop and report.
1. `greeter`, one per terminal, as fork 1 A.
2. `console_multiplexer` on the boot console, Alt+F*n* switching, a greeter on every console.
3. `login` loses the terminal and `terminal_held`, and `LOGOUT` moves to the session's private
   channel.
4. A session gets a spawn endpoint that serves plain programs and refuses every activation verb and
   every owner-only endowment, unless the session presents the matching capability (fork 2 B).
5. `login`'s audit records and the owner-credential events reach the system log (fork 5).
6. `memory_broker`, with the owner's revoke and the per-identity cap file, shaped so a program could
   be its client later (fork 4 D).
7. The §221 amendment: owner authority by identity, the owner-password prompt, the store refusal,
   a recovery boot, and a build-time owner credential for test images. Every gate and bench script
   that types at today's prompt logs in first.

### Exit criteria a stranger could check

Under QEMU on aarch64 (`virt`), riscv64 (`virt`) and x86_64 (`q35`), one `cargo xtask` gate that
`script/test` runs, exiting 0 on all three:

1. Two at once. The gate logs `alice` in on console 2, switches with Alt+F3, and logs `bob` in.
   `alice`'s job, started before the switch, finishes while console 3 is showing, and its output is
   on console 2 when the gate switches back.
2. Files are isolated. Each writes a marker in its own subtree. Each then tries to open the other's
   marker by path and is refused.
3. Terminals are isolated. No byte `bob`'s session writes appears anywhere in console 2's
   transcript.
4. Memory. `alice`, alone, starts jobs until the broker refuses, holding more than half the pool.
   `bob`'s program is then refused by name. After the owner, logged in on console 1, runs `revoke alice`, the
   broker's free count rises and `bob`'s program runs.
5. Owner authority is an identity's. `vouch` from `alice` or `bob` is refused with a named code,
   and succeeds from the owner's session.
6. A listed identity gets the authority. After the owner adds `alice` to `may-run-unvouched`,
   `alice`'s next login holds it and `bob`'s does not.
7. Liveness. Killing `alice`'s shell returns console 2 to the greeter's prompt with no restart of
   `login`.
8. Audit. The system log's reader, in the owner's session, shows both logins and the logout, each
   stamped with its identity, and the owner-password set from criterion 12.
9. Measured, not gated: `alice`'s answer time while `bob` spins, and pages stranded by
   interleaved growth.
10. `login_hands_out_the_terminal_once_and_denies_a_concurrent_second_login_until_logout` is
    retired, and its replacement proves two concurrent logins get distinct terminals.
11. New rows in `notes/confinement-claims.md` for user against user, each with a replayable
    falsification.
12. Owner bootstrap. A boot with no owner credential asks for one before any shell. A boot whose
    store is unreadable refuses every login, names the recovery boot and logs the refusal. The
    recovery boot reaches a shell with no password and is logged. A release image carries no preset
    credential, checked by the gate.
13. A `design/decisions/` section records the session object, the spawn endpoint's restriction and
    the broker's wire format. §253 records the rulings, not these.

## Follow-ons, proposed, unnumbered

- The `grant_broker` of fork 2 D. Depends on this slice.
- Remote login over `sunset`, fork 6, on the terms above. Depends on this slice, 649 and 783.
- Memory for running programs from the same broker: #1777's growable heap.
- Graphical sessions in compositor windows, after milestone 270 (wire `virtio-gpu-pci` and
  `virtio-input` into the x86_64 test runner) and when calef wants graphics.

## BUGS

- Virtual terminals on one serial line are one person's sessions. Two people arrive with fork 6.
- The audit trail lives in memory until milestone 687, so it does not survive a reboot.

## Index row

The machine stops being one person's. Two identities log in on virtual terminals at once, each
confined to its own files, terminal and share of memory, with authority granted at login and every
login audited. It retires milestone 49's single-session rule and answers milestone 481's condition.
