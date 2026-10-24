# How long to run it, and why nobody can tell you

*Name: provisional.*

An appendix to [notes/soak.md](../soak.md), which keeps the conclusion. This file holds the survey
of 2026-09-03 behind it: seL4, stress-ng, the Linux Test Project, JEDEC, and the literature on
randomized and stress testing.

## The admission was correct

This note and milestone 225 (run the soak on radon, argon and xenon) both say no duration is
prescribed, because nobody knows what would be persuasive. Only the boards are where its answer means
anything. The admission is correct, and it is the field's condition rather than this project's. Nothing found prescribes a duration for a concurrency
soak. The one place a duration *is* derived derives it from a thermal model that has nothing to do
with interleavings.

Everything below was fetched and read on 2026-09-03. Where a thing was not found, it is written as
not found rather than as absent.

## seL4 runs nothing sustained, and its multicore tests are measured in milliseconds

This is the kernel this project measures itself against. So it is the first place to look, and the
most surprising answer.

`seL4/sel4test`'s test directory (`apps/sel4test-tests/src/tests`, read at `master`) contains no
stress, soak, load or endurance file. Its multicore coverage is `multicore.c`, and every test in it
has the same shape. Start a helper, `sel4test_sleep(env, 10 * NS_IN_MS)`, and check a counter moved
or did not. Ten milliseconds is the whole observation window. The property under test is functional
(a suspended thread stops, a resumed one runs, an affinity change takes effect) rather than
statistical.

`seL4/ci-actions` is the repository holding seL4's GitHub Actions (directory listing read at
`master`). It has 40-odd actions, and none of them is a soak or a stress run. The hardware ones are
`sel4test-hw`, `sel4test-hw-run` and `sel4test-hw-matrix`, which run the terminating suite above on
real boards, and `sel4bench-hw`, which is a benchmark. `sel4test-hw`'s own `action.yml` describes
itself as *"Runs sel4test builds for all hardware test platforms."*

On why there is not more, here are the project's own words. Gerwin Klein wrote on the seL4 Discourse
thread *Testing infrastructure* (2021-02-09) that the CI is *"pull request checks (style, compile,
licenses, etc)"* and *"continuous integration test (either on the master branch of a specific repo,
or, more commonly on repo collections/manifests)"*. And, plainly: "hardware tests are harder,
proposals welcome".

The reading to take from this is not that seL4 is careless. It is that a project with a functional
correctness proof does not buy much from a soak, because the thing a soak samples is the thing the
proof already covers. nife has 145 Kani harnesses and no refinement proof, so the trade is not the
same one. Copying seL4's answer here would be copying a conclusion without its premise.

## stress-ng picks a round number and says so

`stress-ng(1)` is the closest thing Linux userland has to a standard soak tool. Its `-t, --timeout T`
option reads, verbatim (Debian testing manual page, fetched 2026-09-03):

> run each stress test for at least T seconds. One can also specify the units of time in seconds,
> minutes, hours, days or years with the suffix s, m, h, d or y. [...] A 0 timeout will run stress-ng
> forever with no timeout. The default timeout is 24 hours.

Twenty-four hours, with no stated reason. The manual page has one account of what the tool is for:
*"stress-ng was originally intended to make a machine work hard and trip hardware issues such as
thermal overruns as well as operating system bugs that only occur when a system is being thrashed
hard"*. Its one strongly worded caveat is about throughput rather than duration: *"it has never been
intended to be used as a precise benchmark test suite, so do NOT use it in this manner."* Nothing in
it says how long is long enough, or what a clean run licenses.

## The Linux Test Project prescribes nothing either

LTP's documentation (`setup_tests`, read 2026-09-03) treats runtime as a resource to be capped, not
a target to be reached. Tests that run for more than a second or two must declare a `runtime` and
check actively how much is left. `LTP_RUNTIME_MUL` and `-I` scale it. The knobs are all for making
runs shorter. No recommended soak length was found.

## Hardware is the exception, and its number is derived

Semiconductor qualification is the one practice found where "run it for N hours" is a real
requirement rather than a habit. It is worth reading closely, because the derivation is the part
that does not transfer.

JEDEC Standard No. 47G, *Stress-Test-Driven Qualification of Integrated Circuits* (fetched
2026-09-03), Table 1, requires High Temperature Operating Life at Tj at or above 125 C, with Vcc at or
above Vccmax. It wants 3 lots of 77 units, "1000 hrs / 0 Fail". That is a hard number with a hard
accept criterion. And note 5.5(a) says where it comes from:

> with apparent activation energy of 0.7 eV, 125 °C stress temperature and 55 °C use temperature, the
> acceleration factor (Arrhenius equation) is 78.6. This means 1000h stress duration is equivalent to
> 9 years of use.

The same note is careful that the number is not self-justifying: *"The duration listed here is
generally acceptable to qualify for the given Application Level. However, it does not necessarily
imply the demonstration of the lifetime requirement for a particular use condition."*

So the hardware world has what the software world does not: a model that converts stress hours into
a claim about the field. Arrhenius does that for a wearout mechanism at a raised temperature. There
is no analogous model that converts soak hours into interleavings explored, and the reason is the
next section.

Part of what a board soak tests genuinely is the board, and this row of the table is the one that
applies to that half. radon under sustained load is closer to an operating-life sample than to a
concurrency test. It is also the half nife is least equipped to judge, having one unit per
architecture where JEDEC wants 231.

## The academic angle exists, and its finding is that clock time is the wrong axis

There is a literature here, and it is not neutral about stress testing.

Burckhardt, Kothari, Musuvathi and Nagarakatte wrote *A Randomized Scheduler with Probabilistic
Guarantees of Finding Bugs*, ASPLOS 2010 (the PCT paper, PDF fetched 2026-09-03). It opens by
describing exactly the practice this milestone is about:

> Popular testing methods involve various forms of stress testing where the program is run for days or
> even weeks under heavy loads with the hope of hitting buggy schedules. This is a slow and expensive
> process. Moreover, any bugs found are hard to reproduce and debug.

Two of its results bear directly on choosing a duration.

The state space is not the thing to cover, and bug depth is. The paper defines *"the depth of a
concurrency bug as the minimum number of scheduling constraints that are sufficient to find it"*. It
proves that a run of a program with n threads and k steps finds a bug of depth d with probability at
least `1/(n k^(d-1))`. It observes that a naive bound over schedules is useless (*"This program, to
the first-order of approximation, has n^k possible thread schedules"*). It rests on the claim that
real bugs are shallow: *"Concurrency bugs typically involve unexpected interactions among few
instructions executed by a small number of threads."* Their examples put ordering errors at depth 1,
and atomicity violations and lock-cycle deadlocks at depth 2.

That claim is independently measured. Lu, Park, Seo and Zhou wrote *Learning from Mistakes: A
Comprehensive Study on Real World Concurrency Bug Characteristics*, ASPLOS 2008 (PDF fetched
2026-09-03). They examined 105 real concurrency bugs in MySQL, Apache, Mozilla and OpenOffice. Their
finding 3 is "Almost all (96%) of the examined concurrency bugs are guaranteed to manifest if certain
partial order between 2 threads is enforced". Their finding 8 is "Almost all (92%) of the examined
concurrency bugs are guaranteed to manifest if certain partial order among no more than 4 memory
accesses is enforced." Their own caveat is attached and should be carried: the findings *"are
associated with the four examined applications and the programming languages these applications
use"*.

And stress-test coverage saturates, measurably. This is the single most useful thing found, because
it is a measurement of the exact question this note asks. PCT section 5.3.3 instrumented a work
stealing queue with twenty events and 168 possible event pairs, and compared coverage against run
count:

> We restrict the horizontal axis to the 8192 runs as stress did not explore any new event pair beyond
> those already explored in the new runs after that and PCT eventually explored all the event pairs.
> [...] Fig. 11 shows that stress does not cover more than 20% of the event pairs, few of which result
> in a bug. Thus, stress's inability/ineffectiveness to detect the bug is highly correlated with the
> event pairs not covered.

Their stress infrastructure was not a strawman by their account. It inserted *"random sleeps, thread
suspensions, and thread priority changes"*, a superset of what this soak's jitter does. It still
stopped finding anything new, and then ran forever without improving.

That is this note's own line, measured by somebody else. *A soak that repeats one interleaving for
eight hours has explored one interleaving* was written here as an intuition. PCT put a number on the
shape of it: coverage climbs, flattens, and the flat part is free.

## So the field has a habit, not a standard, and here is what to do instead

Stated plainly, because it is a real finding and dressing it up would be worse than useless: 24
hours, 48 hours and overnight are round numbers. The one prescribed duration found anywhere (1000
hours) is prescribed for a thermal wearout model. It states its own derivation, and warns that the
number does not by itself demonstrate the requirement. For concurrency, nothing found in tooling
documentation, in seL4's practice, or in the literature converts clock time into a claim.

The alternative is to reason from this workload's own counters. That is available here, and is not
available to most people asking this question. `script/soak-test` already prints, every five
seconds, `rounds`, `rate`, `wakes`, `wakerate`, `crossings`, `remote`, `steals` and `deferred`.
Those support three questions, none of which is "how many hours":

1. What is the run buying per hour, in the units that matter? Not round trips, which saturate the
   machine by construction, but `crossings`. The cross-core wake path is what the tick route
   exercises. (The defect once recorded there was retracted; see `multicore-defect-curve.md`, row
   D6.) And the crossing rate is one to two orders of magnitude below the round-trip rate. A radon
   run's crossing rate is the honest denominator. At the QEMU aarch64 figures (about 3,779 crossings
   in 25 seconds on the better of two runs), an hour is a few hundred thousand crossings. A second
   hour is another few hundred thousand of the same kind. Decide the duration against a target
   crossing count, arrived at deliberately, and then say what it was.
2. Is the run still producing new behavior, or is it flat? This is PCT's saturation question, and
   this tree cannot currently answer it, because nothing here counts distinct behavior. `remote`,
   `steals` and `deferred` are the closest available, and they are volumes rather than varieties.
   This is the gap worth closing before the duration argument is worth having. It is a milestone
   rather than a note: something like a coarse histogram over the placement decisions, so a beat can
   be compared with the beat before it and a flat run can be recognized as flat.
3. Would the time be better spent on more starts than on longer running? The crossing count varies
   by more than a factor of two between identical runs. The parent's BUGS records it, and it is
   evidence that the initial conditions matter more than the tail. Under PCT's model, independent
   runs multiply the probability of finding a shallow bug, and a single long run does not. Ten
   one-hour boots are ten samples of the boot-time placement lottery, and one ten-hour boot is one.
   Nothing here proves that trade for this workload. It is the question a duration decision should
   be made against rather than around.

`script/interleaving-check` is the complementary instrument, and this section sharpens why. Loom
over the extracted protocols searches the state space directly, which is the thing a soak samples
badly and saturates at. The two are not competitors, and the soak is not the weaker one. The soak is
the only one that runs on silicon at all, which is where risk 5 says the defects are.

What none of this decides is the number, deliberately. It says the number is an architect's, and
gives them the axis to pick it on: a crossing target on real silicon, chosen and written down, rather
than an hour count inherited from a tool's default.
