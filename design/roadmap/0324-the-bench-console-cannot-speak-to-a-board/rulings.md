# The two rulings and how they were reached, 2026-09-19

An appendix of [milestone 324 (the bench console cannot speak to any of the three
boards)](../0324-the-bench-console-cannot-speak-to-a-board.md), moved out of it on 2026-10-12 (UTC)
under §212 (a prose budget). The block states both rulings and what they built, and is complete
without this file. This file keeps the reasoning behind each ruling, the premises that had decayed
before promotion, and why the title changed.

## Part 4: it writes, under a mode that names its purpose

The invariant in `script/board-console`'s header changes from *"it reads and never writes to the
board"* to it writes only what a named mode sends, and every byte it sends is printed into the log.
That second clause is not decoration. The log is the artifact a bench run is judged from. A byte the
board received that the capture does not show makes the capture a lie about the run.

The reason for *narrow* rather than a general typing mode is an incident rather than a preference.
Milestone 249's lane sent the escape byte by detaching the console, and its block records what that
cost: *"Sending it at the wrong moment also stopped U-Boot's autoboot countdown and cost a power
cycle."* The hazard is not writing; it is an open keyboard beside a countdown a stray byte consumes.
So the shape is 249's own proposal, `--stop` and `--stop-after <n>`, each a command with a purpose.

The reason this block offered for answering "no" had expired, and that is worth recording. Part 4
was filed on the possibility that *"Milestone 218 (every boot of the VisionFive 2 needs a human
typing four commands into U-Boot) may remove the need by fixing autoboot"*. 218 is BUILT and was
confirmed on a real boot on 2026-09-16, with the countdown expiring and nobody typing
(`bench/radon-2026-09-16/tour-083200.log`). It removed the four `StarFive #` commands. It did not
touch 249's escape, which had created a second reason to write thirteen days earlier.

## Part 3: one tool with a board profile, and the profile is the firmware prologue only

A tool each would fork 3,545 lines whose portable majority is already proven shared.
`script/soak-test --arch <a>` runs on all three architectures, and its own header says the judging
is literally `script/board-console`'s code, exit statuses included.

The split the profile needs was already in the source, undocumented as a design. `Stage` in
`crates/serial_console/src/progress.rs` was two things in one enum. `Spl`, `OpenSbi`, `UBoot` and
`Handoff` are radon's firmware chain. `Banner`, `Machine`, `SelfTest`, `Tour` and `Soak` are the
kernel's own ladder. That half's doc comments already recorded it as reachable on all three
architectures since milestone 268. A board profile is a firmware prologue and nothing else; the rest
is shared and always was.

Argon is deferred rather than profiled, and the scope note is the point. It has never booted nife
and sits behind milestone 127 (the seL4 machine), which is NOT-STARTED. A profile written for it
today would be a Jetson boot chain read out of vendor documentation and never watched on a wire.
That is the assertion-shaped-as-measurement failure this tree keeps catching. Its prologue stays
unwritten until a board prints something.

## Why answering part 4 first mattered

Parts 1 and 4 are close enough to collide, and that is an argument for the cluster rather than
against it. Part 1 wants to send one keystroke and part 4 asks whether sending anything is allowed.
Answering 4 first makes 1 either trivial or refused, and nothing in four separate proposals said so.
It was answered first, on 2026-09-19, and it made 1 trivial. That is the cluster earning its keep in
the one way a cluster can.

## Two premises had decayed before promotion

Two of the four parts had decayed by the time they were read, the same as milestone 323's two.
Part 4's reason for a possible "no" was spent by milestone 218 on 2026-09-16. Part 3's premise was
falsified by xenon's own capture on 2026-09-17. Both happened after filing and before promotion on
2026-09-18. See 323's *What the two decisions cost*, which argues that promotion is where a stale
premise is cheapest to catch; four instances across two clusters is the evidence for it.
