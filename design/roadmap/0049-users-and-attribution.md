---
status: BUILT
raised: 2026-07-31
built: 2026-08-27
---
# 49. Users, login, and attribution: what identity is for once it stops being authority

A login service exists and is wired into the real interactive boot on both ISAs. It authenticates
against the credential service milestone 56 (secrets and entropy) already built. `login` hands back all
three capabilities this milestone names: a directory, a budget, and a terminal. See BUGS for what
remains a named, accepted limitation rather than a blocker.

At boot, `crates/system_initializer::boot` builds `credentialer`, `identity_provisioner` and
`login`, plus `audit_sink`. The sink drains `login`'s audit trail so its blocking send never parks
the service. All of them draw on the real virtio-rng-backed entropy service that DECISIONS §120
(a QEMU-only virtio-rng stopgap)'s 2026-08-26 amendment unblocked. The boot provisions a
demo identity, `operator`, with a password it generates from that entropy service. It prints the
password once, before the prompt:

    init: login ready -- generated credentials: identity 'operator' password '...' (shown once; use it now)

That is the shape a cloud image's generated first-boot password already takes. `login` also holds
a `WRITE | GRANT` view of the interactive terminal, so a successful login receives it:
single-session, deny cleanly (see `components/src/login.rs`'s "The terminal: single-session, deny
cleanly"). Proven by `script/swish-check` on both ISAs, which sees the generated-credential line
printed by a real boot. Also proven by `kernel::user::login_tests` on both ISAs, all ten tests,
including `login_hands_out_the_terminal_once_and_denies_a_concurrent_second_login_until_logout`.

Two appendices carry the working this page has no room for. Neither is needed to act on this page.

- [`build.md`](0049-users-and-attribution/build.md): each piece as it landed and how it was
  proven: the service and its tests, per-identity subtrees, the channel-per-client front door, the
  caretaker measurement check, logout and reclamation, and the three-step boot wiring.
- [`bugs-found.md`](0049-users-and-attribution/bugs-found.md): the resource leaks and boot hangs
  found while building it, each of which outlives `login`, and why two kernel limits were raised.

A named prerequisite for milestone 152 (durable delegation): a scheduled job registered by a
specific user (milestone 129 (scheduled execution)'s #387) needs a durable principal to be
supervised by. Login-produces-capabilities is where that principal first exists. 152 gates on this
milestone rather than guessing at identity itself. It inherits §109 (attribution is a channel
property)'s channel-shaped answer to how attribution composes with its own per-user sessions.

## In brief

Unix's uid does four different jobs at once. Three of them are already answered here, structurally
and without anyone having declared it. The fourth had no mechanism at all. This milestone writes
down the first three, builds a login service that produces capabilities instead of changing an
identity field, and decides what to do about the fourth.

Users and groups are Unix's ambient authority mechanism. A process's authority comes from who it
belongs to, not from what it was given. That makes every program a confused deputy by default.
`setuid` is that idea in its purest form: a program running with the union of its owner's authority
and its invoker's intent. It has been a security disaster for fifty years. Saying "we do not have
uids" is not the interesting claim. The interesting claim is that *the work a uid does still has to
get done*. Here four different mechanisms do it instead of one overloaded number.

## The starting position: the tree is already identity-free, by accident of good design

Verified rather than assumed: no `uid` or `gid` appears anywhere in our logic. The vendored RedoxFS
on-disk `Node` carries the fields, and `create_node` inherits them from the parent, because that is
the format. Nothing ever reads them for an access decision. The `std` PAL lists permissions under
Unsupported, and `set_permissions` refuses rather than pretending. So this milestone documents and
completes a position the code already holds.

## What each of the uid's four jobs becomes

| Unix uses uid for | Here | Status |
|---|---|---|
| Authorization | Capabilities. There is no check to bypass because there is no check | Built |
| Isolation between humans | Milestone 47's per-shell root. Two people's shells hold different directory capabilities and neither can *name* the other's files | Built (never demonstrated multi-user) |
| Resource accounting | The untyped budget. A user's allowance is the region they were granted; `run --mem 16` splits from it | Built |
| Attribution (*who did this?*) | A channel (DECISIONS §109) | Decided and built: `login` mints a fresh channel per successful login and records who established it (see BUGS on the scope of what is proven) |

Isolation is the one worth dwelling on, because it is stronger than what a uid buys. Unix isolates
by *refusing* a request that names another user's file. The name is still sayable, the check is
still code that can be wrong, and root skips it. Here no capability reaching those files exists in
that shell, so the request cannot be phrased. A check that cannot be wrong because it is not
performed.

## There is no root, and that is a statable property

Milestone 22 (trusted init) did something Unix structurally cannot: `root_supervisor` gives
its authority away, deleting its untyped once the sub-servers are running. The consequence is worth
stating plainly next to the benchmarks, because a demonstrator exists to make claims like it. There
is no point after boot at which any principal can do everything. That is not a policy or a hardening
measure; no capability naming everything survives. Unix's root is always one `sudo` away by
construction.

## Groups are a delegation pattern, not a mechanism to build

Sharing is two parties holding the same capability, or capabilities derived from a common one.
Nothing needs to be added to support it. Managed sharing (revocable, narrower for some holders,
auditable) is a caretaker. `fs_file_caretaker` is already that shape: a component holding a resource
and serving several clients on its own terms. So this milestone builds no group mechanism. It
documents the two patterns instead, because the alternative is someone inventing a group table
later.

## Login: authentication produces capabilities

Unix login authenticates and then mutates an identity field. Here it authenticates and then hands
over a capability set: a root directory, a budget, a terminal. That is a better failure mode as
well as a cleaner model. A compromised login service leaks *what it can grant*, not the ability to
become anyone. It is the powerbox pattern with the human at one end. It needs a real answer to a
question: who gets which capabilities at startup. For the one login path this milestone builds,
that is decided at run time rather than baked into `crates/system_initializer`'s boot wiring.

## What is built

`login` (provisional name) is `components/src/login.rs`, with its wire contract `crates/login_protocol`
(provisional). It holds the credential service's verify endpoint (milestone 56, unmodified), the file
service's root directory capability, and a construction budget. A client first sends
`login_protocol::CONNECT` and gets a private request/result pair and staging page of its own. It then
presents an identity and a secret. On a match, `login` builds a fresh `fs_subtree_caretaker` scoped
to that identity's own subtree, splits a fresh budget, and delegates both over
`abi::endpoint::SEND_CAP`. It also delegates a logout ticket and, to the first caller, the terminal.
Two successful logins are therefore two different endpoint *objects*, which is the channel-shaped
attribution §109 decided on. `login` sends one attribution record per successful login (identity,
sequence number) on its own audit endpoint. That makes §109's "can say who a channel belongs to"
checkable rather than merely claimed.

Each of these was proven end to end by a test that exercises the capability, not just its arrival.
A directory answers a real `READDIR`, a budget retypes a real page, and two identities each get a
working channel named correctly in the audit trail. [`build.md`](0049-users-and-attribution/build.md)
has each piece, its test and its date.

## Attribution is the actual work, and the one place Unix does something we do not

A capability says what you may do. It says nothing about who did it. Unix gets audit almost free
because the uid is present at every syscall and doubles as the answer. Measured boot (milestone 22,
trusted init) establishes *what code* is running and capabilities establish *what it can reach*. Nothing
records *who asked*, and that gap is real.

Decided (calef, 2026-08-22, DECISIONS §109): channel. A server that wants to know who is asking
gives each principal its own endpoint, established once, and logs which one a request arrived on.
This tree had already faced the question three times under different names. They were the
compositor's shared-endpoint identity, the FS server's confinement, and the fault endpoint's sender
trust. Each time it gave each principal its own channel rather than badge a shared one (seL4's
mechanism, never built here), without anyone naming it as one policy.

It also composes for free with milestone 152's durable per-user sessions. Once a user's session
exists, every service reached through it already has a per-user channel. That is attribution at
exactly the granularity audit wants. §109 has the full reasoning, including where the channel
model's cost eventually bites: roughly tens of concurrently durable sessions against `MAX_REGIONS`.
This is a general-purpose OS, so that headroom should be measured against cron's uncapped,
resource-bound norm, not a consumer file-sharing throttle. §109 corrects an earlier draft that used
the wrong comparison. Raising `MAX_REGIONS` again, the same cheap move already made once, is the
expected response as durable sessions are built and measured.

## Sequencing

After 47, because isolation is 47's per-shell root and login hands out exactly what 47 defines; 47
landed 2026-08-22. Built across several lanes, in order:

1. the login service and its channel-shaped attribution logging;
2. per-identity subtree scoping, under DECISIONS §117 (a principal's subtree is named by its
   identity string);
3. the channel-per-client front door;
4. the real virtio-rng entropy chain under the interactive boot (DECISIONS §120);
5. the boot wiring and the terminal, which closed the milestone on 2026-08-27.

Milestone 152 (durable delegation) gates on this milestone reaching BUILT, which it has; that gate
is clear.

## BUGS

Named here as well as at the component, so a reader meets the scope where they meet the status line.
`components/src/login.rs`'s own BUGS words each item more precisely.

- Resolved, 2026-08-23: per-identity subtrees. Every principal used to get the same subtree,
  `filesystem_protocol::fixture::tree::SUB`, with the same rights. Each identity now gets a subtree named by the
  identity string itself (§117), created at provisioning time by milestone 155 (a provisioning tool)'s
  `identity_provisioner`, not auto-vivified at login. Two bounds came with it. An identity longer
  than sixteen bytes (`filesystem_protocol::grant::MAX_NAME`) cannot get a per-identity subtree at all. An
  authenticated identity with no provisioned subtree is refused indistinguishably from a wrong
  password, a considered fold.
- Resolved, 2026-08-27: the terminal. The roadmap's recorded recommendation, the deny-cleanly shape,
  is what got built. The first successful caller gets `WRITE` on the terminal. Every later login is
  refused with `login_protocol::NO_TERMINAL` (a dedicated code, not folded into `DENIED`) before its
  secret is relayed, until a bare `login_protocol::LOGOUT` frees it. Named limitations, scoped to
  today's single-tenant boot: `LOGOUT` is unauthenticated, and nothing checks the liveness of an
  abandoned holder. Real multiplexing stays unbuilt on purpose; the narrow shape commits to nothing
  the wider one would later have to unwind.
- Resolved: the interactive boot. All four services are built by `boot` on both ISAs, with a
  generated demo password. The three-step plan and the order of its builds are in
  [`build.md`](0049-users-and-attribution/build.md#wiring-the-interactive-boot). The two hangs found
  on the way, and the budget and table raises, are in
  [`bugs-found.md`](0049-users-and-attribution/bugs-found.md). The audit receiver is
  `components/src/login_audit_receiver.rs`.
- Resolved: the caretaker load is measured. `login` verifies `fs_subtree_caretaker` against the same
  measurement table init uses, once at startup, per milestone 104 (the measurement continues past
  init)'s discipline. A refusal answers every login `DENIED` rather than crashing the service.
- Resolved: logout reclaims a session. The client gets a fourth capability, a logout ticket, and
  destroying it reclaims the caretaker's construction memory. The order the client destroys its two
  regions in matters.
- Resolved: channel per client. `login`'s front door gives each client its own channel rather than
  one staging page shared across every client, so two callers reaching it together contend only for
  service order, never for each other's secret. A channel, answered or not, is never reclaimed in
  this slice.
- Open, accepted: the audit endpoint proves establishment, not per-request attribution. §109
  describes a server establishing a channel and, separately, a server logging which channel a later
  request arrived on. `login` is only the first half. No server here needs the second: every
  multi-client server either serves one principal by construction (`fs_subtree_caretaker`) or is
  anonymous by design (the credential service). Milestone 480 carries the refusal.

## Follow-on

- **Milestone 152.** Durable delegation, which gates on this milestone reaching BUILT so that a
  scheduled job has a durable principal to be supervised by. That gate is now clear.
- **Milestone 153.** `OutOfMemory` collapsing a spent budget, a full capability table and
  `MAX_REGIONS` into one error code. This milestone is where it first cost real time: four memory
  hypotheses were measured and ruled out before the capability table was the answer.
- **Milestone 159.** The JH7110's TRNG: milestone 159 (a real hardware entropy source), minted alongside §120. The boot
  wiring here runs on the virtio-rng backend, which does not exist on silicon. Nothing here changes
  159 either way, as §120's "what this does not decide" said.
- **Recorded.** `MemoryRegion::DESTROY` frees the region and the objects retyped from it but never the
  destroyer's own capability-table slot naming it. A server that destroys a region per request runs
  out of table while its budget still looks healthy. `design/roadmap/0153-out-of-memory-causes.md`
  carries the account.
- **Recorded.** A region destroyed out of LIFO order strands its pages until its parent dies, which cost
  368 pages of holes in one suite run here. The general form, that a short-lived region wants a
  parent nothing long-lived is carved from, sits with the mechanism in
  `crates/memory_regions/src/lib.rs`.
- **Recorded.** An identity longer than sixteen bytes cannot get a per-identity subtree, because the
  grant name travels in two argument words (`filesystem_protocol::grant::MAX_NAME`) and `login_protocol` allows
  sixty-four; `login` refuses rather than truncating. An authenticated identity with no provisioned
  subtree is refused indistinguishably from a wrong password, a considered fold. Both are in
  `components/src/login.rs`.
- **Recorded.** A per-client channel, answered or not, is never reclaimed in this slice; `LOGOUT` is
  unauthenticated; and nothing checks the liveness of a terminal holder that walked away.
  `components/src/login.rs` has each of the three beside the code that causes it.
- **Recorded.** The credential service still shares one verify page across every client, the
  structural limit `login`'s front door escaped by minting a page per caller. It is written up in
  `notes/credentials.md`, along with the sealed store's inability to revoke one secret.
- **Recorded.** The construction budget (2048 to 12288 pages) and `CAPABILITY_TABLE_SLOTS` (16 to 17)
  were raised with real margin found empirically, not tuned to a minimum. A later lane that wants
  either tighter has bisection work rather than a guess to correct.
  [`bugs-found.md`](0049-users-and-attribution/bugs-found.md#two-limits-raised) records what was
  paid and why.
- **Refused.** The second half of DECISIONS §109, a server logging which channel a request arrived
  on. No server in this tree needs it: every multi-client server either serves exactly one principal
  by construction (`fs_subtree_caretaker`) or is anonymous by design (the credential service).
  Building it now would be a mechanism with no consumer to shape it.
  This refusal is milestone 480
  (design/roadmap/0480-a-server-that-logs-which-channel-a-request-arrived-on.md), which carries it
  with the condition that would change it.
- **Refused.** Real terminal multiplexing. `login` hands the terminal to the first successful caller
  and refuses the rest with a dedicated code until `LOGOUT`, deliberately, because the narrow shape
  commits to nothing the wider one would later have to unwind.
  Milestone 806 (concurrent login sessions) is the work that lifts it.

## Index row

three of Unix's four uses for a uid are already answered structurally; the fourth, attribution, is
a channel (DECISIONS §109). `login` is wired into the real interactive boot on both ISAs,
provisions a boot-generated demo credential, and hands back all three capabilities the milestone
names: a directory, a budget, and (single-session, deny cleanly) the terminal
