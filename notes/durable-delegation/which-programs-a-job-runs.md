# Fork 8: which programs a scheduled job may run on the real boot

*Milestone 152 (durable delegation), written 2026-09-27 (UTC) to the seven questions of CLAUDE.md's
"A fork reaches an architect with its questions already answered". **Status: DECIDED**, option D,
calef, 2026-09-27 (UTC), on #1377; built the same day (below). The code cited is as of the
commit that added this file.*

## The question

When a timetable fires on the real boot, which program bytes may it load, and who puts a program in
that set?

## What the tree does today

- `login` is handed one *schedule archive*: `user_timetable_keeper`, `timetable`, and a nested `jobs` archive.
  `vouched_schedule` checks every entry against the boot slot's measurement table
  (`vouched_schedule` in `components/src/login.rs`). `user_timetable_keeper` copies `jobs` into itself and again into
  the timetable (`components/src/user_timetable_keeper.rs`, its BUGS section). The timetable resolves every entry's program
  against that archive when a document arrives (`resolve` in `components/src/timetable.rs`) and loads it
  itself at each fire (`fire` in `components/src/timetable.rs`). No other process sees a job's bytes.
- The real boot passes a zero length (`system_initializer::boot`, where it starts `login`), so
  `SCHEDULE` is a plain login there. The kernel harness passes one program, `least_authority_demo`
  (`wired` in `system_tests/src/user/login_tests.rs`).
- The prompt, the analogous case. A bare name resolves through the live activation generation, one
  entry per name, and ignores owner-vouch rows. That is the bare-name ruling, B2, whose decision
  file is on #1374's branch and not yet on `main` (`bare` and `live_generation_listing` in `components/src/swish/capabilities.rs` there). The spawner then
  checks the bytes' digest against the same generation (`system_initializer::vouched`). An
  unvouched image runs only for a holder of the run-unvouched capability (gate D2), which fork 3
  keeps from every job.
- The threadbare-base ruling (B2, on #1421) moves every candidate job program out of the slot. The command-line tools, `timetable`
  and `login` become packages in the default set, and the slot keeps the floor (the progenitor, the
  console, `swish`, storage, `net_stack`).

## Is the premise true? One correction

Fork 3's recommendation says §220 (signed builds)'s drop of a distrusted key "reaches scheduled work at its next
fire, through the progenitor's ordinary activation-set lookup". That is false as built. A job's
bytes are checked against the boot measurement table once, when `login` starts, and the progenitor
never sees a fire. The system-log section on #1423 says the same: "A scheduled job is spawned by its timetable, not the
progenitor". So fork 3's property holds only under an option below that resolves jobs through the
activation set. The note's fork 3 text is corrected in the same commit.

## Options

| option | what a job may run | who decides | cost | reversible | verdict |
|---|---|---|---|---|---|
| A. The slot's whole measured catalog as `jobs` | every program in the boot archive | the image build | 8.8 MB (`target/initial-ramdisk-riscv.img`, 2026-09-21), copied twice per session, against a 640-page (2.5 MiB) durable budget (`DURABLE_BUDGET_PAGES` in `components/src/login.rs`) | yes | Refused: does not fit, and after the threadbare base the slot holds only the floor |
| B. A named list, in the image | e.g. the 12 command-line tools | the image build | 926 KB (#1421's table), larger alone than the session's region (192 pages, `DURABLE_UT_PAGES` in `components/src/login.rs`) or the timetable's (224 pages, `TIMETABLE_REGION_PAGES` in `components/src/user_timetable_keeper.rs`) | yes | Refused: does not fit, and names bytes the threadbare base moves out of the slot |
| C. Owner-vouched programs only | `vouch ./x` rows | the owner, per digest | a schedule line would have to name a digest or path, since the bare-name ruling says a vouch claims no name | yes | Refused: excludes every installed package, which after the threadbare base is the whole userland |
| D. The live activation generation, resolved at registration and re-checked at each fire | what a bare word runs at the prompt, minus D2 | the owner by installing (§208 (installing a package is granting it)), the user by naming it | not built. The timetable gains read access to `activation/` and `packages/`, and the `jobs` archive and its two copies go. Each fire reads one image into its 48-page instance (`INSTANCE_PAGES` in `components/src/timetable.rs`), a limit every option already has | yes: `login_contract::user_timetable_keeper` is provisional and only this lane speaks it | **Recommended** |
| E. D's set, snapshotted by `login` at `SCHEDULE` and at boot | the programs the user's document names, as installed then | as D | copies stay, and a `REPLACE` naming a new program fails until the session is rebuilt, since the archive is fixed at spawn. A revoked key reaches a job at the next boot, not the next fire | yes | Refused, the runner-up: it keeps the timetable directory-free and pays for it with fork 3's property |
| F. Status quo | nothing | nobody | zero | yes | What a no leaves |

## Prior art, read 2026-09-27

- cron, `crontab(5)` (man7.org): "The entire command portion of the line ... will be executed by
  /bin/sh", as "the user who owns that particular crontab". The job runs what that user could run,
  looked up when it fires. From memory, not re-read: cron's default `PATH` is `/usr/bin:/bin`, and
  `cron.allow`/`cron.deny` decide who may use cron, not which programs.
- systemd, `systemd.service(5)` (`man/systemd.service.xml` on `main`): a command is "an absolute
  path to an executable or a simple file name without any slashes", resolved "using a fixed search
  path determined at compilation time". A timer only activates such a unit.
- launchd, `launchd.plist(5)`: `Program` is "the absolute path to the executable"; without it, the
  first `ProgramArguments` element may be "a relative path which is resolved using _PATH_STDPATH".

None of the three keeps a second list of schedulable programs. Each resolves, at the fire, in the
system's standard set. D is that shape with the standard set as the activation generation and no
search order, which is the bare-name ruling's shape at the prompt.

## Recommendation: D

One rule for what may run, at the prompt and on a schedule: installed and vouched, never D2.
Installing is granting (§208), so the owner already decides the set, and a user who wants a job
installs or vouches it first, as fork 3 said. It is the only option under which §220's revocation
reaches a job at its next fire, which is what fork 3 was ruled for. It removes the `jobs` archive
and its two copies instead of sizing them, and it still works when the threadbare base empties the slot.

**Would we choose D if every option cost the same?** Yes. B is the least work and E is close to
it, so D is not chosen for effort. The price is real: the timetable holds a read-only view of two
store directories, which its module doc lists as a thing it does not have today. A job gains
nothing from it, since the timetable endows a job only with what its entry grants.

## If calef says no, and what is blocking

- **If no:** F stands. `SCHEDULE` on the real boot stays a plain login and 152's durable half
  runs only in the kernel harness. If calef prefers E, the next step is the snapshot's build in
  `login`, with fork 3's text saying revocation waits for a boot.
- **Blocking now, whatever the answer:** `LOGIN_CONSTRUCTION_PAGES` (768, in
  `crates/system_initializer`) must rise past the 800 pages `login` splits once it has any
  schedule archive (`DURABLE_UT_PAGES` in `components/src/login.rs`).
- **Eventually calef's, not blocking:** the D build's contract change for the timetable (a grant in
  place of `a1`'s archive), and the names that come with it.
- **Found on the way:** the three decision files #1361 minted (the line editor, the set of names,
  bare names) are not on `main`. That pull request shows as merged, but a force-push dropped them.
  The restore is commit `5f4bca6db` on #1374's branch and lands with it. So the rulings of #1374,
  #1421 and #1423 are cited here by pull request, and gain section numbers when those land.

## What building D found (2026-09-27)

- The timetable needs a file-service channel of its own. A job fires while its user may be
  using the store at the prompt, and two clients staging bytes in one page overwrite each other.
  So the timetable reads through the file service's last window, which the progenitor and the kernel
  harness reserve for `login` (milestone 599 (a frame per filesystem client channel)).
  `login` badges the endpoint with the window's number and builds both caretakers on it.
- That costs `login` a slot at rest, which leaves a 24-slot table exactly full at a login's
  peak with one durable session. The open stays inside it because `login` now builds the two store
  caretakers before the rest of the session, and the client's own caretaker after it. Counted from
  the code, not measured, in `login_contract::durable`.
- It first bound each entry to the digest registered, so an upgrade stopped the job until its
  document was re-sent. calef called that a foot gun and amended D the same day (#1377). Every
  fire now resolves the name and plans the line again against the current manifest, as a bare
  word does. A version asking for more than the line grants does not fire. Nothing can tell the
  owner yet; `components/src/timetable.rs`'s BUGS names the system log (#1423) and notices (#1424).
- The harness fixture first installed the demo under the image's own name,
  `least_authority_demo`. That ran into §229 (how a bare name at the prompt reaches an installed
  program), which rules that a package cannot take an image program's name (calef, 2026-09-27; it
  landed on `main` while this branch sat). `with_entry` answered `ImageName` and the expect behind
  it panicked the guest suite at its first login test. The inbound check went down with it: 0 of 4,
  no listener ever ran. Found 2026-09-30 (UTC) reproducing the rebase regression. The fixture now
  installs the same bytes as `scheduled_demo` (provisional), a
  name no boot image packs; the image's own name stays legal where the archive path genuinely hands
  out image programs (`timetable_tests`' narrowed archive, the shipped `timetable.conf`).
