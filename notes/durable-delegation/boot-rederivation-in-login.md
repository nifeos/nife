# Fork 7: where boot re-derivation runs

Name: provisional, minted by milestone 152's lane on 2026-09-27.

**DECIDED: option A**, calef, 2026-09-27 (UTC). Built the same day: `login`'s `rederive`, and
`session_reviver` is gone. DECISIONS §123 carries the amendment. What follows is the analysis as
it was presented, kept as the record of why.

Milestone 152 (durable delegation) found that `login` already restores a stored schedule when its
user logs in. Doing the same thing at start-up is boot re-derivation, which DECISIONS §123 (the
boot-time re-derivation privilege) gives to a separate, boot-only process. This appendix to
[notes/durable-delegation.md](../durable-delegation.md) asks which process should run it. It is
written to the seven questions in `CLAUDE.md`. Counts are from the code at `c457dee32`, 2026-09-27
(UTC), unless a line says otherwise.

## The options

| Option | Boot-time holder | New wire format | Durable sessions after boot | Verdict |
|---|---|---|---|---|
| A. `login` re-derives at start-up, before it serves its front door; `session_reviver` is retired | `login`, with the capabilities it already holds | none | one today, `login`'s table (below) | recommended |
| B. §123 as written: `session_reviver` in the real boot hands each session to `login` | `session_reviver`, then `login` | a hand-over channel, four capabilities and a name per identity | one, the same table | refused |
| C. The re-deriver keeps and supervises its sessions for the life of the boot | a second long-lived process beside `login` | a reattach and suspend door `login` calls | about four, a fresh 24-slot table | refused |
| D. Status quo: `session_reviver` stays synthetic | nobody on the real boot | none | none until the user logs in | the answer on a no |
| E. No boot pass at all, and `session_reviver` retired; a stored schedule resumes at its user's next login | nobody | none | none until the user logs in | refused |
| F. The progenitor runs the pass (§123 allowed "a phase folded into `system_initializer`") | the progenitor | B's hand-over, from the progenitor | one | refused |

### What B's hand-over would carry

`login` keeps a durable session as four capabilities and a name (`Durable` in
`components/src/login.rs`). B has to move exactly that across a process boundary:

- a new rendezvous, `session_reviver` holding `WRITE | GRANT` and `login` holding `READ`;
- per identity, one word carrying an opcode and the name's length, and two words carrying the name
  packed as `filesystem_contract::grant::pack_name` packs it (16 bytes at most);
- four `SEND_CAP`s in a fixed order: the user's budget (`WRITE | GRANT`), the session process's
  region (`WRITE`), the registration page (`WRITE`) and the session's readiness endpoint (`READ`);
- a closing word with the count, so `login` knows the pass is over before it opens its front door.

That is a format two programs agree on, which `CLAUDE.md` puts on the expensive side of the line.
It also changes where memory comes home. A budget split from `session_reviver`'s region returns
to that region when `login` retires the session, and nothing can split that region again once
`session_reviver` has deleted its name for it. Each re-derived session that later ends strands
its 640 pages for the rest of the boot. `login`'s own durable budget does not have this problem,
because `login` is the one holding its parent.

B also has to build the session process, so `session_reviver` would need the schedule archive,
the measurement table and a copy of `open_schedule`. §123's third refinement asks the re-deriver
to stay minimal, and this is the opposite.

F is B with the progenitor in `session_reviver`'s place. The progenitor peaks at 23 of 24 slots
(`kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED`), and F needs the budget, the hand-over endpoint and
a session under construction live at once. It was refused on that and on B's grounds.

## 1. What else was considered, and why each lost

- B loses because its only property A lacks is that the re-deriving capabilities are deleted
  after one use. Question 4 shows that property protects nothing on a boot where `login` runs. It
  costs a wire format, stranded memory, and a second copy of `open_schedule`.
- C puts a second process on the machine holding the file-service root and a construction budget
  for its whole life. That is the standing "impersonate any user" power §123 was written to deny,
  held twice rather than once. It needs a door for reattachment and for `SUSPEND`, which is a wire
  format too. Its one gain is room for about four sessions in a fresh table, which question 5
  shows can be bought more cheaply.
- D and E leave a user's jobs silent from a reboot until that user logs in. That is launchd's
  LaunchAgent model, and the milestone exists to do better than it.
- F loses on the progenitor's last slot and on B's format.

## 2. What this tree already does in the analogous case

It does A already, at login time. `serve_login` reads `<identity>/schedule` through
`read_stored_schedule`, splits a budget from `durable_ut`, builds the session process with
`open_schedule`, puts the document in force with `Durable::restore`, and records the identity in
the manifest of §125 (which identities have pending work). That path was built to calef's ruling
on §108 (disabling credentials kills the durable session), "the stored schedule resumes at the next login".

A boot pass is the same calls in a loop over the manifest, skipping identities on the suspended
list. `login` already reads both files: `record_in_manifest` writes the first and `is_suspended`
reads the second. The only thing a login adds that a boot pass lacks is the caretaker `mint`
builds for a client, and the boot pass has no client. That caretaker is built on reattachment, as
today.

The same pass also retires fork 4's question. The per-identity narrowing of §123's first
refinement is a way to shrink a separate re-deriver's window. With no separate re-deriver there
is no window to shrink beyond the one `login` already has.

## 3. Prior art outside the tree

Read on 2026-09-27 (UTC):

- systemd: `loginctl enable-linger` means "a user manager is spawned for the user at boot and kept
  around after logouts" (`loginctl(1)`). `user@.service(5)` says the system manager, PID 1, starts
  every user manager. The process that starts a user's manager at boot is the one that starts it
  at login, and it holds that power for its whole life.
- launchd: system-wide jobs load at boot and run with nobody logged in; per-user agents
  (`LaunchAgents`) load when a user logs in and stop at logout (Apple's launchd documentation).
  There is no per-user-at-boot tier; that is option E.
- cron: `cron(8)` reads every account's crontab from `/var/spool/cron` at start-up and rechecks
  them each minute. From memory, not read: `cron` runs as root and switches to the crontab's
  owner for each command, so one long-lived process holds the power to act as any user.

From memory, not read: Fuchsia's `session_manager` is started by the component manager at boot and
launches one configured session, with no per-user model to compare. seL4's capDL loader
distributes capabilities once and exits, which is §123's shape; it starts a static system, though,
and has nothing that later logs users in.

No system read here splits boot-time start of a user's session from login-time start into two
processes. Where both exist, as in systemd, one long-lived manager does both.

## 4. Is the premise true?

The premise of Fork 7 is that A adds no authority. Checked against both endowments.

`login` holds, for its whole life (`crates/system_initializer`'s build of it, and `login.rs`'s
capability contract):

| Slot | Capability | Rights |
|---|---|---|
| 0 `REQUEST` | the front door | `RECEIVE` |
| 1 `RESULT` | the front door's answers | `WRITE \| GRANT` |
| 2 `VERIFY` | the credential service | `WRITE` |
| 3 `FS_EP` | the file service's root directory | `WRITE \| GRANT` |
| 4 `FS_PAGE_FRAME` | the file service's shared page | `WRITE \| GRANT` |
| 5 `CONSTRUCTION_UT` | its construction budget | `WRITE \| GRANT` |
| 6 `AUDIT` | the audit receiver | `WRITE` |
| 7 `TERM_EP` | the terminal | `WRITE \| GRANT` |
| 22 `RUN_UNVOUCHED` | the run-unvouched capability | `WRITE \| GRANT` |

plus the caretaker's bytes, the measurement table and the schedule archive, mapped read-only.

`session_reviver`, as the kernel harness spawns it, holds `REPORT` (`WRITE`), `UT` (`WRITE`) and
`FS_EP` (`WRITE`, the same root) and one mapped file page. Every capability it holds is one
`login` holds with equal or wider rights. A boot pass inside `login` uses `FS_EP`, the file page,
`durable_ut` (split from `CONSTRUCTION_UT`) and the schedule archive. It takes nothing new.

So the power §123 guards, building any identity's session with no credential presented, is one
`login` has held since milestone 49 (users and attribution). What stops it being used at 3pm is
`login`'s code, not an empty slot. §123 did not compare the re-deriver with `login`, and its
"dies after one use" property was never available on a boot that runs `login`. The one thing A
changes is where the uncredentialed path lives: in a long-lived process, run once before its
receive loop, rather than in one that exits.

Authority per option, at boot and after:

| Option | Holds the file-service root and a construction budget after boot | Processes holding both |
|---|---|---|
| A | `login`, as today | one |
| B, F | `login`, as today; the re-deriver deletes its copies | one after the pass |
| C | `login` and the re-deriver | two |
| D, E | `login`, as today | one |

No option removes `login`'s standing authority, and C adds a second holder. B's deletion leaves
the machine exactly where A leaves it.

## 5. What each option costs, measured

### `login`'s capability table

Counted from `login.rs` against `kernel::cap::CAPABILITY_TABLE_SLOTS` (24, per process). No gauge
reports one process's table; the kernel's `highest_seen` is machine-wide and the test kernel does
not print it. These are counts from the code, and a `login_tests` assertion would make them a
measurement.

- At rest: the nine endowed slots above, plus `own_ut`, `durable_ut` and `channel_ut` from `_start`.
  Twelve.
- Each kept durable session: its budget, its session region, its registration page and its
  readiness endpoint (`Durable`). Four.
- An ordinary login at its peak, inside `mint`'s `build_child`: the channel's `result` and
  `region`, then `region`, `narrow_ep` and `ready`, then the child's address space and one frame or
  its thread. Seven. (`serve_login` deletes `request` and `page` before `mint` runs.)
- A login that opens a schedule, at its peak inside `open_schedule`'s `build_child`: the channel's
  two, `mint`'s returned two, the budget, then `session`, `its_budget`, `page`, `ready` and the
  build's two. Eleven.

With N durable sessions kept, an ordinary login peaks at 19 + 4N. One session reaches 23 of 24;
two need 27. The first durable open also peaks at 23. So one is the ceiling today, and the lane's
report of it was right.

At boot under A there is no channel and no caretaker, so re-deriving the k-th identity peaks at
12 + 4(k - 1) + 7, which fits two. But the first ordinary login after that would need 27, so the
boot pass must stop at one as well.

### Widening the table

`CAPABILITY_TABLE_SLOTS` is one constant for every thread. A slot is 32 bytes, asserted in
`kernel/src/cap.rs`, and `kernel::sched::MAX_THREADS` is 256, so each slot added costs 8 KiB of
kernel memory. Holding N durable sessions needs 19 + 4N slots:

| Durable sessions | Slots needed | Table raise | Kernel memory |
|---|---|---|---|
| 1 | 23 | none | none |
| 2 | 27 | +4, to 28 | 32 KiB |
| 4 | 35 | +12, to 36 | 96 KiB |
| 8 | 51 | +28, to 52 | 224 KiB |

Each session also takes `DURABLE_BUDGET_PAGES`, 640 pages (64 client, 192 session region, 384
session budget), 2.5 MiB with 4 KiB pages. The alternatives to a global raise are fewer slots per
session or a per-process table size. Neither was examined here, and both change more than a
constant.

The progenitor is untouched by A. Its peak of 23 of 24 is its own table. A adds nothing to it
beyond fork 8's schedule archive, which travels as a mapped blob, and blobs cost the progenitor no
slot (`login`'s build comment, milestone 233 (`login` dies on every boot)).

### A finding: the real boot cannot yet fit one durable budget

The progenitor gives `login` `LOGIN_CONSTRUCTION_PAGES`, 768. When a schedule archive is present,
`_start` splits `own_ut` (128) and `durable_ut` (640) before `channel_ut` (32): 800 pages from 768.
`durable_ut` takes the last page, `channel_ut` fails, and `login` dies at `fail(2)` before serving
anyone. It is latent because the real boot passes no archive today. Whatever answers fork 8 has to
raise the constant to at least 800, plus 128 per concurrent login, first. That is option-neutral:
B's re-deriver would need its own 640 pages per session from the progenitor instead.

### Boot latency

`Durable::restore` waits up to `END_WAIT_SECS` (5 seconds) for the timetable's reply. Under A that
wait sits before `login`'s front door opens, once per re-derived identity. Nobody has measured the usual wait, in the suite or on a board.

### Code

A adds a loop to `login.rs` around calls that exist, and deletes
`components/src/session_reviver.rs` (415 lines), `kernel/src/user/session_reviver_service.rs` and
`system_tests/src/user/session_reviver_tests.rs`. B keeps all three, adds the hand-over on both
sides and moves `open_schedule` into a crate both programs share. That comparison is effort, and
it is not the reason for the recommendation; question 7 is.

## 6. How reversible, and who has already acted on it

A is reversible. Nothing crosses a process boundary that did not before, and every on-disk format
stays as it is: the schedule file of §122 (the on-disk, per-user schedule store), the manifest (§125) and the suspended list. Bringing a
separate re-deriver back later costs B's hand-over then, no more than it costs now.

Who has acted on §123's shape:

- `session_reviver` exists and is tested, spawned only by the kernel harness. No real boot runs it.
- §125 names it as the manifest's reader, and §108 and §222 (who holds a user's schedule) cite it. Under A the
  manifest's writer and reader are the same program, and its format does not change.
- calef ratified the name `session_rederiver` on 2026-09-13; the rename was never performed. A
  retires the program, so that ruling lapses rather than being overturned.
- `packages/init.package.toml` lists it, and `xtask/src/swish_check.rs` mentions it in a comment.

B is the option that is not reversible, because of its hand-over format.

## 7. Would we choose A at equal cost?

Yes. If B cost what A costs, B would still add a format and strand memory, to gain a deletion that
leaves `login` holding the same power. C adds a holder. D and E give up what the milestone is for.
The recommendation is not about effort.

## Recommendation

A. `login` re-derives at start-up, before its receive loop, for the one identity its table can
keep; `session_reviver` is retired, and §123 is amended to say the re-deriver is `login` and why
the deletion property does not apply. Widening beyond one session is separate work, priced above,
and fork 4 closes as moot.

If an architect says no, D stands: `session_reviver` stays a synthetic proof in the kernel harness,
a stored schedule resumes at its user's next login, and milestone 152's boot half stays a
demonstration. If the answer is B, the next step is a proposal for the hand-over format, since that
is a decision two programs agree on.
