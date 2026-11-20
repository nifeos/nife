---
status: BUILT
raised: 2026-08-02
built: 2026-09-19
---
# 64. Enough `std` to run somebody else's crate

Built on 2026-09-19 (PR #984), after five passes. The first was on 2026-08-04 (PR #113),
the second on 2026-08-17, three on 2026-08-18, and the last on 2026-09-19. The last bound file times
and closed the one item the Follow-on still called outstanding ([the last
pass](0064-std-for-real-crates/tcp-listener-and-file-times.md)). Every other row on the ranked list
is closed, declined with a recorded reason, or owned by another milestone or a proposal; the
Follow-on names each home. The measurement's deliverable is the prioritized gap list that milestones
99 and 66 consume, in `notes/crates-io-on-nife.md`: 50 crates.io crates, 43 built, 7 failed. 43 is
against this tree as it ships and 39 is against it without `entropy_backend`, which is the number
this block carried until the third pass.

The block's own sequencing still holds: run the measurement phase first and independently, pick the
probe crates, build them, and let the failures name the work. The `File::open` resolution question is
a design fork inside it, to be raised before code and answered jointly with milestone 47's namespace
half rather than twice.

## The record of each pass

Each pass's working moved on 2026-10-12 (UTC) into three appendices under §212 (a prose budget);
this page is complete without them.

- [ranked-list.md](0064-std-for-real-crates/ranked-list.md): how the ranked list was worked from
  the first pass to the fourth. It covers the corrections to the build split, the calls that abort a
  process, and what each remaining rank waited on.
- [tcp-listener-and-file-times.md](0064-std-for-real-crates/tcp-listener-and-file-times.md): the
  fourth pass's `TcpListener` binding and its gate, and the last pass's file times by path.
- [scope.md](0064-std-for-real-crates/scope.md): the original framing, covering what milestone 27
  (Rust `std` on the native ABI) left, why the work came due, the method, and the tiers shared with
  milestone 47 (navigation and naming).

## BUGS

- "Runs unmodified" is the claim to be careful with. A crate that compiles is not a crate that
  works. A crate that works under one grant may fail under another, because on this system what a
  program can do depends on what it holds. The acceptance evidence has to be a crate doing its job
  with a stated endowment, not a green build.
- The PAL patches std's own source, so every function added here is more surface for
  `toolchain drift` to break against a future nightly. That is a real recurring cost and the reason
  to add only what a probe demands.
- Threads opened a scheduling question this project has now answered: declined, for now.
  `std::thread::spawn` implies a thread the program owns, and the kernel has TCBs and a budget
  model. Which of those a `std` thread is stayed open after the second pass. All four of the crates
  behind rank 3 (`rayon`, `crossbeam-channel`, `tokio`, `ignore`) already compile and link, so
  nothing was blocked on the code and everything was blocked on the answer. The fork is written up
  in full in notes/thread-spawn-fork.md (pull request #394, 2026-08-22). `Tcb::CONFIGURE` consumes
  the `Aspace` capability it binds and the `Thread` struct owns it outright. So no two TCBs can share
  one address space today, which is what a `std` thread actually needs. Two real shapes a fix could
  take were costed alongside declining outright. One is a kernel-level shared VSpace, seL4's own
  answer and the lineage `objtype::ASPACE` already cites. The other is sibling processes kept
  aliased by replicated frame mappings, whose apparent cheapness (no system call touched) does not
  survive contact with what a growing heap needs from an allocator. calef decided 2026-08-22
  (§105): decline. No customer for real shared-memory threads exists yet; `thread::spawn` stays
  `Unsupported` permanently rather than pending, and milestone 149's Rayon-parallel NPB-Rust variants
  stay out of scope until one does. §105 is the record; nothing here forecloses building the
  seL4-shaped option later.
- The shim fills `env`'s `TZ`, `LANG` and
  `TERM` from a granted inert-configuration page (§111 (inert configuration is a validated page);
  see the Follow-on). A program granted no such page still starts empty, and every `env::var` it
  reads is `None`, which is the design.
- A fifth check of the ranked list (2026-08-22) found no genuinely buildable row. After §105
  closed rank 3 and PR #341 closed rank 21, every remaining row fell into one of five groups:
  - already closed;
  - already declined with a recorded reason (ranks 7, 10, 14, 15, 20, 23, 29, 30);
  - a documented contract property rather than a gap (`set_nonblocking`, read/write timeouts and
    `lookup_host` all say so in `sys/net/connection/nife.rs`'s own doc comment);
  - part of the `File::open` fork now held by milestone 154 (a process that holds two directory
    capabilities);
  - a wire-format change wanting a `DECISIONS` section (rank 2, rank 19, and now rank 28,
    `File::set_times`, named alongside rank 19 for the first time here).

  This pass's only change was correcting the two stale mentions of rank 21 as still open and naming
  rank 28. Do not read that as the list being exhausted the way the third pass wrongly said it was.
  A `panic!` or a trap instruction hiding behind a green build would look exactly like this too.
  Only reading the PAL's neighboring functions (the third and fourth passes' method) or building a
  new gate would find one.
- File times are only as true as the server's clock, and it has none. `modified()` reports what
  `GETMTIME` says, and for a file written on this system that is the FS server's per-mount counter.
  So it reads as early 1970, orders wrongly against files the host tool stamped, and restarts each
  boot. And because the engine only moves an mtime forward, a nife write to a host-made file does
  not change its time at all. Proposed as `design/roadmap/0497-a-filesystem-server-that-knows-the-time.md`; the
  details are in notes/std/file-times.md's `BUGS`.
- The open-`File` time forms refuse (`File::metadata().modified()`, `File::set_times`), because
  the verbs take a name. `set_times` truncates to whole seconds and refuses an access time whole.
  None of this is exercised through a grant narrower than the mount root from `std`.
- The sweep is a gate now (`cargo xtask std-aborts`, fourth pass), and it is honest about its
  boundary. It covers `library/std/src/sys/` only, because `sys` is the platform layer. A panic
  above it (`path.rs`, `thread/scoped.rs`) is a caller's bug that behaves identically on Linux; a
  first version that swept all of std found about forty of those and none of the platform kind. The
  cost of that boundary is real: portable std code that is only reachable on a platform this thin
  is invisible to it, and finding those still needs somebody reading. It also proves reachability
  of a *body*, never of a *call*. See notes/std.md, "What still ends a nife process".

Effort: not estimated, deliberately. The measurement is the first deliverable: pick the probes,
build them, and report what breaks.
## Follow-on

- **Refused.** Rank 2, the `std::os::unix` fallthrough, stays absent: a uid and a Unix mtime-set
  would be a fiction laid over a capability refusal, and `notes/crates-io-on-nife.md`'s second row
  already reads declined.
- **Refused.** Rank 3, `thread::spawn`, is declined for want of a customer, written up in
  `design/decisions/0105-thread-spawn-decline-for-now.md` and in `notes/thread-spawn-fork.md`. It
  stays unsupported permanently rather than pending.
- **Done.** Ranks 19 and 28 no longer want a decision: milestone 47's mtime lane landed the get,
  set and set-at verbs in `crates/filesystem_contract`, on
  `design/decisions/0112-touch-mtime-authority.md`, decided 2026-08-23.
- **Done.** Ranks 19 and 28 are bound by path (2026-09-19, [the last pass](0064-std-for-real-crates/tcp-listener-and-file-times.md)):
  `metadata(p).modified()` on `GETMTIME`, `fs::set_times` on `SETMTIME_AT`, and the shim's comments
  that said no verb existed are corrected.
- **Milestone 504.** The open-`File` forms (`File::metadata().modified()`, `File::set_times`) still refuse,
  because the verbs take a name; a handle-taking form is a wire change and is
  milestone 504 (an mtime for an open file), `design/roadmap/0504-an-mtime-for-an-open-file.md`.
- **Milestone 497.** A file written on nife reads as early 1970, because the FS server stamps its own
  per-mount counter rather than a wall-clock second (notes/touch.md's `BUGS`, and notes/std/file-times.md's
  `BUGS` where a std reader meets it). Giving the server the clock page is
  milestone 497 (a filesystem server that knows what time), `design/roadmap/0497-a-filesystem-server-that-knows-the-time.md`.
- **Recorded.** The exit event still carries no exit code, so a supervisor can tell exit from crash
  and cannot tell `exit(0)` from `exit(1)`. The reasoning lives in `notes/std.md`.
- **Done.** `env` is seeded now: `design/decisions/0111-inert-config-is-a-validated-page.md` and
  the `environment_contract` package define a validated read-only page, and the environment shim under
  `patches/std-nife/overlay/std/src/sys/env/nife.rs` fills `TZ`, `LANG` and `TERM` from it before
  `main` runs.
- **Milestone 154.** Tier-two `File::open`, anything that joins or traverses, still waits on a
  second directory capability reaching a real process. 154 has the mechanism host-tested and
  guest-proved, and nothing in the PAL resolves against two roots.
- **Recorded.** A green build is still not evidence, and the acceptance bar stays a crate doing its
  job under a stated endowment. `script/crate-probes` reports the split and cannot report
  behavior.
- **Recorded.** The PAL patches std's own source under `patches/std-nife/overlay/`, so every bound
  function is more surface for toolchain drift, which is the reason to add only what a probe
  demands.
- **Recorded.** The abort scan in `xtask/src/main.rs` covers std's platform modules only and proves
  a body reachable, never a call, so portable std code reachable only on a platform this thin stays
  invisible to it.
- **Recorded.** `script/crate-probes` and the abort scan both ship under provisional names, said so
  where they live, and naming is an architect's.

## Index row

Fifty crates.io crates measured: **43 build, 7 fail** (2026-08-18; 39 without `entropy_backend`),
ranked by demand, which is the list milestones 99 and 66 consume and is `script/crate-probes` rather
than a prose recipe. **A green build is not evidence**. Four std calls aborted a nife process while
compiling perfectly (`env::temp_dir`, `env::split_paths`, `process::id`, and `process::exit`, a trap
instruction that `cargo xtask std-aborts` found). None could appear on a list built from
`Unsupported` counts. Closed: the namespace verbs, `getrandom`, `env`, `File::set_len`, `fs::copy`,
`TcpListener`, the four aborts. Last (2026-09-19) came file times by path, `modified()` on
`GETMTIME` and `fs::set_times` on `SETMTIME_AT`, which had been a binding for three weeks while the PAL said no
verb existed. Declined with reasons: the `std::os::unix` fallthrough and `thread::spawn` (§105).
Proposed: an mtime through an open `File` (a wire change), and an FS server that stamps a real time
rather than a per-mount counter
