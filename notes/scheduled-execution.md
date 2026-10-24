# Scheduled execution: a cron whose every entry is a grant

Milestone 129. What `cron` becomes on a system with no ambient authority, why the interesting half
is what a schedule is *refused*, and the one kernel primitive whose absence shapes the whole
program.

The pieces: `crates/timetable` (the decision, host-tested and Kani-reached),
`components/src/timetable.rs` (the budget, the counter, the loader), `components/timetable.conf` (the
document), `system_tests/src/user/timetable_tests.rs` (both ISAs). Every name in that list is
provisional; the block for milestone 129 (scheduled execution) declines to propose one and AGENTS.md says the eventual one
is an architect's.

## What cron actually is, and what is wrong with it here

A crontab line is a command run as a user. That is the whole model, and it means the authority
behind a scheduled job is a property of *who owns the crontab* rather than of anything the line
says. Two consequences follow, and both are structural rather than accidents of implementation:

- Nothing can be printed. Ask "what will this entry be able to do?" and Unix has no answer
  narrower than "whatever that account can do". There is no tool that could print it, because there
  is no object that holds it.
- The crontab is the attack surface. Write access to one file is write access to everything the
  account reaches, at a time of your choosing, forever.

Neither is a criticism of cron, which is doing exactly what a Unix scheduler can do. It is the
observation that a capability system can do something else.

## The inversion

An entry is a schedule plus a grant expression, and the grant expression is the same text a
person would type at the prompt:

```text
every 150ms  least_authority_demo 7
at-boot      least_authority_demo 3
every 1s     date
```

At registration each command goes through `grant_plan::parse_run` and `grant_plan::plan`, which is
the same function `swish` checks a prompt line with. That is one call rather than a second
implementation, so "a scheduled entry is checked exactly as a typed one" is true in the code instead
of being a convention two places are trusted to keep. What comes out is a `grant_plan::Endowment`:
the complete authority the scheduled child will hold, computable before anything fires and therefore
printable.

So `timetable` prints its whole plan and *then* arms:

```text
timetable: the plan, before anything fires
  every 150ms   least_authority_demo 7
    grants least_authority_demo exactly:
      cap 0  endpoint  report its answer to this timetable
      arg      7
      and nothing else: no clock, no disk, no console, no network
  every 1s      date
    will not fire: this timetable holds no clock, so it cannot grant one
timetable: armed
```

The line `caps least_authority_demo 7` prints at the prompt and the line this prints are the same decision, made
by the same code, one of them for a job that has not been scheduled yet.

## The four answers, and why three of them are the point

Milestone 126's `ps` made the case that the negative controls are the demonstration: three distinct
outcomes where `/proc` has one. Registration here has four where a crontab has one.

| answer | what it means | fixed by |
|---|---|---|
| `Admission::Fires` | planned, backed, armed | nothing to fix |
| `Admission::Refused` | the program's own manifest refuses the line, exactly as at the prompt | editing the entry |
| `Admission::Unbacked` | the line is legal and *this scheduler* holds nothing to back it | granting the scheduler more |
| `timetable::Error` | the document does not parse, with the line number | editing the document |

The `Refused`/`Unbacked` split is the one worth keeping. Collapsing them would tell a person to
edit a line that has nothing wrong with it. `memory_grant_depleter` with no `--mem` is wrong wherever it is typed;
`date` is a perfectly good line that this particular scheduler cannot back, and the fix is a decision
somebody makes on purpose at the spawn site.

### The entry Unix cannot refuse

`every 1s date` is the example the shipped document carries deliberately. In a crontab it runs,
every time, because reading the clock on Unix is ambient. Here the wall clock is a capability
(DECISIONS §43, notes/clock.md): a read-only mapping of the clock page, endowed by whoever is doing
the spawning. `timetable` was granted none. So the entry is refused in writing, at registration,
before the first tick, rather than firing once a second and printing that it does not know what
time it is.

`every 1s ps` is the same shape one milestone later: a process view is a supervision endpoint with
`ENUMERATE` (milestone 126, notes/process-view.md), the timetable holds none to hand over, and a
scheduled `ps` is refused rather than shown an empty list.

Those two entries are the milestone in miniature. They are lines whose danger a Unix scheduler has no
vocabulary to discuss.

## `Held`: what the scheduler holds is a parameter, not an era

`timetable::Held` is `grant_plan::Holdings` one level out, and it exists for exactly the reason that
one does. "This scheduler holds nothing to back that" has to be a statement about a particular
process's capability table. The same document is four refusals in a scheduler granted nothing but a budget, and
four running jobs in one granted a clock, a directory and a terminal. Neither the document nor the
program manifests can tell those two apart.

Every field on the shipped scheduler's `Held` but `mem_pages` (four, for the `--mem 4` line) is
`false`, and that is checkable against the
slot list in `components/src/timetable.rs`'s header:

- slot 0: its output endpoint (WRITE)
- slot 1: an untyped budget (WRITE)
- slot 2: the child report endpoint (WRITE|GRANT)
- slot 3: the supervision endpoint (READ|GRANT)

No clock, no directory, no console, no network, no device. Widening it is an edit to `Held` and a
visible change in the printed plan, which is the property worth having: a scheduler gets wider
because somebody decided it should, not because an entry talked it into it.

## The archive it holds is narrowed to the plan

The endowment above is what a scheduled *child* holds. What the scheduler holds is a different
question, and until 2026-08-18 the answer had one embarrassing entry in it: the whole initrd,
mapped read-only, which is every program in the tree.

That is wider than anything the document can ask for, and the plan is what makes the gap
measurable. `Registry::programs` is the complete set of programs a document will ever start, as a
bitmask, computable at registration and therefore before the spawn site has to decide what to hand
over. So the spawn site builds an archive of exactly that set with `nifefs::write_image`, and the
timetable is handed that instead of the initrd.

The plan cannot be computed in the kernel. Holding a printable plan costs a kilobyte per entry,
so registering a document needs a stack frame far wider than the kernel's 4096-byte guard page
allows. The spawn site therefore carries the program set as a written list, `PLANNED_PROGRAMS`,
and a host test, `the_archive_a_timetable_holds_is_measured_against_what_it_will_build`, holds it
equal to the plan. That is rung two of the ladder, forced rather than chosen.

A process cannot narrow its own endowment, so `timetable` measures it instead: `timetable::Audit`
prints whether the archive it holds carries exactly the programs its plan names, and the
cross-ISA test asserts that it does. The frame sizes, the guard-page measurement and the audit's
two sentences are in [archive-narrowing.md](scheduled-execution/archive-narrowing.md).

It does not narrow per *entry*, and that was refused on 2026-09-26: an image is code, not
authority, so reaching the union of the plan's programs gives a compromise nothing it lacked. See
[one-image-per-entry.md](scheduled-execution/one-image-per-entry.md).

## Registration is the security boundary

Milestone 129's block puts it in one sentence: *whoever can register an entry can make its grants
periodic, nothing more.* Concretely, compromising `timetable` yields the union of what it holds,
which is a memory budget and two endpoints into its own children. It cannot read the clock, cannot
touch a disk, cannot open a socket, and cannot give any of those to a child, because there is no
ambient authority anywhere for a child to fall back on.

Who may register is answered by where the document lives, and for the first deliverable that is
`include_str!`. The document is compiled into the binary, exactly as the multicast DNS responder's
configuration was until that program was retired on 2026-09-15. The recorded reason is the same:
reading a file needs a file capability wired through the spawn (see notes/mdns.md and milestone
131). So today the authority to register is
the authority to rebuild the image, which is the strongest possible answer and also the least useful
one. A runtime registration protocol is a real decision with a real fork in it (the boot endowment?
the shell? a per-registrar endpoint whose entries can only be as wide as the registrar?) and the
honest thing was to ship the document and leave the fork visible rather than settle it by accident. calef ruled it as §222 (who holds a user's schedule): see [registration.md](scheduled-execution/registration.md).

## The arithmetic, and the decision inside it

`timetable::next_after(prev, period, now)` answers the next fire, and it has three properties:

- strictly after `now`, so a polling loop cannot fire one occurrence twice;
- congruent to `prev` modulo `period`, so an entry that ran late comes back onto its original
  beat instead of inheriting every delay it ever suffered;
- it skips rather than catching up. A scheduler away for an hour fires a 10ms entry once, not
  360,000 times.

The third is a decision, and Vixie cron makes the same one. Catching up turns a stall into a
stampede: a housekeeping job that runs two hundred times back to back on a machine that was already
struggling is how a slow morning becomes an outage. The cost is real and stated rather than smoothed
over: occurrences are genuinely lost, and work that must not be skipped wants a durable queue rather
than a scheduler.

Five Kani harnesses hold those properties (`crates/timetable/src/proofs.rs`), and the reason it is
Kani rather than more host tests is that every wrong answer here is quiet. An off-by-one at a
period boundary fires twice in one polling pass and nothing complains. A lost phase drifts a
schedule a few nanoseconds an hour and surfaces as a beat nobody can explain a month later. A
catch-up implementation satisfies every property except the one that bounds it. None of those is reachable by
sampling, because the interesting inputs are the ones nobody thinks to type.

Before editing `next_after`, read
[next-after-proofs.md](scheduled-execution/next-after-proofs.md). It records why the function snaps
`now` back onto the beat rather than counting periods (the snapping form proves in about a second,
the counting form did not finish in ten minutes). It also records the one gap: phase preservation
is host-tested up to `u64::MAX / 4` rather than machine-checked, because a second 64-bit modulo is
where CBMC stops finishing.

## The primitive that was missing, and what it still costs

When this was written (2026-08-18 UTC) there was no timed wait anywhere in this kernel. No sleep, no
timeout, no deadline: the syscall surface was `EXIT`, `YIELD`, `INVOKE` and `CAP_DELETE`, and a
process had exactly one blocking wait point. So a program whose entire purpose is to act at a time
could only yield and re-read the counter, and a running timetable costs a core's worth of yields.

Corrected 2026-10-11 (UTC): the primitive exists now. Milestone 106 (a wait that ends on either the
interrupt or the deadline) built a timer object on 2026-09-26, `objtype::TIMER` with `ARM` and
`CANCEL`, which signals a notification at a deadline ([timer.md](timer.md)). Its own block lists
this note among the places it made false. `timetable` has not adopted it: `components/src/timetable.rs`
still yields between fires, so the cost above is still paid. Adopting it is milestone 674 (the last
yield spinners sleep on a timer), not started.

That was the fork of milestone 51 (wall-clock time) and the gate of milestone 106, and this program
was its fifth consumer. The
block counts four (`net_stack`'s retransmit window, `thread::sleep`, `RECEIVE`'s no-timeout limitation,
the shell's `^C` poll); this is the first whose whole reason for existing is a deadline.

The shape of the fix is already in the code. `Registry::next_deadline` computes exactly the instant
a timed wait would block until. It is computed today even though nothing uses it, so that adopting
the timer changes the loop by one line rather than restructuring it.

The second thing the missing primitive costs is lazy reaping, and it is worth naming because it
looks like a design choice and is not. Reaping a scheduled child means blocking on the supervision
endpoint; blocking means not watching the clock; one wait point means you cannot do both. So
`timetable` reaps only when the budget cannot back another instance, and the failure counts it
reports lag reality until then. A wait that returns on either a message or a deadline fixes this too,
and it is the same adoption.

## What is built, and on what

`system_tests/src/user/timetable_tests.rs`, one module for both ISAs (nothing in it is
architecture-specific, so the parity gate is met by literally the same test running twice). It spawns
the real program on the real `components/timetable.conf`, reads the plan it prints, then watches what
fires:

- the plan names what an admitted `least_authority_demo` and an admitted `memory_grant_depleter --mem 4` will each hold, and
  says "and nothing else";
- `date` and `ps` are refused for want of a clock and a process view, in the plan, before anything
  runs;
- `memory_grant_depleter` with no `--mem` and `wc` carry the prompt's own refusal sentences, unchanged, which
  is the check being the same check;
- the admitted entries fire, under supervision, and their answers arrive on the endpoint the plan
  said they would hold, `memory_grant_depleter`'s included: its grant is nested inside its own instance's region
  (below) and reclaimed before the loop fires anything else;
- and the summary accounts for every child: `4 fires, 4 clean exits, 0 faults`, which is the reap
  working. A scheduler that leaked a region per fire would print the same fire count and then run out
  of budget instead of finishing.

Nothing in that test asserts on time. The fires are counted, never timed, and no wall clock is
compared to anything. A loaded host makes the test slower and cannot make it red
(notes/load-sensitive-assertions.md). Milestone 62 (tests that assert on time) spent a week putting that property back into
this tree, and this is not the lane to take it out again.

## A backable `--mem` grant, and why it runs alone

Built 2026-08-22. `Held::mem_pages` was zero on the shipped scheduler, so an entry naming a memory
grant was `Unbacked::Memory` even though the process held a budget; `timetable::SHIPPED_HELD.mem_pages`
is now 4 and `timetable.conf`'s `at-boot memory_grant_depleter --mem 4` is planned, backed, and fires.

Milestone 129's block said to split the grant out of the *instance's own region*, "so that a
single `MemoryRegion::DESTROY` still reclaims both and a restart loop is not a leak". The kernel
refuses precisely that: a region with a live child cannot be destroyed, so the grant has to be
destroyed first, by its own separate capability.

The nesting is still the right shape, because it is the only thing in this system that pairs a
death with a grant. `components/src/timetable.rs` makes that pairing structural rather than
guessed. `fire_with_grant` and `collect_grant` are called back to back, with everything already
outstanding drained first, so the very next death on the supervision endpoint cannot be anyone
else's.

That is the decision the block left open, made: at most one `--mem` instance may be outstanding
at a time. The cost lands on every *other* entry. One due while a grant-bearing instance is dying
runs late rather than on schedule, and is never dropped. The shipped document's `--mem` entry fires
at boot, before any interval is due, so the cross-ISA test does not exercise that cost. The proof
that pins the refusal, why a supervisor cannot tell whose death it is, and the cost in full are in
[mem-entries.md](scheduled-execution/mem-entries.md).

## BUGS

- A `--mem` entry blocks everything else in the document while it runs. See "A backable `--mem`
  grant, and why it runs alone" above: the exclusivity that makes the pairing sound also means the
  scheduler is fully unresponsive to its clock for as long as one grant-bearing instance takes to
  die. Fine for a document with one such entry that fires at boot; a document leaning on `--mem`
  entries competing with fast intervals would feel it.

- A running scheduler persists nothing. Milestone 152 (durable delegation)'s client writes the
  store, and `login` restores it at login and at start-up.

- The document is compiled in, not read from disk, which is also what decides who may register
  (see above), unless the timetable is spawned with a registration page, which §222 built.

- Calendar lines are UTC only, and resolve to the minute. G5 (ruled 2026-09-26) has no zone clause,
  since `crates/calendar` refuses tzdata; see [calendar-grammar-g5.md](scheduled-execution/calendar-grammar-g5.md).

- No shipped entry designates a file or a directory, so `Unbacked::File` and `Unbacked::Directory`
  are reached by host tests only. Until 2026-09-26 `register` could not reach them at all: `admit`
  planned against the scheduler's own `dir`, so a designation came back refused. Mutants from milestone
  326 (turn a mutation score upward) found it; the planner is now lent a directory and `unbacked` alone decides.

## See also

- [program-manifest.md](program-manifest.md) and [grant-expression.md](grant-expression.md): what
  the check at registration actually is, one and two levels down.
- [supervision.md](supervision.md): why a scheduled child that dies becomes a message, and what
  `Rendezvous::REAP` needs.
- [clock.md](clock.md): why a scheduled `date` is refused, and why reading a *duration* needs no
  capability while reading the *time* does.
- [process-view.md](process-view.md): why a scheduled `ps` is refused.
- [load-sensitive-assertions.md](load-sensitive-assertions.md): why the test counts fires instead of
  timing them.
- [mdns.md](mdns.md): the configuration-document shape this copied, including its compiled-in
  limitation. The program it described was retired on 2026-09-15; the shape is still the lesson.
