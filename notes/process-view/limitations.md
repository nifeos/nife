# The process view: the BUGS list in full

*Name: provisional.*

An appendix to [notes/process-view.md](../process-view.md), whose BUGS section lists each entry in
one line.

- Userspace cannot learn the machine's online cpu set, so a placement census cannot show an idle
  core. `record::PLACEMENT` reports which cpu a thread was placed on, and a tally of those ids is
  a correct census of *where the threads are*. It cannot report a core that is online and carrying
  nothing, because that core appears in no thread's record, and it cannot tell that case apart from
  a core that is parked. Nothing in this tree gives userspace `smp::online_harts_mask`, and this
  record deliberately does not smuggle it out: the mask is a fact about the machine and a survey
  answers questions about a domain.

  Do not work around it with a range. `for cpu in 0..count` is the VisionFive 2 bug
  (`crates/cpu_set`), and it is wrong on a real board in both directions at once. A supervisor that
  needs the full set needs a machine-description path that does not exist yet; a proposed milestone
  for one is in this lane's report.

- A placement can be stale, and the record says so where a reader meets it. It is where the
  thread was *put*, not where it is. A thread stolen by an idle core, or woken onto its waker's core
  (DECISIONS §28.2), goes on reporting its original placement. Keeping the live answer means a store
  in `schedule()`'s switch, which measured 5.7% of `ipc_fastpath`'s footprint on aarch64 and is why
  `Thread::last_cpu` is behind a soak-build feature. The honest reading of this record on a busy
  machine is "the arrangement the placement lottery produced", which is exactly what milestone 240
  built it to report, and not "the current layout".

- Two mechanisms for two questions, and the tree owes a reader the pointer. calef's 2026-09-21 ruling (the
  decisions section provisionally numbered 204, not yet on `main`, so named rather than cited) said
  more. A thread observing itself gets a per-thread page on `rseq`'s shape rather than a selector,
  because the consumer there is an allocator reading its own cpu on every allocation. That page is
  not built. Until it is, a thread that wants its own cpu has no cheap path, and nothing in this
  note is it.

- Holding a domain with `READ` was more authority than looking needs. Fixed 2026-08-17, and the
  entry is kept because the shape recurs and the fix is small enough to reuse.

  The finding: `READ` on a supervision endpoint is also what `RECEIVE` and `endpoint::REAP` take, so a
  viewer endowed a view could take a death message out from under the real supervisor
  (`job_undertaker`, at the interactive boot) or collect a corpse. `ps` did neither, and its source
  was the whole argument that it did not, which is exactly the kind of argument this system exists
  to replace with a mechanism.

  The lane that found it deliberately left it, on the reasoning that splitting view from control
  changes the rights model and is the *same* decision the signaling stratum needs. That
  reasoning turned out to be wrong in a way worth recording. The signaling stratum mostly evaporated
  when calef ruled that a domain names its members and does not act on them. There was no second
  decision to wait for, and the deferral was buying nothing.

  The fix is `capability::Rights::ENUMERATE`, the kernel-level twin of `filesystem_contract`'s directory
  `ENUMERATE`, and it is the same argument one layer down. `SURVEY` takes it; `RECEIVE` and `REAP`
  still take `READ`; `system_initializer` grants a viewer `ENUMERATE` alone. So a `ps` does not
  get refused a reap, it cannot name one, which is the ladder's top rung in place of an argument
  about a program's source.

  The tell that one bit was doing three jobs is worth carrying off. `READ` on an endpoint
  unlocked receive, reap and survey, and no grant could express any one of them. When a right
  unlocks operations that differ in kind rather than in degree, it is not a right, it is a
  category.

- The cursor and the tid are machine-wide slot indices, so a viewer can *count* threads outside
  its domain even though it can never name one. Found by the 2026-08-17 security audit
  (design/audit-reports/), recorded-accepted, and the fix proposed as a milestone in that report.

  The mechanism, in two lines of kernel. `sched::survey_supervised` returns `slot as u64 + 1` as
  the `next_cursor`, where `slot` is the index into `Scheduler::threads`, which is the whole
  machine's thread table. And a tid is a `generational_table` generational name, `(generation << 32) | slot`,
  so the low half of every tid a survey reports *is* that same index and the high half is the
  number of times that slot has been recycled since boot, machine-wide.

  What a viewer holding `ENUMERATE` alone can therefore work out about the rest of the system:

  - that other threads exist, from a single member, because its member's slot index is a lower
    bound on how many slots were occupied when that member was created;
  - how many threads were created between two of its own members, by subtracting their two
    cursors. That is the `c2 - c1 >= 2` assertion in
    `kernel/src/user/survey_tests::the_survey_cursor_counts_threads_the_viewer_cannot_name`, which
    builds a stranger between two members and measures the gap;
  - machine-wide churn in a slot, from the generation half, which counts other domains' thread
    lifetimes in that slot and only ever increases.

  Two domains that can each spawn can turn this into a covert channel without sharing any
  capability: one modulates global slot allocation by spawning and exiting, the other polls its own
  members' cursors. The bandwidth is low and nobody has measured it.

  Why accepted rather than fixed here. The honest fix is a per-domain cursor and a domain-local
  thread name, and both change what a tid *is*: `endpoint::REAP` takes a tid, `abi::fault`'s death
  message carries one, and `ps` prints one. So it is a change to something two programs agree on,
  which is the category that cannot be un-shipped by reverting a commit. And it reaches the syscall
  surface: the `REAP` of §16 (object revocation) and the death message of §26 (the fault endpoint). That makes it a
  milestone rather than an audit lane's patch. A cheaper partial exists and is worth weighing against it: return an opaque
  cursor (the slot index XOR a per-endpoint value) and leave tids alone, which closes the
  subtraction channel and leaves the generation half open.

  What is not affected, and it is the part that matters most. A viewer still cannot *name* a
  thread outside its domain, cannot learn its tid or its state, and cannot reap it. The
  confinement claim in `a_domain_is_exactly_the_children_of_the_endpoint_that_was_granted` is
  intact; this is a counting channel beside it, not a hole in it. The `caps ps` line and this
  note's example were corrected in the same lane, because "not learn that a process outside this
  domain exists" was a stronger sentence than the mechanism delivers.

- A process has no name, so there is no `CMD` column. This system has `arg0` in `Spawn` and no
  display name at all. Where a name would live is DECISIONS §164 (whether the kernel resolves a tid
  it already sent), still proposed. Who may read one is answered: a viewer holding `ENUMERATE`
  already learns a member's tid and state, and a name is more about a member already named.

- A survey is a sequence of snapshots. See "why one entry per call" in
  [survey-design.md](survey-design.md). A member born into an
  already-passed slot is missed until the next survey, and a row read early may be stale by the time
  the table prints. Fuchsia handles a process dying mid-enumeration and this does not; their answer
  is worth reading when somebody needs one.

- A child that is built but not yet started is not in its domain. Supervision is recorded at
  `START`, so an embryo has no endpoint to match. That is invisible at the prompt (the progenitor starts a job
  in the same breath as building it) and would matter to a builder watching its own construction.

- The survey cursor leaks the thread table's density, and a proposed milestone covers it.
  `survey_supervised` returns a machine-wide thread-table slot index plus one, and a tid is
  `(generation << 32) | slot` over the same table, so a viewer can subtract two members' cursors and
  count threads it cannot name. A test asserts the gap exists rather than pretending it does not.
  Nothing in `ps` or `pgrep` exposes a cursor to a person: `ps::Row` carries a tid and a state, and
  `crates/pgrep` reads the finished `Survey`, which never held one. Do not widen that.

- The comparison against Linux is not apples to apples, and a write-up must say so. Ours lists a
  domain; theirs lists a machine. That is the entire point, and a table putting the two side by side
  without stating it would be dishonest in the way §14's map "tie" caveat exists to prevent.

- `pgrep`'s selector cannot be typed at the prompt, so the shipped program always names every
  member. The reason is a property of the process boundary rather than of the program, and it is
  written up in [pgrep.md](pgrep.md). The filter's own tests hand it real selectors.

- `pgrep` matches glob, not upstream's extended regular expression, and it matches a run state
  rather than a process name, because there is no process name. `pgrep 'r.*'` therefore does not mean
  what a `procps` user expects and `pgrep 'r*'` does. Recorded where a reader meets the feature
  (`crates/pgrep`'s `BUGS`) as well as here.

- `pgrep` has no exit status to report with, so "nothing matched" is a sentence on the second
  stream rather than upstream's exit 1. A caller wanting to branch on the answer has to count lines.
  `user_mode_runtime::exit` takes no code, and giving it one is a syscall-surface change.

- `ps` lists itself, and in a pipeline it may or may not list its own reader. It is a member of
  the domain it was spawned into, which is truthful and is what Unix's `ps` does too. The pipeline
  half is sharper. Both stages of `ps | wc` go into the same domain, so whether `ps` walks before
  or after `wc` exists is a race. The boot gate saw three lines on one run and two on the next.
  That is a snapshot behaving like a snapshot rather than a bug. It is why the boot gate asserts
  the header and the scope, while the confinement claim is asserted in the kernel test, which builds
  the domain it measures instead of inheriting one.

- A doomed thread does not say so. DECISIONS §16's `killed` flag marks a thread whose region
  owner has torn it down and which has not yet reached a preemption. It surveys as `RUNNING` or
  `READY`. Adding a bit to the state word is additive and cheap; it was left out to keep the first
  method minimal, and the moment something wants to watch a `^C` land is the moment to add it.

- `pmap` cannot be reached from the interactive prompt against any real process. See "What
  building it found" in [address-space-enumeration.md](address-space-enumeration.md). Every `Object::AddressSpace` capability
  is minted and consumed within its own builder's thread, `ThreadControlBlock::CONFIGURE` removes the space from the registry
  the instant it binds to a thread, and nothing shipped here delegates one to a second program.
  `ps`'s `Manifest::domain` has no analogue because there is nothing alive to wire it to. Fixing
  this is a spawn-protocol question, not a `pmap` change, and it is not decided here.

- `pmap` shows one row per mapped page, with no VA-range coalescing and no size column.
  `abi::aspace::LIST` reads the space's revocation log, which records one entry per page and
  nothing about adjacency between them; upstream `pmap` coalesces contiguous same-permission pages
  into ranges, which would need an ordering guarantee this log does not make.

- `pmap` cannot tell a device mapping from ordinary read/write memory. `kind` is derived from
  `paging::Flags`, which carries no bit for "this is device memory" as far as the syscall handler
  can see, so a `DeviceFrame` mapping (always read/write, never executable) reads as `rw-`,
  indistinguishable from a heap page.

- A resumed `LIST` cursor can land on a slot the space's own log recycled for an unrelated
  mapping, if a tombstoned entry was reused by `record_mapping` between two calls. Unlike a
  survey's slot table, a log entry carries no generation to detect this; see
  `kernel::revoke::list_mapping`'s doc for the mechanism and why it is accepted rather than fixed.
