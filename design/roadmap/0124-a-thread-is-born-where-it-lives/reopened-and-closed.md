# The reopening and the close, 2026-08-16 and 2026-08-17

An appendix to [milestone 124 (a thread is born where it lives)](../0124-a-thread-is-born-where-it-lives.md).
It holds the two sections the block grew after it was first built: the reopening when the overflow
banner came back, and the close when the fault was found to be a freed stack. The page carries the
outcome.

## Reopened: the guard page is being stepped over again (2026-08-16)

The same `* KERNEL STACK OVERFLOW *` banner, the same one-page guard, the same test on both
architectures:

- aarch64, in a merge-queue run at 01:48Z: slot 87's guard page, `sp` 4096 bytes past the
  bottom of a 16384-byte stack, panicking in `fatal` in `kernel/src/arch/aarch64/exceptions.rs`.
- riscv64, in pull request #213's `cpu matrix` job at 22:29Z: slot 102's guard page, 4088 bytes
  past, on the `rv64` model only, with the other four models green.

Both land in `kernel::user::supervision_tests::a_faulting_child_reports_to_its_supervisor_and_is_
reaped_then_respawned`.

Three things are already known and are what make this worth reopening rather than filing fresh.

It is not any pull request's fault. The aarch64 failure was a merge-queue candidate whose pull
request is 59 lines of markdown, so it reproduces against `main`'s own content. It had been read as
pull request #213's problem for four hours on that evidence, which was wrong.

It is intermittent, which is why the tree stayed green around it: merge-queue runs at 01:52Z and
01:53Z passed on the same base that failed at 01:41Z.

The depth hypothesis is refuted, by measurement, and it was this block's own first guess. The
reopening proposed cumulative depth plus an exception frame arriving at the worst moment. A lane
built the instrument this milestone and `notes/stack-high-water.md` had both named as missing, a
call-graph walker (`script/stack-depth-check`). It measured the deepest chain a thread stack can
reach: 13712 bytes on aarch64 and 13344 on riscv64, against a 16384-byte stack (measured on this
branch, 2026-08-16). Take these from a run, not from here. They moved twice in one afternoon,
13792 to 13760 to 13712 on aarch64, purely from merging other people's work. The gate prints them
and the gate is the authority. It also reports the graph acyclic, and no frame over the guard page
reachable from any thread entry point on either ISA. A
fault at a slot's guard base needs `sp` 20480 bytes in. The deepest watermark ever observed is 10600.

So the banner was never a measurement. `warn_if_guard_page` derived every line from the faulting
*address* and then wrote "so sp went N bytes past it", which is a claim about the stack *pointer*
that nothing in it had read. It now prints `sp` beside the address in the same units and leaves the
comparison to the reader. That one sentence is what sent this reopening, and the four hours before
it, after the wrong thing.

What survives is the evidence the hypothesis was invented to explain. Six recorded guard-page
faults land on exactly two addresses: aarch64 slot 87 (`notes/frames.md`, 2026-08-13, and again
08-16) and riscv64 slot 102 (`sched.rs`'s harness doc ~08-11, `notes/stack.md` 08-14, and again
08-16). Unmoved by #157 and unmoved by this milestone's own rebuild of the spawn path. Depth
wanders; those do not. A fixed address is a stray store rather than a stack running out, and the
two candidate mechanisms are in `notes/stack.md` along with what the next occurrence must print to
settle it. One of them is geometry worth knowing: slot `N`'s guard page begins one byte past slot
`N-1`'s last usable stack byte, so a pointer treating a stack top as inclusive lands in a guard page
with `sp` nowhere near it.

124's own fix comes out stronger than it claimed: no oversized frame is reachable from a thread
stack at all. Its remaining `BUGS` entry, that the riscv64 overflow was not proven fixed, closes as
*the class it addressed is closed, and this is a different bug wearing its clothes*.

This stayed `PARTIAL` until the fault was reproduced on a desk, which happened on 2026-08-17.
The section below is what it was, and it was not a stack overflow of any kind.

## Closed: the stack was freed under its owner (2026-08-17)

A supervised corpse is published as `Dead` while it is still executing on its own kernel stack,
and an out-of-band reaper on another core is allowed to free that stack. `depart()` marks the
thread `Dead`, delivers its death message (which wakes the supervisor), releases `SCHED`, and only
then calls `schedule()`. A supervisor that reaps in that window runs
`reap_supervised` -> `reclaim_region` -> `reap_region_objects`, whose refuse phase asks only about
`state`. `Dead` is not `Ready`/`Running`/`Blocked`, so nothing refuses, the `Thread` is dropped, and
`KernelStack::drop` unmaps six pages with a real `tlbi` under a running core.

The kernel already had the flag that answers this and the reap path never asked it.
`Handshake::on_cpu` means exactly "a core is standing on this thread's stack" and is cleared by
that core's successor in `finish_switch`, whose doc says in so many words that dropping a `Thread`
"must not happen while any core still stands on it". `finish_switch` obeys it; `reap_region_objects`
reasoned from `state`, and "never runs again" is not the same as "off its stack". The fix is one
clause: refuse while `on_cpu`, without arming a kill, because the thread is already dead and the
refusal clears one context switch from now. Both supervision tests now retry their reclaim, which
is the idiom the same file already used two lines later.

Reproduced on the desk, which 45 runs had failed to do, by widening the window in `depart` with
a spin loop: the fault appears on the first run with the delay in and does not appear with the
refusal in. Not committed; the mechanism is recorded instead, and closing the window properly is the
handoff below.

Three pieces of evidence, and the first two were already in the tree. The full account and
arithmetic is notes/stack/kernel-stack-freed-under-its-owner.md (the section left `notes/stack.md`
in the 2026-09-24 split).

- The fixed address is forced, not chosen. aarch64's `SAVE_CONTEXT` walks `sp` down 272 bytes
  per level and stores upward in 16-byte steps. So the terminal store of a cascade lands on the
  guard base *exactly*, every time. riscv64's 8-byte `sd` gives base or base+8, which are precisely
  the two riscv64 addresses ever recorded. notes/stack.md said this on 2026-08-15 and contradicted
  it forty lines later, and the contradiction is what sent this reopening after a stray store that
  does not exist.
- `ELR_EL1` confirms the walk in all three aarch64 dumps: `+0x214`, `+0x228` and `+0x234` are
  the 5th, 10th and 13th `stp` of the same-EL synchronous vector entry, and each one's store offset
  puts `sp` in the `[G-256, G)` window the arithmetic predicts. The paragraph that read `ELR` as a
  refutation compared a CI address against a local build's `exception_vectors`, in a file
  that had already measured the two half a megabyte apart.
- The fourth occurrence names it. CI run 31960738448 was the first firing after this milestone
  added the conservative `.text` scan. The scan took a translation fault on the very first word
  of the slot's stack: the stack is unmapped. Nothing overflows an unmapped stack. That fault
  also destroyed `ELR_EL1`, `FAR_EL1` and the new `sp` line before any of them printed, so the
  instrument ate its own report on its first real firing. `stack.rs` now runs the scan last and asks
  `mmu::is_mapped` before each page, which turns the old fault into the most useful line in the
  report.

Why this test and no other, and why only CI. `a_faulting_child_reports_to_its_supervisor_and_
is_reaped_then_respawned` is the only place in the suite that reaps a corpse the instant it is told
about one, with four assertions between the `ipc_recv` and the `reclaim_region`. That is a race
between a few hundred instructions on each of two cores: one run in six on a loaded 2-core runner,
zero in 45 on an idle laptop. It is also why no fix moved it. #157, this milestone's spawn-path
rebuild and the per-CPU interrupt stack all changed depth, and depth was never the variable.

What this milestone's own work is worth, restated honestly. Nothing here was wasted and nothing
here fixed the banner. `script/stack-depth-check` is the gate that finally *refuted* depth rather
than the one that would have caught this; the frame-shrinking closed a real separate hazard; the
per-CPU interrupt stack is a structural win measured in the interrupt-at-the-worst-instant case.
The banner was a different bug wearing their clothes, for the third time.

The better fix, deliberately not taken here, wants a lane. The window exists because a thread is
published as `Dead` before it is off its stack. The fix is to mark it `Departing` in `depart` and promote it
to `Dead` from `finish_switch`, which already holds `SCHED` and already runs at exactly the instant
the stack is free. That would delete the window instead of refusing inside it, and no caller would
ever see a transient refusal. That is a change to the death protocol and to `RunState`, which lives in
`crates/wake_handshake` where loom searches the transitions, so it is a decision rather than a
hotfix.

The worst `spawn_on` instantiation went from 4592 bytes to 1040, every one of them now clears the
4096-byte guard page on its own merits, and `script/stack-frame-check`'s ratchet is deleted rather
than lowered.

Two predictions in this block were wrong, and the corrections are the useful part. It said the
fix needed `insert_in_place` on `crates/slots` and that reading the Kani harnesses alongside it was
the main cost. Neither was true: the table stores a `TcbPtr`, not a `Thread`, because a TCB lives
on its own page. `crates/slots` was never touched and no harness moved. The copies were all between
`Thread::spawn`'s frame and `ptr.write`, so the fix is `Thread::spawn_into` writing through a pointer
the caller already holds, plus `Threads::insert_in_place` to hand that pointer down.
