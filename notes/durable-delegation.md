# Durable delegation: what holds a session up, and what is still waiting

Milestone 152 (durable delegation: authority that outlives the session that requested it) wants a
user's scheduled job to keep firing after the user disconnects, on the authority they held when they
registered it. The design is in `design/roadmap/0152-durable-delegation.md`. This note is the working
record of the 2026-09-26 (UTC) lane: what it proved, and four questions it found that are an
architect's to answer before anything else here can be built.

Name: provisional, minted by that lane on 2026-09-26, after the roadmap slug.

## What holds a session up

The design's first piece claims no new mechanism is needed. A job's authority is a child region
split off the session's budget, and DECISIONS §16 (object revocation) already refuses `MemoryRegion::DESTROY` on a
parent with a live child. That was proven on `smb_server`'s `DurableSession`, which went with the
SMB code on 2026-08-30, leaving a synthetic copy inside `session_reviver` until that program was
retired on 2026-09-27 (fork 7).

It is proven again on the object a login session actually is:
`kernel::user::login_tests::a_login_session_with_pending_work_refuses_logout_until_the_work_is_gone`.
`login_test_client`'s `PENDING_WORK` behavior (provisional) logs in, splits a one-page child off the
budget `login` delegated, and attempts the logout. The budget's `DESTROY` is refused `NotPermitted`,
the budget keeps working, and once the child is destroyed the logout completes in the order
`LOGOUT` uses, with the same proofs that both capabilities came down. It runs wherever the rest of
`login_tests` runs, which is all three architectures.

**The durable half of a login session is its budget, not the whole session.** `login`'s `mint`
builds two sibling regions off its own construction budget: `region`, which holds the directory
caretaker and is the logout ticket, and `budget`. Only the one a job is split from is held up. The
logout ticket still destroys the caretaker while a job is pending, and nothing refuses it. That is
the right split: the interactive half ends at logout, the scheduled half does not. It does bind
whoever builds the registrar. **A job's directory has to be built inside the job's own region**, the
way `mint` builds a caretaker. Borrowed from the session's caretaker, it would die at logout.

## Four questions, each blocking something

### 1. Who keeps a durable session nameable after its client leaves (reattachment)

Today `login` delegates the budget and then `capability_delete`s its own copy. When the client process
exits, the budget still exists, because it belongs to `login`'s construction region. But nothing can
name it. It is the orphaned session the design warned about, and a reconnect mints a second one
beside it. Something has to keep one capability per durable identity.

| Option | What it costs | Verdict |
|---|---|---|
| A. `login` keeps a `WRITE` copy of each budget, keyed by identity | one capability slot per durable identity in a 24-slot table | recommended |
| B. `credentialer` keeps it | makes the secrets service hold authority over sessions; it has no other session state and is sealed after provisioning | refused |
| C. the scheduler keeps it, since it holds the jobs anyway | the scheduler becomes the directory of who has a session, and reconnect becomes a call into it | viable, second |
| D. list sessions and match | enumeration is authority (milestone 126 (who else is running)) | refused |

Under A, `login` needs no new state to tell the cases apart, because the kernel already answers
them. On a successful authentication for an identity that has an entry, `login` tries `DESTROY` on
its copy:

- Success: the session was abandoned with nothing pending. Its pages come home, which also closes
  today's leak of sessions nobody logged out of, and a fresh session is minted.
- `NotPermitted`: either the session has pending work, or the client logged out and the copy is
  stale. The first draft of this note said the kernel tells those apart. It does not: the
  `DESTROY` handler in `kernel/src/syscall.rs` maps every refusal from `sched::reclaim_region`,
  a dead name included, to `NotPermitted`. A one-page `SPLIT` separates them, since it succeeds
  on a live budget and fails on a dead name. The probe destroys that page at once, then reattaches:
  it hands back the same budget, with a freshly built caretaker for the directory. Regions are
  generational (notes/generational-names.md), so a stale copy never names a reused slot.

One caution goes with A. The probe destroys a session that has no pending work. Under today's
single-terminal boot a second login for an identity only happens after the first has freed the
terminal, so a live, working session is never probed. A multi-session boot would need the probe to
run only for sessions whose client is gone, and nothing today can tell a gone client from an idle
one (`login.rs`'s own BUGS, on having no wait-any primitive).

Keeping the copy is also what makes DECISIONS §108 (disabling credentials kills the durable session) possible at all: whoever holds it is the only
party that could tear a durable session down on a user's behalf.

**Would we choose A if C cost the same?** Yes. A keeps "prove who you are, get back exactly your
thing" in the one process that already does the proving. C splits that across two programs and
needs a lookup opcode in the scheduler, which is a wire format.

**Blocked on this:** reattachment, and any real registrar, because a registered job would otherwise
hang off a budget nobody can name after the registrar's client exits.

### 2. DECISIONS §108 has no trigger, and a reboot resurrects what it would kill

§108 says disabling a user's credentials kills their durable session. There is no way to disable a
credential. The credential store is sealed after provisioning and has no record-removal operation
(`notes/credentials.md`, BUGS: "revocation is per holder, not per secret"), and §108 leaves the
mechanism to milestones 56 and 49, neither of which has built one.

The second half matters more than the first. The credential store is memory only and reprovisioned
every boot, while the schedule store is on disk: DECISIONS §122 (the on-disk schedule store) and
§125 (which identities have pending work). `login`'s start-up pass (fork 7) re-derives every
identity the manifest names without asking the credential service whether that identity still
exists; it asks only at the login that is handed the session. So once disabling is built, a disabled user's jobs come back at the next reboot unless one of
two things is true:

- Disabling also removes the identity from the manifest, in the same act.
- The re-deriver checks each identity against the credential service before re-deriving it. That
  needs an "is this identity provisioned" question the credential protocol does not have, which is a
  wire format.

The first is one write and no new protocol; the second survives a manifest someone edited by hand.
Both are an architect's call, and neither can be built before a disable operation exists.

### 3. A durable job and the right to run unvouched code

Every login session is handed the run-unvouched capability. That is gate D2 of DECISIONS §219 (how
the shell names an installed program to the spawner), and `login_contract`'s BUGS say why every
session gets it today. The revocation ruling of §220 (signed builds) drops a distrusted key's programs
to unvouched, "runnable only by a session holding the D2 capability". If a registrar passes a
session's D2 capability into a scheduled job, that job keeps running a program whose key was
revoked, on every fire, with nobody at a terminal to notice.

**Recommendation: a scheduled job never holds D2.** It runs vouched programs only. §220's drop
does not reach a job through the progenitor's installed-programs lookup: the timetable loads a job from an archive `login` checked
at start-up, and the progenitor never sees a fire. That path exists only if fork 8 resolves jobs through
the installed-programs table. A user who wants to schedule an unvouched
build vouches it first. This is a grant rule for the registrar, so it is recorded here for whoever
builds it; the milestone 129 (scheduled execution) lane working the scheduler in parallel is the likely consumer.

### 4. Where the re-deriver's per-identity narrowing belongs

**Closed 2026-09-27 as moot**, by calef's ruling of fork 7: there is no separate re-deriver whose
window could shrink, and `login` already holds the unnarrowed root for its whole life. The
question as it stood:

The first hardening refinement of DECISIONS §123 (the boot-time re-derivation privilege) asks the re-deriver to narrow its store-read capability
per identity. Inside the re-deriver alone that cannot shrink anything. To build a per-identity
caretaker it has to hold the unnarrowed endpoint the caretaker is built from. A directory handle is
a name on that shared endpoint rather than a capability, so it cannot be kept once the endpoint is
dropped. What does shrink the window is building every identity's caretaker first,
deleting the store-read capability, then processing and destroying one caretaker at a time.

That caretaker is exactly the directory a re-derived session would be handed, which is what
`login`'s `mint` builds. So the refinement should be built when the re-deriver hands a real session
to a real consumer, sharing `mint`'s construction. Built now, it would be up to eight processes per
boot whose only purpose is to be destroyed, in a process §123's third refinement asks to keep
minimal. The same reasoning kept `session_reviver` out of the real boot
(`crates/system_initializer`): until a scheduler received what it re-derived, wiring it in added a
privileged process to every boot that produced nothing anyone held.

## The schedule ruling, and what "the session supervises" needs

calef ruled on 2026-09-26 (16:37 UTC) that a user's schedule is held by a timetable their durable
session spawns and supervises, changed by whole-document replace. The record is on branch
`maintainer/129-decision`, its section number provisional until it merges. It fits what is proven
here: a timetable split off the budget is a live child, so it holds the session up the way
`PENDING_WORK`'s child does.

It also turns question 1 into a sharper one. "The session supervises" needs a running thing that
holds the timetable's supervision endpoint and blocks on it. A process has one wait point and there
is no non-blocking receive in the ABI, so the candidates are few:

| Who supervises | Verdict |
|---|---|
| S1. A per-user session process `login` builds from the session budget; it builds the timetable, hands its replace endpoint out, then blocks on the supervision endpoint | recommended |
| S2. `login` itself | refused: it blocks on its front door and would never read a report |
| S3. The client holding the session, such as the shell | refused: it exits at disconnect, which is the problem being solved |
| S4. Nobody live: the budget owns the timetable's region, and faults wait unread | refused: a dead timetable stops a user's jobs silently |

Under S1, `login` keeps two capabilities per durable identity: the budget, for question 1's probe
and for DECISIONS §108 (disabling credentials kills the durable session), and the timetable's replace
endpoint, which a reattaching client is handed. The session process is one more process per
scheduling user beside the timetable, and it blocks rather than yields, so it costs memory and no
CPU. It exits when its timetable does, which answers the last point: a timetable whose document is
empty should exit, so the session becomes destroyable again and nothing outlives its reason to
exist.

calef ruled S1 on 2026-09-26. The program ships as `session`, a provisional name. (calef ratified
`user_timetable_keeper` in its place on 2026-10-06 UTC.)

### 5. When a session becomes durable, and what logout then means

S1 says who supervises. It does not say when `login` builds the session process, and every answer
changes what `login` does for users who never schedule anything:

| Option | What logout does | Verdict |
|---|---|---|
| L1. At every login | detaches; the session and an empty timetable persist until a new end-session request or §108 | refused: one parked process pair per user who ever logged in, and today's logout tests change meaning |
| L2. On request: a new request on the private channel after `OK` builds the session process and returns the replace endpoint | unchanged for a session with no schedule; a scheduled one survives because its session process is a live child | recommended |
| L3. When the identity's stored schedule is non-empty | as L2 | not enough alone: the first registration still needs L2's request |

calef ruled L2 on 2026-09-26. The request word's value and name are provisional.

L2 is what the design block already decided in words: a session persists because it has scheduled
work. It needs one request word, which is a wire format and so calef's, and it hands the replace
endpoint back the way the run-unvouched capability is handed back today, as an announced extra
capability. It also asks two things of milestone 129's timetable. It should exit when its document is
empty, so the session process exits and the budget becomes destroyable. And the plan should travel
in the replace reply rather than on an output endpoint, because the session process blocks on
supervision and cannot also read a stream.

### 6. Where a scheduled job's report goes once nobody is attached

**DECIDED: option C** (the third below), calef, 2026-09-27 (UTC), on #1377; built the same day.

Today every scheduled child is handed the timetable's child-report endpoint and blocks sending its
answer until someone receives it; in `timetable_tests` that someone is the kernel harness. Under S1
the only process left for it is the session process, which already blocks on the timetable's
supervision endpoint and has one wait point. So a durable session has three streams (the plan, job
reports, the timetable's death) and one reader. Options:

- Route job reports and the timetable's death to one endpoint the session process reads, and tell
  them apart by the death message's form. Reports are then counted or dropped, which is a recorded
  limitation rather than an output story.
- The session appends each report to a log in the identity's own subtree, which ruling 5 of the
  schedule decision already lets the session reach. That is cron's mail, with a directory
  capability in place of `sendmail`.
- A job holds no report endpoint at all, and writes its own output through a directory grant in its
  entry, as any other granted program would.

The third needs nothing new and fits the claim that an entry holds only what its line grants, so
it was the recommendation.

Question 3's rule applies to the `Held` a session hands its timetable.

## What was built to the rulings, and what it found (2026-09-26)

S1, L2 and the suspension ruling are built (the block's "What was built" entries). Three findings
from building them, each recorded where a reader meets it:

- The ruling put L2's request after `OK`. It is the request itself (`login_contract::SCHEDULE`),
  because `login` blocks on one endpoint and cannot wait for a word after `OK` without every client
  sending one.
- Reclaiming a session process too early kills it between its two teardown destroys and strands a
  region under the user's budget. It now says `STOPPED` and `login` waits for the word.
- "The stored schedule resumes at the next login" makes `login` read an identity's stored schedule
  at login time. Doing the same at start-up is boot re-derivation, which is why the block proposes
  moving it into `login` (fork 7).

### 7. Where boot re-derivation runs

**DECIDED: option A.** calef, 2026-09-27 (UTC): at start-up `login` re-derives every durable session
that is not suspended, using the code that restores a stored schedule at login, and
`session_reviver` is retired. DECISIONS §123 carries the amendment. Built the same day:
`components/src/login.rs`'s `rederive`, with the capacity in `login_contract::durable` (both
provisional names), proven by `login_tests`'
`a_durable_session_is_re_derived_at_start_up_unless_suspended`.

The options table, the authority each holder has, the prior art and the measured costs are in
[the fork 7 appendix](durable-delegation/boot-rederivation-in-login.md). In short: `login` already
holds every capability the separate re-deriver of §123 would, so deleting the re-deriver's copies
after one use leaves the machine where `login` already stands. `login`'s 24-slot table holds one durable session (19 + 4N at an ordinary login's peak), so that
is also the start-up pass's limit. The built limit derives from `abi::CAPABILITY_TABLE_SLOTS`
rather than being written as one. It also derives from the memory `login` sets aside for durable
budgets, which is one budget, so a wider table (PR #1360 proposes 32 slots, room for three) moves
only the first. `login.rs`'s BUGS says what raising the second takes.

The appendix also found a latent failure. The real boot gives `login` 768 construction pages, and
with a schedule archive `_start` splits 800 before it serves anyone, so `login` would die at
`fail(2)`. Option A does not make it bite: the start-up pass runs only when an archive is present,
and the real boot still passed none. Fork 8's build raised `LOGIN_CONSTRUCTION_PAGES`.

### 8. Which programs a scheduled job may run on the real boot

**DECIDED: option D**, calef, 2026-09-27 (UTC), on #1377: the live activation generation, what a
bare word runs at the prompt minus D2, checked at each fire. Built the same day, over a file-service
window of the timetable's own. The options, costs and what building it found are in
[the fork 8 appendix](durable-delegation/which-programs-a-job-runs.md).
