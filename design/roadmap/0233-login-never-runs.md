---
status: BUILT
raised: 2026-09-02
built: 2026-09-02
---
# 233. `login` dies on every boot, and the boot says it is ready

Minted the same day by the maintainer, from milestone 230's
(`script/shell-check` is red on `main`, on both architectures, and nothing says so) lane, which found
it the moment that check could see straight. *(Number provisional until the merge queue lands it.)*

It was minted with no gate, on the grounds that the cause was measured and the fix understood, and
that held. What it leaves behind is a check: `script/swish-check` now fails if the kernel reported
killing any user thread during the run, on both architectures. Proven able to fail rather than
assumed, `components/src/least_authority_demo.rs` was temporarily patched to trap on one argument and the gate went red
naming the thread.

In brief. The `login` thread died on every boot, on both architectures, and had been
doing so unnoticed. It does not any more; the rest of this block is written as it was found, because
the finding is the interesting half.

Measured rather than deduced: instrumenting `login::fail` to fault at `0xFA11_0000 + step` gives
`far 0xfa110001`, which is step 1, `nifefs::Fs::parse` refusing the archive. `login`'s `_start` reads
the archive length from `a1`, and `crates/system_initializer` starts it with
`thread_control_block_start(login_tcb, 0, 0, 0)` and endows it with **no mapping of the archive**.

And the boot prints `init: login ready`, with a generated password, because init measured the
identity provisioning rather than login's survival. So the line is true about what it checked and
false about what a reader takes it to mean.

That makes this the sharpest member of a family this tree found four of in one day (milestone 232,
audit every check against two questions). Not a check that failed to run, but a check that passed
while the thing it named was dead.

## Why it is coupled to milestone 231

Handing `login` a mapping of the archive costs init capability slots, at exactly the peak
milestone 230 (shell-check is red on main) measured and sized the table against: 21 of 24. With
three slots of deliberate
headroom that its own block calls a guess standing in for a mechanism.

So the fix and the accounting move together, and milestone 231 (nothing counts how many capability
slots a boot actually uses) is the accounting. Doing this one blind is how the table gets raised a
fourth time, reactively, after another silent failure.

## What it needed, and what each was decided as

`login` stopped needing the archive, which was the open half of this block. It is handed two
blobs instead: `fs_subtree_caretaker`'s ELF bytes and the measurement table, at the two addresses
`login_proto` now names, with their lengths in `x0` and `x1`.

Giving it the archive was the option refused, and the refusal has two reasons rather than one. It
could not be done with what init holds. `supervision_proto::build_child` maps only pages the spawner
has a `PageFrame` capability for, and the archive is reserved RAM the frame allocator does not own
and nothing names. It would have needed a new kernel object naming reserved RAM, which is a
system-call-surface decision and calef's. And it is the wrong shape anyway. This program needs one
program's bytes and a table to check them against, and a service that can read every file in the boot
image to answer a password holds authority it never exercises.

The deeper defect was that the two spawners disagreed and only one was tested. The kernel's own
harness could map the archive because it is the kernel; init never could. So the path the whole
guest suite exercised was the path the real boot never took. Both now lay down the same two blobs,
which is the part of this fix that stops the class rather than the instance.

The boot's report stops overstating. `init: login ready` is now `init: login credentials
provisioned`, and the credential half is unchanged: `identity_provisioner` answering `IDP_RPT_OK`
means a store exists holding that identity and that secret, which is worth saying. That `login` is
alive is a different claim, and init cannot make it without a message and a wait it does not have,
and a boot that blocks on a child's readiness hangs when the child is the broken thing. The claim
moved to the gate rather than being dropped.

**The no-thread-killed assertion is in**, reading the kernel's own fault-report text out of the
transcript (the same `KERNEL_WRITER_ANCHORS` milestone 230 introduced for the interleaving reader, so
the two uses cannot drift apart). It is green on both architectures, which it could not have been
before this milestone.

## What it cost, which is the thing the block said was interesting

Nothing in capability slots. Milestone 231's gauge (nothing counts how many capability slots a
boot actually uses) reports 21 of 24 both before and after, on both architectures.
`supervision_proto`'s `fill_and_map` holds one frame capability at a time and deletes it, so `blobs`
reaches the same transient peak `build_child` already reaches copying `login`'s own segments. So
`CAPABILITY_TABLE_SLOTS` was **not** raised a fourth time, and that is a measurement rather than a
hope, which is exactly what pairing these two milestones was for.

Twenty-five pages of `login`'s address space, out of init's root budget, for the caretaker image and
the table.

## What the trap experiment found, which was not the thing being tested

Proving the new assertion could fail meant making something trap on purpose. `worker` was patched to
fault on one argument and `script/swish-check` run against it. The assertion fired, and so did
something else: **the prompt never came back.** `swish` waits on a job's result endpoint, and a
thread the kernel killed never sends, so a spawned command that faults hangs the shell rather than
returning a status. Recorded in `components/src/swish.rs`'s `BUGS`; an ordinary non-zero exit is fine and
is not this case.

## BUGS

- How long this was true is unknown. Nobody bisected it, and unlike milestone 230's five days
  there is no green-to-red transition to search for, since the check that would have noticed was
  itself not running.
- The measurement check `login` performs is weaker than it was, and its own docs say so. It used
  to read the same physical archive the kernel maps for init, so it was independent of whoever
  spawned it. Both blobs now come from init, which has already run the identical check over them, so
  what remains is a consistency check on the hand-over. Kept because it costs one hash and catches a
  spawner that pairs the wrong two blobs.
- Nothing else that init starts has been checked program by program. The no-thread-killed
  assertion now covers all of them at once, which is stronger than asking the question of each, but
  it only catches a program that *dies*. A service that comes up and answers nothing useful still
  passes every check in the tree, which is milestone 232's (audit every check against two questions)
  territory rather than this one's.
- **A spawned command that traps hangs the prompt**, found by the experiment above and recorded in
  `components/src/swish.rs`'s `BUGS`. Not fixed here: the pieces exist (init supervises every child) and
  what a faulted job should look like at the prompt is a design question `grant_plan::spawnproto`
  has no word for.

## Follow-on

- **Refused.** Handing `login` a mapping of the boot archive, for two reasons rather than one. It
  could not be done with what init holds, since `supervision_proto::build_child` maps only pages
  the spawner has a `PageFrame` capability for and the archive is reserved RAM nothing names. It
  would need a new kernel object naming reserved RAM, and that is a system-call-surface decision. And it
  is the wrong shape anyway: a program that answers a password should not be able to read every
  file in the boot image.
- **Milestone 235.** A spawned command that traps hangs the prompt, found by this milestone's own
  trap experiment. `swish` waits on a job's result rendezvous and a killed thread never sends.
- **Milestone 232.** Nothing else init starts has been checked program by program. The
  no-thread-killed assertion catches a program that dies; a service that comes up and answers
  nothing useful still passes every check in the tree, which is 232's territory.
- **Recorded.** `design/roadmap/0233-login-never-runs.md`'s own `BUGS`: how long `login` had been
  dying is unknown. Nobody bisected it, and there is no green-to-red transition to search for,
  because the check that would have noticed was itself not running.
- **Recorded.** `design/roadmap/0233-login-never-runs.md`'s own `BUGS`: the measurement check
  `login` performs is weak. Both blobs come from init, which has already run the identical check,
  so what remains is a consistency check on the hand-over rather than a read of the physical
  archive the kernel maps for init.

## Index row

it is handed the caretaker image and the measurement table as blobs instead of the archive, for
zero capability slots; the boot line names what init measured, and the gate fails on any killed
thread
