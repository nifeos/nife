---
status: DECIDED
raised: 2026-09-27
decided: 2026-09-27
ratified_by: calef
---

# 243. Notices for people: programs publish, users curate, displays show

*Section number provisional: §241 (threadbare) is claimed by #1421 and §242 (a system log) by
#1423, so this took
243 on 2026-09-27 (UTC) and may move at merge. The slug and every wire value and constant stay
provisional; the name and the service, ratified below, do not.*

Raised 2026-09-27 (UTC) by calef in the maintainer session: *"We want components to be able to
publish notifications for systems users. We want users to be able to curate the types of
notifications they receive. We want different displays to be able to display the notifications."*
His examples: a notice in every session that the system is going down for reboot, an update poller
telling an appropriate user that packages are out of date, and a background job that ended.
Written by the lane `proposal/user-notifications` (pull request #1424), which builds nothing.

## The ruling

calef ruled all six questions below on 2026-09-27 (UTC), each as a maintainer comment on pull
request #1424 (`gh pr view 1424 --comments` has the full text). §243 moves to DECIDED, and the
sections that follow keep each question's reasoning with the ruling folded in.

1. **A.** A separate service, `notice_board`, which also writes each notice to §242's system log
   as history. Retention decided it: the log's ring would drop a notice before an absent owner
   reads it.
2. **N1, ratified.** The item is a *notice*, the service `notice_board`, the contract
   `notice_protocol`.
3. **Yes, as recommended.** A publisher holds a badged endpoint; its spawner registers the badge's
   program, audience and bypass right, and the board stamps the rest. Only a bypass-registered
   badge may publish critical.
4. **W1 and R1.** Publish is the 16-byte binary header plus class, key and text; read is JSONL,
   the same split §242 ruled for the log the same day.
5. **Four levels.** low, normal, high and critical, kept separate from §242's severity; the board
   writes both into the log record.
6. **A.** A critical notice prints at the next line boundary over a full-screen program, as `wall`
   does, accepting a corrupted screen for critical only.

## The name comes first, because the obvious one is taken

"Notification" is a kernel object: §101 (notification objects), milestone 151 (notification objects), `objtype::NOTIFICATION
= 4` in `crates/moho_abi/src/lib.rs:512`, with `SIGNAL`, `WAIT`, `POLL` and `BIND`, and
`Received::Notification` in `crates/user_mode_runtime`. The design below uses that object as a
display's doorbell, so the two would meet in one sentence, and the person-facing thing needs
another word. Names are calef's call (`design/naming.md`, CLAUDE.md's naming authority); these were
the options:

| option | concept, service, contract crate | collides with |
|---|---|---|
| N1 | notice, `notice_board`, `notice_protocol` | syslog's severity 5 is named "Notice" (RFC 5424), and §242 ruled syslog's eight levels. It is a value in one field, not a type. As a noun it is unused in code; the 41 hits are the verb. |
| N2 | bulletin, `bulletin_board`, `bulletin_protocol` | nothing in the tree or in RFC 5424; reads as a broadcast, which two of three examples are not |
| N3 | memo, `memo_board`, `memo_protocol` | nothing; reads as addressed to someone, which fits, and as informal |
| refused | alert | RFC 5424 severity 1, "action must be taken immediately"; a job that ended is not an alert |
| refused | message | an IPC message is `Received::Message` beside `Received::Notification` |

Ratified: N1. It parses without Unix exposure, which `design/naming.md` asks of a component name,
and its collision is a severity value rather than a second kind of object. This file uses "notice"
throughout.

Name: ratified 2026-09-27 (calef, pull request #1424, Q2); why the runners-up lost is in the
table above.

## What the tree has today

Nothing person-facing. Lookups on base `995ac5bce`:

- No `motd`, banner, `wall` or broadcast concept exists.
- No program reboots the machine, so the first example has no publisher yet. Proposed:
  `design/roadmap/0688-build-a-reboot-program.md`.
- No update poller. §241's P5 (an OS update is a set of packages through a slot) is unbuilt.
- A job's end is already observed. On #1377 the session process "blocks on one endpoint that
  carries both the timetable's death and every job's report" and, until milestone 152 (durable
  delegation)'s Fork 6 is built, "receives and drops reports".
- One interactive session at a time: `login` holds a single `terminal_held` flag
  (`components/src/login.rs:159`) and refuses a second login with `NO_TERMINAL`.
- Nobody may list sessions. "List sessions and match" was refused as "enumeration is authority"
  (`notes/durable-delegation.md#1-who-keeps-a-durable-session-nameable-after-its-client-leaves-reattachment`,
  milestone 126 (who else is running)).
- Per-user state lives at `<principal root>/<identity>/`, beside `schedule`
  (`crates/schedule_store/src/lib.rs:79`). There are no home directories (`notes/credentials.md#bugs`).
- The compositor draws a fixed scene of three windows with `MAX_WINDOWS = 4`
  (`crates/compositor/src/lib.rs:160`) and has no overlay or popup
  (`notes/compositor.md#what-this-does-not-do-that-a-real-compositor-would`).

## Prior art, read

Read from primary sources by this lane; "(memory)" marks what was not verified.

| system | who names the sender | who controls loudness | replace or dedupe | no display attached |
|---|---|---|---|---|
| freedesktop Notifications (D-Bus `Notify`) | the caller's own `app_name` string | the sender's `urgency` hint: low, normal, critical | `replaces_id` from an earlier `Notify` | not handled; a `persistence` capability is optional |
| Apple UserNotifications | the app bundle | per app; `interruptionLevel` passive/active/timeSensitive/critical, critical needs an entitlement | a request with the same identifier replaces the pending one | not handled |
| Android channels | the package | the user, per channel, fixed once the channel is created | same `notify()` id updates; `setOnlyAlertOnce`; `setTimeoutAfter` is a TTL | not handled |
| `wall`, `write`, `mesg` | the kernel's uid | the recipient, with `mesg n`; "only the superuser" writes past it | none | lost |
| systemd `shutdown` and logind | the privilege to power off | none; `--no-wall` is the sender's | warnings repeat as the deadline nears (`logind-wall.c`'s interval table) | lost |
| Ubuntu `update-notifier` and `update-motd` | root scripts at login | none | regenerates only when the package lists or dpkg status change | the next login's message of the day |
| Fuchsia | | | | no user notification library found in the FIDL index |

What carries over: Android's user-owned channel is curation by (program, class); Apple's
entitlement makes bypass a granted right; `update-notifier` republishes only when its inputs change.
Every desktop system assumes an attached display, so the headless case has only `motd`.

## Question 1: a separate service or a view over the log

Priced both ways. A view over §242 would make a notice a log record with a new body kind (F3 has
the byte for it), needing four changes to §242, each measured against what was ruled today:

- Audience. The log's badge maps to (program, user, process); a notice also needs an audience, so
  registration grows a field.
- Per-user read. §242's scope filters by who wrote; a notice is read by whom it is addressed to,
  a second filter axis.
- State. A notice is replaced, dismissed and expires; an append-only ring expresses those only as
  tombstones, which every display must fold back into state.
- Retention, the decisive one. The service's 64 KiB ring holds about 24 minutes of a chatty
  reporter (§242's own figure), so a job that ends at 03:00 is evicted before its owner wakes.

A notice must outlive the traffic around it, and a log is sized by traffic.

**Decided: a separate service, `notice_board`**, that holds the live notices as state and appends
every accepted notice to the log as history. The board is one more writer with its own badge, so
this costs the log nothing, and its records carry an `audience` field, which calef's JSONL ruling
already allows ("structured fields can join the object later").

Refused: folding the board into the log service as a second front door. It saves a process and
couples a fan-out service to the one calef just ruled must fall back to the UART when it wedges.

Refused: writing into each session's terminal, as `wall` does. That needs every terminal, which
is enumeration, and has no curation.

## Question 3: the capability shape

**Decided, as recommended:** a publisher holds a badged endpoint to the board, and the badge names
its audience. The writer never states its audience, program or time.

- Minting mirrors §242: whoever spawns a publisher holds the board's unbadged endpoint, mints a
  badged copy (`rendezvous::BADGE`, §230 (badged endpoint capabilities)) and registers it, badge to
  (program digest, audience, bypass); the board stamps the rest.
- Three audiences: one identity; the owners (§221 (the boot prompt is the owner's console)'s
  owner-written list plus the console owner); and every attached display.
- `login` registers per-user badges. It mints one publish badge (audience: that identity) when it
  builds a session process, and one read badge per login.
- The spawner of a system service registers the others. The reboot program (proposed,
  `design/roadmap/0688-build-a-reboot-program.md`) gets "every display, may bypass". The
  update poller gets "owners".
- A user cannot address another user; `write(1)` has no analogue until someone asks for one.
- The board keeps a list of who is subscribed, not who is logged in: a display subscribes with its
  read badge, so "every display" is the board's own subscriber set, and no publisher learns who
  else is on, keeping "enumeration is authority" intact.
- Bypass is registered, never sent: only a bypass-registered badge may publish at critical urgency
  and the board refuses critical from any other, Apple's entitlement as a capability, and the
  analogous case in the tree is §221's owner-written list.

A display is woken with a §101 notification object: it hands the board one with `SIGNAL` right
when it subscribes, and binds it (`BIND`) so a new notice wakes it out of `RECV`. That is the
tree's doorbell, "a doorbell, not a meeting" (`crates/moho_abi/src/lib.rs:520`).

## Curation

Recommended: a per-user document, stored at `<principal root>/<identity>/notices` (provisional)
beside `schedule`, written by the user's own session and handed to the board whole, §222 (who
holds a user's schedule)'s shape: one document in one shared page, parsed whole or refused.

- A rule is (program, class, least urgency shown). Program is the board's stamp, class is the
  publisher's label (`job.ended`, `package.updates`). The pair is Android's channel.
- A publisher cannot change a user's rule. A rule can hide a class, lower it to the list only,
  or raise it. It cannot silence critical.
- Defaults are compiled into the board: show normal and above, keep low in the list, always show
  critical. Before a user's document arrives, or when it is absent, the defaults apply.
- Whoever can write that identity's subtree may change it. Nothing new.

## Question 5: the urgency scale

**Decided: four levels**, low (listed, never presented), normal (presented), high (presented and
kept until dismissed) and critical (presented, cannot be curated away, bypass right required). They map
onto Apple's four and onto freedesktop's three with high folded into critical. Android's five include
`NONE`, which is a curation outcome rather than something a sender asks for.

Freedesktop's three would lose the difference between "keep this until I see it" and "this
overrides my settings", which the update and reboot examples need.

Urgency is not §242's severity. A job that failed is severity error and urgency normal; a reboot is
severity notice and urgency critical. The board writes both into the log record.

## Delivery

Recommended:

- Held until dismissed or expired, in memory, each with a publisher-set time to live capped by the
  board (7 days, provisional). Nothing persists across a reboot; the log's RedoxFS half, when
  built, holds the history.
- Replacement by (program, key): a same-key notice replaces the old one (`replaces_id`, Android's
  id), scoped by the stamped program so no program can replace another's.
- No nagging. A byte-identical replacement does not re-alert (Android's `setOnlyAlertOnce`); a
  dismissed key stays dismissed until its text changes, tracked by an 8-byte digest.
- Retraction. A publisher may withdraw its own key, how a cancelled reboot disappears.
- Dismissal is per user, across all of that user's displays.
- At least once per display, which reads from the last sequence number it saw, so a late attach
  gets the backlog.
- No attached display: the notice waits in the user's table until it expires. A table holds 32
  live notices per identity (provisional); overflow drops the oldest lowest-urgency one and counts
  it.
- Refused for now: actions (freedesktop's `ActionInvoked`), a reverse capability to the publisher
  and a design of its own.

## Question 4: the formats, which are irreversible

Two surfaces become wire formats.

| surface | who agrees on it | options |
|---|---|---|
| publish, publisher to board | every program that publishes | W1: a 16-byte binary header (urgency, flags, class length, key length, time to live in seconds, spare), then class, key and text as UTF-8, 224 bytes in total, in a shared page like §222's `REPLACE`. W2: one JSON object per notice, parsed by the board. |
| read, board to display | every display, including a remote one | R1: JSONL, one object per notice, with the board's stamped `seq`, `time`, `program` and `audience`, then `class`, `key`, `urgency`, `expires`, `text`. R2: the binary record as stored. |

**Decided: W1 and R1.** §242's split, ruled the same day for the log: binary where programs write,
JSON where people and tools read, an in-tree JSON writer with no dependency, §46 (thin primitives
or whole subsystems). W2 would put a JSON parser in the board, where every publisher's bytes reach
it, and make every publisher link a JSON writer; R2 would make the remote client a second binary
parser for nothing. W1's 224-byte total means a notice's text always fits F3's text bound when the
board logs it.

## Displays

Recommended, and reversible:

- A console line in a swish session. Before each prompt, swish reads new notices and prints one
  line each, how Bash reports a finished job before the prompt (memory), costing one read badge.
- Critical on the console, which cannot wait for a prompt when a full-screen program holds the
  terminal. **Decided (Q6): the terminal server prints it at the next line boundary**, accepting
  what `wall` accepts, a corrupted screen, for critical only; prompt-only was refused, since a user
  inside an editor would miss the reboot.
- A compositor toast: a small client with the user's read badge, drawn last in the one spare
  window of four.
- A remote client later reads the same JSONL with a read badge carried off the machine.

## The three examples, end to end

A reboot. A reboot program (proposed, `design/roadmap/0688-build-a-reboot-program.md`; name
unminted) holds a badge registered "every display, may bypass". It publishes class
`system.shutdown`, key `reboot`, urgency critical, time to live ending at the deadline: "Rebooting
at 14:05 UTC". The board signals every subscribed display's doorbell; the console's terminal prints
it at the next line boundary and the toast shows it. The program republishes under the same key as
the deadline nears, logind's repeating wall without its interval table in the protocol. A cancel
withdraws the key.

An update poller, badged "owners", computes the set of out-of-date packages each hour and publishes
class `package.updates`, key `updates`, urgency normal only when the set's digest changed,
`update-notifier`'s rule. An owner who dismisses it is not told again until the digest changes; a
non-owner never receives it.

A job that ended. The timetable reports the job's exit to the session process, which already
receives that report (#1377), and it holds the publish badge `login` registered for its identity.
It publishes class `job.ended`, key the entry's name, urgency low on success and normal on failure.
An absent user gets it in their table; at their next login swish prints it before the first prompt.
A user who wants only failures sets `job.ended`'s least urgency to normal.

## What it costs

Derived, nothing is built or measured.

- Slots. `login` gains one (the board's unbadged endpoint). Its own comment counts eight at rest
  and six more at a login's peak (`components/src/login.rs:367`), against a table now of 24. A
  session process, swish and each display gain one badge. The board holds one doorbell per
  subscribed display. The spawner of system services gains one too. If that spawner is the
  progenitor, the table is at 23 of 24 (`kernel/src/cap.rs:309`) once §242 takes its own slot. A
  progenitor-registered system publisher, such as the reboot program if it spawns there, is
  blocked until pull request #1360 raises `CAPABILITY_TABLE_SLOTS` to 32.
- Pages. A stored notice is at most 272 bytes (48 stamped: sequence, time, program, audience,
  expiry, digest; 224 the publish record). A 32-notice table is 8,704 bytes, three pages.
- Per notice. W1 is one `CALL` carrying a page; at most hourly in every example, so the per-notice
  time is irrelevant against the 350 ns IPC round trip (`notes/benchmarks.md`).

## The seven questions

In [`seven-questions.md`](0243-notices-for-people/seven-questions.md).

## What it unblocks, and what remains

All six questions are decided, which unblocks building `notice_board` and `notice_protocol`. It
gives milestone 152's dropped job reports a consumer and gives §241's P5 a way to tell an owner.
Two things still gate a shipped publisher, each with a home: the first example's publisher is
proposed at `design/roadmap/0688-build-a-reboot-program.md`, and a progenitor-registered
system publisher waits on pull request #1360 (see "What it costs"). Curation and Delivery can be
built as recommended.
