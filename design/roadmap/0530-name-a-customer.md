---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: name-a-customer
milestone_dependencies: none
decision_dependencies: 157
machine_requirements: none
specific_machine: none
needs_person: no
---
# 530. Name a customer, or admit the ranking function has nothing to rank

*(Number provisional until the merge queue lands it.)* Promoted from the proposal `name-a-customer`, filed 2026-09-21, on calef's instruction of 2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own, unedited except for this paragraph: the argument is its author's and promotion is not the moment to improve it. Raised by the maintainer, from calef's question about what nife
would have to be to change anyone else's behavior, and from the fact that every honest answer began
with "a customer".

Nobody but calef can name one, and no lane can substitute for it.

## The hole this names

`AGENTS.md`'s first principle ranks work by the shortest path to a system a customer runs, and then
says plainly that the path is vacant. It has been since 2026-08-30, when the family's backups
went to borg over SSH on cordoba and the premise of milestone 55 (Time Machine over SMB3) went with it. The file is clear that
this is the principle working rather than failing: a customer with a real deadline went elsewhere,
early, which is exactly what the principle exists to surface.

**But a ranking function with nothing to rank stops ranking.** The tie now breaks toward
`design/fatal-risks/README.md`, which the file itself calls a stand-in rather than a replacement. Ten weeks
on, the visible effect is real: this project is very good at the work that can be justified without a
user, and the roadmap has grown to 515 milestones.

## What the first customer has to be, from what the last one taught

The recorded lesson is precise and it is about size, not enthusiasm: *"A first customer should be
something nife can plausibly be adequate at within a milestone or two."* The family backup server
was among the largest things a home system can be asked to be: somebody else's filesystem, a
network protocol, crash consistency, and the only copy of irreplaceable data.

So a candidate is worth naming only if:

- It is adequate-able within a milestone or two, on evidence rather than optimism.
- Somebody notices when it breaks, which is the only test that cannot be gamed.
- Failure is survivable. Not the only copy of anything, not anyone's livelihood.
- It exercises something the demonstrator claims. A workload that avoids isolation, drivers and
  real hardware proves nothing about this thesis.

## The candidates already in the tree

Named so the decision starts from what exists rather than from a blank page. None is recommended
here; the ordering is the maintainer's reading of adequacy distance, closest first.

- A machine that runs one thing on real silicon. xenon boots nife from firmware today; radon
  boots and has been a bench target for weeks. A single long-running service on a board that nobody
  needs back is the smallest honest customer this project could have.
- The package manager's own path. DECISIONS §157 (a trivial install is a web page, a USB drive
  and packages) defines an install story, and §203 (capacity is rented rather than bought) has just
  made rented machines part of how this project works. A nife machine that serves this project's own
  builds is a customer whose complaints arrive as our own broken builds.
- A measurement appliance. The benchmarking work already runs real workloads on three
  architectures; a box whose job is to produce numbers on a schedule is a workload nife is close to
  adequate at, and its output is evidence the demonstrator needs anyway.
- Something outside this project entirely, which is the only candidate that tests principle 1's
  distinction between "the architect runs it" and "a customer runs it". `AGENTS.md` records the
  precondition calef set for that: a package manager and a trivial install must exist before a
  second customer can be accepted at all.

## calef's answer, 2026-09-21: the path stays vacant, and it is blocked rather than empty

**No customer is named, and that is a decision rather than a deferral.** The reason is a precondition
calef set himself and which this proposal had listed as one candidate's caveat rather than as the
governing fact: **no third party may be accepted until a package manager and a trivial install
exist.** Every external customer is therefore blocked on milestone 198 (the package manager) and
DECISIONS §157 (a trivial install is a web page, a USB drive and packages).

**So the ranking function is not idling through neglect.** It is waiting on a gate calef already
closed, and writing that down converts a gap into a gate. A reader who finds the customer path empty
now finds the reason beside it. And `design/fatal-risks/README.md` doing the ranking meanwhile is the
designed behavior rather than a drift.

The candidate that came closest was refused for the reason that matters. A measurement appliance
scores well on every stated criterion: adequate within a milestone or two, failure survivable,
somebody notices when the numbers stop, and it exercises isolation, drivers and real hardware rather
than avoiding them. It fails the distinction principle 1 exists to draw. calef's own correction
is that he is the first customer and not the audience, which separates *the architect runs it* from
*a customer runs it*. An appliance serving this project is the architect again in a different hat,
and naming it would let the ranking resume without resolving what made it vacant.

What this makes of the package manager. If no customer can be named until it exists, then the
work that unblocks the customer path is the customer path, and milestone 198 inherits the
ranking function's top slot without being a customer itself. That is the strongest available reading
of principle 1 while the path is blocked, and it is what the roadmap should act on.

## What would reopen this

- Milestone 198 and §157 landing, at which point a third party becomes acceptable and this
  proposal's criteria apply again to whoever is named.
- A workload arriving that meets the criteria without a package manager, which is possible and
  should not be ruled out by this decision: the criteria are the test, and the precondition binds
  only third parties.

## Index row

`AGENTS.md`'s first principle ranks work by the shortest path to a system a customer runs, and then
says plainly that the path is vacant.
