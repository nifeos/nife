---
status: BUILT
raised: 2026-08-22
built: 2026-10-03
---
# 152. Durable delegation: authority that outlives the session that requested it

Updated 2026-10-03 (UTC). Minted 2026-08-22, from a milestone 129 (scheduled execution) discussion: calef
wants nife to support multiple users, and wants the jobs a user schedules to carry capabilities that
reflect that user's own authority. Working through what that requires surfaced a gap this tree had
not needed to close before, and #387 (milestone 129's `--mem` grant, held pending this) is where it
was found. All four design pieces (the durable session, the on-disk schedule store, boot-time
re-derivation in `login`, and the real registrar) are built and tested. Flipped to `BUILT` on 2026-10-03 after a re-check against the tree. The two residues it
carried, a real-boot gate that types `SCHEDULE` and a place for a durable job to write, are
routed in `## Follow-on` (a proposal and milestone 687) rather than held open here.

## In brief

Option 3 of #387's runtime-registration question already gives "a scheduled job can never hold more
authority than its registrar held" for free: `timetable::Registry::register(doc, held)` already
takes an arbitrary `Held`, so a registrar handed a narrower bundle than the scheduler's own produces
narrower jobs, structurally. Making that registrar a user's own session instead of a fixed
system component is a small idea with one hard consequence. A session's authority exists only as
long as the session does, but a scheduled job needs to keep firing after the user who registered it
disconnects.

Concretely: if calef authenticates, registers `every 24h verify_backup`, and disconnects, that
job still needs to fire tomorrow, on the authority he held at registration. Whatever holds that
authority between registration and firing has to outlive the connection.

## The existing rule this collides with, and what it implies

DECISIONS §92 already decided the general shape of "a component holding derived authority," and
its rule is the opposite of durable: *"A caretaker holds authority derived from a grant made to
one client. When the grantee dies, the derived authority should die."* `fs_subtree_caretaker` is
supervised by, and dies with, the client it serves (§40's subtree-death rule). That is the right
rule for the case it was built for (a directory grant narrowed for one live command), and it is
exactly the wrong rule for a scheduled job, which is supposed to survive its registrar disconnecting.

This does not make §92 wrong. Its logic says the authority dies when *its client* dies, and the
design below answers what that client is for a durable delegation, rather than inventing a new
supervision rule.

§92's own BUGS section already named the adjacent gap: *"This says nothing about a caretaker
with no client, because none exists yet."* This milestone is that case.

## The design, worked out 2026-08-22

### The durable principal is the session itself, kept alive by its own live children

No new kind of object. The durable thing a scheduled job's authority is supervised by is the
user's own login session, and what keeps it alive past a disconnect is exactly the rule
`Untyped::DESTROY` already has (§16). *A parent with live children refuses to be destroyed, and
becomes destroyable again once they're gone.* A session that refuses to tear down while it still has
scheduled-job children is that rule, applied to a session instead of a region. Nothing here needed
before was a snapshot of the user's authority or a live re-check against an external identity
source: the job's capability is the *same* live one from the original login, continuing to exist
because nothing tore it down.

Checked against prior art. systemd's `loginctl enable-linger`
solves almost exactly this problem (keep a user's session manager running after logout so pending
timers keep firing). It is a static, explicit admin toggle: an account either lingers or it
does not, independent of whether anything is actually pending. The design here is the dynamic
version instead: a session persists *because* it has scheduled work, and becomes destroyable once it
does not, which is more minimal (nothing outlives its own reason to exist). It maps onto §16's
existing mechanism directly rather than needing a new opt-in flag and a new place to store it.
[loginctl (systemd)](https://www.freedesktop.org/software/systemd/man/latest/loginctl.html).

### Reattachment on reconnect, via a scoped lookup, not enumeration

When a user reconnects and a durable session already exists for them (kept alive by pending jobs),
the new connection needs to find and attach to *that* session rather than minting a second one
alongside it. Otherwise the design degrades into either session proliferation or an orphaned session with
no way back to it.

This cannot be built as "list all sessions and find the match." This tree already refuses that
shape on principle (milestone 126: enumeration is itself authority). It has to be a targeted lookup:
given a *proven* identity, return the one record for it, never a list. The natural home is the
credential service (milestone 56's credentialer) or a login broker built beside it (milestone 49
already describes login as "authentication produces capabilities"). A small table, identity to
durable-session capability (or nothing yet), where a successful authentication returns the existing
entry or creates and records a fresh one. Additive to what 56 already does, not a new kind of ambient
power: still "prove who you are, get back exactly your own thing," never a directory of everyone's.

### Decided: disabling a user's login credentials kills their durable session

calef, 2026-08-22: yes. Revoking credentials cascades to killing the durable session, which
cascades to everything derived from it (§40's subtree-death rule), including every scheduled job the
session was supervising. One action, one consequence, using mechanism that already exists rather
than inventing a second revocation path that has to be kept in sync with the first. Recorded as
[DECISIONS §108](../decisions/0108-credential-revocation-kills-durable-session.md).

### Boot-time bring-up is re-derivation, not restoration

Capabilities do not survive a reboot; nothing in the kernel does. So bring-up at boot cannot be
"reload the session's old authority from disk," because there is no such operation. It has to be
re-derivation: boot re-establishes each durable session's authority fresh, the same way a login
would, without a live person presenting credentials at that moment.

This needs a durable, on-disk record that does not exist yet. Milestone 129's own "Still to
build" list already names this gap without solving it: *"Calendar syntax, wall-clock entries,
persistence... none started."* The runtime-registered, per-user schedule needs its own durable store
(today's `timetable.conf` is the compile-time equivalent, baked into the image), written when a job
is registered during a live session, read back at boot.

Trust at boot does not come from a fresh secret challenge, because nobody is presenting one. It
comes from the durable store itself only ever having been written by an already-authenticated
action: the user proved who they were once, at registration time. Boot only has to trust that
the store is authentic and untampered, which is what measured boot (§22, already one of milestone
49's three answered pillars) and the credential store's own existing persistence (milestone 56's
sealed store) are for. Boot-time re-derivation is a privileged, boot-only operation, in the same
shape as `root_supervisor` handing out its authority once at boot and never again, not a standing
"impersonate any user" capability left lying around afterward.

This belongs to 152, not to milestone 129's own "persistence" line. 129's item is really about
the schedule's data format (calendar syntax, wall-clock semantics); this is about the durable
session's lifecycle at its most extreme boundary, the kernel itself restarting. 129's BUGS entry
points here for the mechanism.

## What this unblocks

#387's runtime registration: §222 (who holds a user's schedule), built here.

## What was built (2026-08-24) and is now retired

`smb_server`'s `DurableSession` first proved §16 (object revocation) holds a budget up while it has a
live child; it went with the SMB code on 2026-08-30. `crates/schedule_store` (provisional) holds the
file names and the manifest format of [§122](../decisions/0122-durable-schedule-store-format.md) and
[§125](../decisions/0125-durable-schedule-manifest.md). `session_reviver`, the boot-only re-deriver
of [§123](../decisions/0123-boot-time-rederivation-privilege.md), ran only under the kernel harness
and was retired on 2026-09-27 (below). Git history has all three.

## What was built (2026-09-26, the live-children proof re-homed on a login session)

`kernel::user::login_tests::a_login_session_with_pending_work_refuses_logout_until_the_work_is_gone`
proves the §16 property on `login`'s delegated budget, driven by `login_test_client`'s
`PENDING_WORK` (provisional, as are its three flags): with a one-page child split off, the budget's
`DESTROY` answers `NotPermitted` and the budget keeps working. Once the child is gone the logout
completes with `LOGOUT`'s proofs. It runs with the rest of `login_tests` on all three ISAs, and
`session_reviver.rs`'s six comments citing the deleted type now cite it. It also found that only the
budget is held up: the caretaker region is a sibling and the logout ticket still destroys it, so a
registrar must build a job's directory inside the job's own region.

## What was built (2026-09-26, the session process and `SCHEDULE`)

To calef's rulings S1 and L2. `login_protocol::SCHEDULE` (provisional) is `LOGIN` plus "open my
schedule": `login` splits a durable budget, builds `components/src/user_timetable_keeper.rs` (provisional) from
a region of it, and `OK` announces the timetable's registration page. The session process builds the
timetable to `timetable::contract` and blocks on the timetable's supervision endpoint. A later login for that identity is handed the same budget and page.
`login` tells a live session from a stopped one by the timetable's exit word in the page, because
`DESTROY` answers `NotPermitted` for a stale name and a busy one alike. An empty replace stops the
timetable, the session process gives its budget back and exits, and the next login retires what is
left. `kernel::user::login_tests::a_users_schedule_outlives_their_login_and_ends_when_they_empty_it`
runs that whole life on every ISA. The ruling put the request after `OK`; it is the request itself,
because `login` cannot wait for a word after `OK` without every client sending one.

## What was built (2026-09-26, suspending a user)

To calef's §108 ruling. `login_protocol::SUSPENDED_LIST` (`suspended`, provisional) sits at the
file service's root in `may-run-unvouched`'s format; the credential store stays sealed. `user
suspend <name>` and `user resume <name>` edit it at the owner's console (`swish-check` types both).
`login` refuses a listed identity `SUSPENDED` after authentication and ends its durable session;
the front-door word `SUSPEND` ends it at once. `session_reviver` skipped a listed identity at boot
(retired 2026-09-27; `login`'s start-up pass skips one now, below).
`SUSPEND`, `SUSPENDED` and `APPLIED` are provisional wire items. The console holds no capability to
`login`'s front door, so on the real boot the cascade runs at the next login attempt; the real boot
has no durable sessions yet. Proven by `suspending_a_user_ends_their_schedule_and_refuses_them_until_resumed`
and `a_suspended_identity_is_not_re_derived_at_boot` (retired with `session_reviver`), each
falsified by hand on aarch64.

## What was built (2026-09-27, boot re-derivation moves into `login`)

To calef's ruling of Fork 7, option A. `components/src/login.rs`'s `rederive` runs once in `_start`,
before the front door opens. For each identity the manifest names and the suspended list does not,
it opens a session with `Durable::open` and puts the stored document in force with
`Durable::restore`, the calls a login makes. A refused document ends the session and keeps the
manifest line, so its user sees the verdict at their next login. Only an authenticated login is
handed the session.

- The cap, `DURABLE_SESSIONS`, is the fewer of `login_protocol::durable::sessions_held` over
  `abi::CAPABILITY_TABLE_SLOTS` (one at 24 slots, three at PR #1360's 32) and one budget of memory.
  The arithmetic and the skip are host-tested there.
- `session_reviver`, its spawner, its tests and `fs_test_client`'s seed and verify roles are gone.
- Proven by `login_tests`' `a_durable_session_is_re_derived_at_start_up_unless_suspended`, falsified
  twice on aarch64 (a pass ignoring the list, and no pass at all).
- §123 is AMENDED. Fork 4 closes as moot.

Provisional names: `login_protocol::durable` and its items, `rederive`, `Durables`,
`DURABLE_SESSIONS`, `fs_service::set_home_file`, and the test.

## What was built (2026-09-27, forks 6 and 8)

To calef's rulings on #1377. **Fork 6 C:** a durable job holds no report endpoint. The session
process places none; the timetable probes the slot and hands jobs what it holds
(`timetable::Held::report`), and the plan lists no endpoint (`F_NO_REPORT`). **Fork 8 D:** a
durable timetable holds no archive. It holds read-only caretakers for `activation/` and `packages/`
over the file service's last window, reserved for `login` (milestone 599 (a frame per filesystem
client channel)), since its user may hold window 0. It resolves each entry as the prompt resolves a
bare name, and plans each fire against the current version's manifest (amended by calef on #1377).
`login` gets `session` and `timetable` as two blobs, and builds the store caretakers first and the
client's last. The real boot hands all of it over, `login`'s budget sized by
`login_protocol::durable::BUDGET_PAGES`.
The [fork 8 appendix](../../notes/durable-delegation/which-programs-a-job-runs.md) has the costs.

**Not the slot table: 64-KiB staging refused aarch64's fixture (2026-10-02).**

## Forks this lane found, for an architect

Options and reasoning are in [notes/durable-delegation.md](../../notes/durable-delegation.md).

1. Who keeps and supervises a durable session. Ruled S1 (calef, 2026-09-26); built.
2. §108 (disabling credentials kills the durable session) has no trigger. Ruled 2026-09-26:
   `user suspend` and `user resume` at the owner console, the mark kept as a list file; built.
3. A scheduled job never holds the run-unvouched capability, so the key-trust drop of §220 (signed
   builds) reaches it. Built: the timetable refuses to run holding it (milestone 129), and the session
   process never passes it.
4. The per-identity narrowing in §123 (the boot-time re-derivation privilege) belongs with the first
   real consumer, sharing `login`'s `mint`. **Status: CLOSED** 2026-09-27, moot under fork 7's
   ruling: with no separate re-deriver there is no window to narrow.
5. When `login` builds the session process. Ruled L2 (calef, 2026-09-26); built.
7. Boot re-derivation moves into `login`; `session_reviver` is retired. Reasoning in
   [the fork 7 appendix](../../notes/durable-delegation/boot-rederivation-in-login.md).
   Status: DECIDED, option A, calef, 2026-09-27; built the same day.
8. Which programs a scheduled job may run on the real boot. **Status: DECIDED**, option D (the
   live activation generation), calef, 2026-09-27, on #1377; built the same day.
6. Where a scheduled job's report goes once nobody is attached. **Status: DECIDED**, option C (no
   report endpoint; output through the entry's grants), calef, 2026-09-27, on #1377; built.

## BUGS

- ~~`smb_server` has no session/connection separation to build this against.~~ Built 2026-08-24,
  then removed with the SMB code on 2026-08-30; the proof is re-homed (2026-09-26).
- ~~The on-disk, per-user schedule store has no format, no write path, and no read-at-boot path.~~
  Built 2026-08-24; see `crates/schedule_store`'s module doc, §122 (the on-disk schedule store) and §125 (which identities have pending work).
- ~~Boot-time re-derivation's own mechanism was asserted, not designed.~~ Built 2026-08-24 as
  `session_reviver`, and moved into `login` on 2026-09-27 (Fork 7); see `components/src/login.rs`'s
  `rederive` and BUGS, and §123 as amended.
- ~~#387 (milestone 129's `--mem` grant): no scheduled job is registered against a real session.~~
  Built 2026-09-26: a user's session registers through `SCHEDULE` and the registration page.

## Follow-on

- **Done.** Registration persists: the client writes the identity's schedule file before it
  replaces, `login` keeps the manifest, and `login`'s start-up pass opens real sessions from both
  (2026-09-26 and 2026-09-27).
- **Done.** The real boot hands `login` what Fork 8 D needs, 2026-09-27.
- **Milestone 769.** Milestone 769 (a gate that types SCHEDULE on the real boot). A gate that types `SCHEDULE` on the real boot: `script/swish-check` has no login
  client, so only the kernel suite's `login_tests` run it. Rechecked 2026-10-03. See
  `design/roadmap/0769-a-real-boot-gate-that-types-schedule.md`.
- **Refused.** Per-login narrowing of the directory capability was deliberately not taken, because
  the adapter it applied to was deleted: the SMB implementation went on 2026-08-30, calef's call,
  after journey 2 was retired.
- **Done.** Reattachment: `login` keeps the durable session and hands it back (2026-09-26). It
  keeps one; `components/src/login.rs`'s BUGS says why.
- **Refused.** Per-identity narrowing of the re-deriver's filesystem endpoint (§123's first
  hardening refinement): moot once `login` re-derives, since `login` holds the unnarrowed root for
  its whole life anyway. Fork 4, closed 2026-09-27.
- **Done.** `LOGIN_CONSTRUCTION_PAGES` holds a durable budget, derived from
  `login_protocol::durable::BUDGET_PAGES` (2026-09-27).
- **Milestone 687.** A durable job has nowhere to write yet: its timetable holds no directory to
  back an entry's grant, so such a line is planned unbacked. Milestone 687 (the system log persists
  through RedoxFS: what a directory grant has to answer) is the likely grant.
  `components/src/timetable.rs`'s BUGS has the store-mode limits.
- **Recorded.** No liveness watchdog for the start-up pass. It is bounded instead: each re-derived
  session waits at most `END_WAIT_SECONDS` for its timetable, before the front door opens.
  `components/src/login.rs`'s BUGS.
- **Recorded.** `login` keeps one durable session, at start-up and at login alike. The table limit
  derives from `abi::CAPABILITY_TABLE_SLOTS` and rises to three with PR #1360's 32 slots. The
  memory limit stays one budget until `login`'s `DURABLE_UT_PAGES`, the progenitor's
  `LOGIN_CONSTRUCTION_PAGES` and the harness are raised together and the out-of-order hole in
  `durable_ut` is fixed. `components/src/login.rs`'s BUGS.
- **Done.** `session_reviver` is retired and the real boot's `login` carries the start-up pass.
  2026-09-27.
- **Done.** The manifest question is settled: `design/decisions/0125-durable-schedule-manifest.md`
  is DECIDED, ratified by calef on 2026-08-25, and its own text notes the recommended shape was
  already built rather than merely proposed.
- **Done.** The §16 live-children proof is re-homed on a real login session's budget
  (`a_login_session_with_pending_work_refuses_logout_until_the_work_is_gone`), and the six
  `session_reviver.rs` comments that cited the deleted type now cite it. 2026-09-26.
- **Done.** DECISIONS §108's cascade, as `user suspend`/`user resume` (fork 2). 2026-09-26.
- **Recorded.** Ending a durable session waits for a running job to finish, and `SUSPEND` waits
  while a session holds the terminal; `components/src/login.rs`'s BUGS has both.
- **Done.** No scheduled job holds the run-unvouched capability (fork 3). 2026-09-26.

## Index row

calef wants a scheduled job's capabilities to reflect the scheduling user's own authority
(milestone 129's #387), so the registrar is a user's session. Its authority must outlive the
connection that registered it, which DECISIONS §92 (a caretaker is supervised by the client it
serves) does not allow. Built: the on-disk schedule store (§122), boot-time re-derivation (in
`login` since 2026-09-27, §123 as amended, §125), and the §16 live-children proof on a login
session's budget (2026-09-26). The session process is ruled (S1) and built, as are forks 6 (C)
and 8 (D), ruled on #1377. A durable job holds no report endpoint and runs what the live activation
generation names, read over a file-service window of its own.
