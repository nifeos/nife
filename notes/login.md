# Login

Authentication that produces capabilities instead of mutating an identity. Milestone 49's login
half; the attribution half is [DECISIONS §109 (attribution is a property of a channel, not of a capability)](../design/decisions/0109-attribution-is-a-channel-property.md).

The contract is `crates/login_contract`, the service is `components/src/login.rs`, and its test
client is `fixtures/src/login_test_client.rs`. The names were ratified 2026-09-15 (calef, the
whole `login` family).

## The problem this exists to solve

Unix login authenticates a presented password and then, on success, mutates a global field: the
process's uid becomes the user's. Every future authority decision for that process reads back through
that one number. It is efficient, and it is why `setuid` exists. That is a program running with the
union of
its owner's authority and its invoker's intent, which has been a security disaster for fifty years.

This system has no uid to mutate, and milestone 49 (users and attribution)'s own doc names the shape the replacement should
take: authentication should **hand back a capability set** rather than change anything ambient. A
compromised login service then leaks *what it can grant*, not *the ability to become anyone*. That
is the same trade the credential service (milestone 56 (secrets, credentials, and the entropy to make them safe)) already made for the secret store itself.

## The shape

`login` relays the presented identity and secret to the credential service's `VERIFY` unchanged: it
never touches `credential_contract`'s protocol or `credentialer.rs`. On a match it **builds**,
rather than narrows, a capability set. It holds a fresh `fs_subtree_caretaker` (the same
construction
`crates/system_initializer` performs for a directory-granted spawn) and a fresh budget split off its
own construction untyped. Two different successful logins therefore hold two different endpoint
*objects*, which is §109's channel-shaped attribution rather than a badge on a shared one.

The exchange is two-phase since milestone 49's channel-per-client update
(`crates/login_contract`'s module docs have the diagram). The front door every client is handed at
spawn answers only `CONNECT`, and the answer is a fresh, private request/result pair and staging
page delegated to exactly that caller. The identity and secret travel only on that private
pair. Two clients that reach the front door close together can contend only for which is
served first, which is a wait, not a hazard. The process keeps its own copies of the private
triple it delegates (the "delegate, then keep going" pattern `FS_PAGE_FRAME` already uses, unlike
the "delegate, then drop" pattern `mint`'s capabilities use). It is the one that goes on
to serve the login that channel was minted for.

### Why a caretaker, not a narrowed copy of the file service's own endpoint

The cheaper-looking move is to hand every authenticated client a `WRITE`-narrowed copy of the same
directory capability `login` itself holds. That fails the property this milestone is for. A
narrowed copy of one shared endpoint is still one endpoint, so the file service (or anything
downstream) cannot tell two principals apart. That is the shared-endpoint anti-pattern §109 names
three times over (the compositor, the FS server's handle table, the fault endpoint) and
refuses. Building
a fresh `fs_subtree_caretaker` per login costs a process per login and buys a real, distinguishable,
independently revocable object per principal.

### Which subtree a principal gets (§117 (a principal's subtree is named by its identity string))

Each identity is attenuated to the subtree named by its identity string itself (§117, 2026-08-23),
used directly, with no separate lookup table. `alice` and `bob`
land in two different, independently-scoped subtrees; neither can name the other's. The subtree must
already exist: `identity_provisioner` (milestone 155 (a provisioning tool)) provisions it at
provisioning time. This program never creates one (§117's own file records why provision-time
creation was chosen over
creating it on a principal's first login). An authenticated identity with no provisioned subtree is
refused, folded into the same `DENIED` a wrong password gets.

Two bounds came with that change, and both still hold. An identity longer than
`filesystem_contract::grant::MAX_NAME` (16 bytes) cannot get a per-identity subtree in this slice
at all, though `login_contract::MAX_IDENTITY` (64 bytes) would otherwise accept it. The grant
name travels in two `START` argument words, not a frame, and `mint` refuses rather than truncate
a name that two identities agreeing on their first 16 bytes would silently share (a collision
hazard, not merely a usability one). Lifting it means a frame for the caretaker's grant, a change
to `filesystem_contract::grant`'s contract and every caretaker built against it. And the
unprovisioned-subtree fold is a considered answer, not the accident of reusing the fold. A caller
must not be able to tell "your identity has no home" from "your password is wrong" by comparing
outcomes across attempts. That would let a caller probe which identities are provisioned without
ever presenting a right password for one (`login_contract::DENIED`'s own doc gives the reasoning).
The honest cost is that an operator who forgot to run `identity_provisioner` sees the same denial a
typo would produce. The audit trail does not help, since it only records a successful login.
Distinguishing the two would need a deliberately-weaker operator-facing channel than the login
result, real work this slice did not build.

### Provisioning an identity: the subtree first

`identity_provisioner` is `useradd` one level down. Whoever holds the credential service's provision
endpoint (before its seal) and `WRITE` on the principal tree may run it. It authenticates no caller,
like the provision endpoint itself. How an operator comes to
hold those two capabilities is the boot-wiring question `login.rs`'s BUGS name.

It makes two calls to two servers with no cross-server commit, so their order decides what a
failure leaves behind. The `MKDIR` of the home subtree goes first. If it fails, nothing was stored
and the caller retries. If the `PUT` then fails (`credential_contract::FULL`, `NO_ENTROPY`, or a
duplicate identity), the result is an orphaned, empty subtree with no identity. That is inert, and
re-running recovers it. The other order risks a credential that verifies for an identity with no
home. So `MKDIR`'s `EEXIST` means "already created, proceed", exactly the state a failed run
leaves, and any other `MKDIR` failure stops before the store is touched.

It never sends `SEAL`: sealing ends provisioning for the whole store, the caller's decision once
every identity is in. It does not deprovision.

## Reclaiming a session: the design argument

Moved here 2026-10-09 from the module doc; the resolved history is in git.

A successful login delegates four (or five, with the terminal) capabilities, and the fourth is
the client's **logout ticket**. It is `mint`'s own construction region, undropped and narrowed
to `WRITE`, with nothing left to `SPLIT` or `RETYPE` (the region's whole budget went into building
the caretaker). Its only remaining use is `MemoryRegion::DESTROY`. Calling it reclaims the
caretaker's TCB, address space and endpoints, and the pages come home to `CONSTRUCTION_UT` under
§13 (capability revocation and untyped reclamation) region ownership (the region's builder, not its destroyer). The client's budget (the third
capability, `WRITE | GRANT`) was always able to do the same for its own half. A full logout
destroys the budget and then the region and gives back everything a session spent.

Two candidate shapes for giving this memory back were refused before the ticket was found. Both are
recorded with their reasons in the appendix: a supervision endpoint reaching back into this
process, and a caretaker `DESTROY`ed by name. The first is the durable-session machinery milestone
152 is scoped to design in general. The second needed a name for the caretaker's region this
process does not keep. The ticket needed neither a session nor new supervision plumbing. The
caretaker's own construction region is the only thing that ever needed tearing down, and its builder
can hand the means directly to the one party who should hold it.

`MemoryRegion::DESTROY` works here for a reason checked against the documented gap of §32 (a
supervisor may collect a corpse without being able to build one). `REAP` collects an already-dead
thread, and `DESTROY`, which kills a live one, refuses permanently only
against a thread blocked on an endpoint *outside* the region being destroyed
(notes/hung-component.md's case (c)). The caretaker's client-facing endpoint is retyped from the
same region. So its steady state is case (b), blocked on an endpoint whose region the destroyer
holds: reclaimable, with collateral (the wait queue drains, the blocked receive aborts). The one
narrow exception is the instant the caretaker is mid-`forward` to the file service, a `CALL` on
an endpoint the region does not own. A `DESTROY` in that window is refused transiently, and a
client retries a bounded few times rather than treat one refusal as final.

## The terminal: the design argument

Moved here 2026-10-09 from the module doc; the 2026-08-27 resolution is in git.

A terminal in this system is a singleton hardware-backed resource, wired once at interactive boot
(`crates/system_initializer::boot`), so handing it to a login-authenticated principal has to say
what a second concurrent login gets told. Naming another capability slot does not answer by
itself. The shape built is the narrow one, matching what this boot actually is: one interactive
boot, one physical terminal, one session live at a time. A login while the terminal is held is
refused `NO_TERMINAL` before its identity and secret are relayed to the credential service. The
first login to arrive while it is free receives the terminal endpoint as its fifth delegated
capability, and `LOGOUT` releases it for the next login. `LOGOUT` is a bare word on the front
door, not a private channel, because it carries no secret and there is nothing a shared endpoint
exposes by handling it directly. The whole of the state this needs is one `terminal_held:
bool`.

What this does not build, named rather than assumed away: `LOGOUT` authenticates nothing. That is a
deliberate choice for today's actual deployment, one interactive boot with no untrusted co-tenant
reaching the front door. (`login_contract`'s own BUGS records the real interruption hazard a hostile
holder of the front door would pose in a different deployment). There is no liveness check on
whoever currently holds the terminal. A session that exits, crashes, or simply never calls
`LOGOUT` leaves the terminal held for the rest of this process's life. This process has no wait-any
primitive with which to watch a client and keep serving new connections at the same
time (the structural bound notes/hung-component.md names). Recovering from an abandoned session means
restarting this process. And this is explicitly not the "real multiplexing" shape (more than one
live session, each with its own view, composed the way the compositor composes windows). The
roadmap's own BUGS entry recommends against building that now, and choosing the narrow shape
commits to nothing the wider one would later have to unwind.

## Who may run new native code (§219 (how the shell names an installed program to the spawner) gate D2)

A session may run bytes nobody vouched for only when its identity is on the owner's list
(`login_contract::RUN_UNVOUCHED_LIST`, empty by default; §219's consequence, §221 (the boot prompt is the owner's console) ruling 2). The
run-unvouched capability, when the spawner placed one (the progenitor does, after building this
process), is delivered as a sixth capability on the `OK` reply, `WRITE` with no `GRANT`: a session
can present it and cannot pass it on. Which users may run new code is therefore decided here, per
session, from a file the owner writes and no session can reach. No session built here holds a
spawn endpoint to use it with yet, so it is delivered and proven (`kernel::user::login_tests`)
rather than exercised.

## Measured boot: the posture

This program spells measured boot's load-or-refuse decision itself. `_start` runs
`measured_boot::verify_in_manifest` over the caretaker blob and folds all three outcomes, absent,
refused and not-an-ELF, into `care_elf = None`, answered per login with `DENIED`. It does not call
`measured_boot::verdict`, which milestone 246 (measured boot's refusal path is tested by nothing)
moved the same decision into a crate for.
Switching would buy the one thing 246 was about, the refusal branch tested rather than assumed,
at the cost of a `Verdict` whose `unvouched` field this program has nothing to do with. It has
not been done, on the record, because it is a change to a boot path the changing lane was not
gating. The check's trust root is the progenitor's hand-over (both the bytes and the table arrive
from it, already verified there), so what remains is a consistency check. It is kept because it
costs one hash and catches a spawner that pairs the wrong two blobs. Why the fold into `DENIED` is
not anti-oracle reasoning here (a failed measurement varies with nothing a caller controls). The
proof test and its limit are recorded at the check itself.

## What attribution means here, and what it does not

§109 describes two halves: a server that establishes a channel per principal, and (separately) a
server that, serving a request later, can say which channel it arrived on. `login` is the first
half. It sends one record per successful login, on its own audit endpoint, naming the identity
that established each channel, so the property is checkable rather than merely claimed.

**No server in this tree today needs the second half.** `fs_subtree_caretaker` already serves exactly
one principal by construction, so there is nothing for it to distinguish. The credential service is
anonymous by design and neither wants nor needs to know who is asking. The second half is real, named
follow-on for whenever a genuinely multi-tenant consumer exists; forcing it onto either of those two
would be inventing a requirement neither has.

## EXAMPLES

Two phases since the channel-per-client update; `crates/login_contract`'s module docs carry the
full diagram.

```rust
use login_contract as proto;

// Phase one, on the front door every client is handed at spawn.
send(REQUEST, proto::connect_word(), 0, 0);
let (word, _, _) = receive(RESULT);
assert_eq!(word, proto::CONNECTED);
let (_, priv_request, _) = receive_cap(RESULT); // WRITE
let (_, priv_result, _) = receive_cap(RESULT);  // READ
let (_, page, _) = receive_cap(RESULT);         // the staging page, READ | WRITE

// Phase two, on the private pair only.
let w0 = proto::place(page, b"alice", b"correct horse battery staple", proto::LOGIN).unwrap();
send(priv_request, w0, 0, 0);
let (verdict, _, _) = receive(priv_result);
assert_eq!(verdict, proto::OK);
// In this fixed order: the directory, the file service's shared frame, the budget,
// the logout ticket, the terminal (and the run-unvouched endpoint when the reply says so).
let (_, dir_ep, _) = receive_cap(priv_result);
```

A refusal sends nothing further:

```rust
let w0 = proto::place(page, b"alice", b"wrong", proto::LOGIN).unwrap();
send(priv_request, w0, 0, 0);
let (verdict, _, _) = receive(priv_result);
assert_eq!(verdict, proto::DENIED);
// No RECEIVE_CAP here. The protocol promises nothing follows a refusal; a client that tried anyway
// would block forever, which `login_test_client.rs` relies on as its own check that the promise
// holds: its wrong-secret run is `LOGIN` with `credential_proto::fixture::WRONG` for a secret, the
// same code as the honest run (milestone 293).
```

## What is proven, and where

Host tests (`cargo test -p login_contract`, milliseconds, no emulator): the request page's encoding
round-trips through the same `credential_contract` helpers the credential service uses, and the attribution
hint is stable per identity and distinct across identities.

Guest tests (`kernel::user::login_tests`, both aarch64 and riscv64):

- A correct identity and secret produce a directory capability that answers a real `READDIR` and a
  budget that retypes a real page, not merely that the capabilities arrived.
- A wrong secret is refused, and the client proves nothing followed the refusal by never calling
  `RECEIVE_CAP` on that path.
- Two different identities each get an independently working channel, and the service's own audit
  trail names each correctly, in the order they were established.
- Two clients connecting together get independent channels and neither observes the other's secret
  (`two_clients_connecting_together_get_independent_channels_and_neither_observes_the_others_secret`).
- A full session's worth of memory comes back at teardown, in the destroy order
  (`caretaker_teardown_reclaims_a_full_session_worth_of_memory`), and the service serves past the
  old capability-table ceiling
  (`the_login_service_serves_past_the_old_capability_table_ceiling`).
- The caretaker measurement matches the real table and a tampered one would be refused
  (`logins_caretaker_measurement_matches_the_real_table_and_a_tampered_one_would_be_refused`).

## The kernel's test harness

`kernel/src/user/login_service.rs` starts `login` for the guest tests, and it starts it the way
`crates/system_initializer` does. `login` gets the caretaker, the program measurements and the
durable-session programs as read-only blobs, never the initial ramdisk itself. A harness that
starts a program differently from the system is not testing that program.

`start` returns only once `login`'s start-up pass is over. That pass reads the manifest, the
suspended list and each stored schedule through the file service's shared page, which `login` treats
as its own because no session is live yet. A caller that writes that page during the pass races it.
So `start` sends `REDERIVE_SKIPS`, the one front-door word with no side effect, and waits for its
answer.

The eight capabilities are granted one at a time, each asserted into the slot
`components/src/login.rs` expects. Collected into an array first, an unoptimised build put the whole
table and a temporary per element on the stack. That took the frame to 4,288 bytes, past the
4,096-byte guard page `script/stack-frame-check` gates. One at a time it is 1,632 bytes on aarch64
and 1,648 on riscv64.

Each `login_test_client` run gets a five-page scratch region of its own. It pays the page tables for
mapping the frame `CONNECT` delegates, before the run holds any other budget, and its report endpoint
is retyped from it. The kernel's own endpoint chunks are never given back, and the machine shares
`sched::MAX_RENDEZVOUS` endpoints and `memory_region::MAX_REGIONS` regions. So `wait_client` reclaims
the scratch once the run's thread is dead, and the endpoint goes with it.

## BUGS

See `components/src/login.rs`'s own BUGS for the itemized list, which this note summarizes
rather than repeats. The living items:

- one durable session at a time, and the shared durable window;
- start-up waiting on each re-derived timetable, and `LOGIN_CONSTRUCTION_PAGES` sizing;
- a half-failing `user_timetable_keeper` leaving splits behind;
- `SUSPEND` waiting on the terminal holder, and ending a durable session waiting for a running
  job;
- a durable session never retired until the next login;
- the 16-byte subtree-name bound;
- the measured-boot decision this program spells itself;
- the audit trail proving establishment rather than per-request attribution;
- not being wired into the interactive boot.

Summarized in
[design/roadmap/0049-users-and-attribution.md](../design/roadmap/0049-users-and-attribution.md)'s own BUGS.
