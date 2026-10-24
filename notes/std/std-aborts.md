# What still ends a nife process

*An appendix to [`notes/std.md`](../std.md), which is the page to read. This file holds the five
calls that compiled and then killed the process, and the `cargo xtask std-aborts` check with its
EXAMPLES and BUGS. The directory `notes/std/` and this file's stem are provisional names. Naming is
calef's.*

The records this file cites by number:

- milestone 64 (enough `std` to run somebody else's crate)
- §42 (a filesystem declares what it offers and must be truthful)
- milestone 168 (a multi-tasking workload benchmark)


## What still ends a nife process

*(Milestone 64, fourth pass, 2026-08-18.)*

The dangerous std call is not the one that returns `Unsupported`. It is the one that compiles and
then kills you. Five have been found so far, each by a different accident:

| call | what it was | found by |
|---|---|---|
| `std::env::vars()` | `panic!("not supported on this platform")` | working the ranked gap list and noticing a *neighbor* |
| `std::env::temp_dir()` | `panic!("no filesystem on this platform")` | reading every module the PAL falls through |
| `std::env::split_paths()` | `panic!("unsupported")` | the same reading |
| `std::process::id()` | `panic!("no pids on this platform")` | the same reading |
| `std::process::exit()` | `crate::intrinsics::abort()` | `cargo xtask std-aborts` |

None of them could appear on a gap list, because notes/crates-io-on-nife.md's list is built from
PAL functions that answer `Unsupported`, and a function that ends the process never answers. And the
fifth could not be found by the method that found the middle three either: `sys/exit.rs` is not a
`sys/<module>/mod.rs` backend, it is one file with a `cfg_select!` inside a function, so "read every
module the PAL falls through" walks straight past it.

So the reading became a check.

```sh
cargo xtask std-aborts       # on its own, against whatever the farm last built
script/test                  # runs it too: `std-exerciser` ends with it
```

### What it does

It asks the compiler which `library/std/src/sys/` sources it actually compiled for the nife targets,
by unioning every `library/std/src/sys/` path out of cargo's own dep-info under
`std_exerciser/target/`. It greps exactly those for bodies that end a process: `panic!`,
`unimplemented!`, `todo!`, `rtabort!`, `intrinsics::abort()`, `panic_nounwind`. Comment lines are
skipped, because this tree's PAL files discuss the panics they replaced at length.

What it finds is compared against `ABORTS_ACCEPTED` in `xtask/src/farm.rs`, which carries the
reason for every entry. A new one fails the build with the file, the line, and the two things it
can be. That makes the reading rung two of AGENTS.md's ladder, a check, where it had been rung
four, a person reading. Today there are 26 across 79 compiled sources, in three groups:

- unreachable on nife (nine): a body behind a `cfg` this target does not satisfy. `cfg_select!`
  keeps every arm's source in the file, so they are read but not compiled into anything reachable.
- no answer exists (nine): single-threaded, so the call can only deadlock or end, and upstream
  chose to end. `Condvar::wait` and `Once::wait` are the two that matter, and they stay open until
  milestone 64's `thread::spawn` fork is decided rather than being fixable by a PAL arm.
- ours, and deliberate (three files' worth): the clock and entropy refusals, where ending the
  process is the honest report because the call has no error channel and inventing a value would be
  the lie §42 forbids.

### EXAMPLES

Adding a `panic!` to a compiled fallback and running the check:

```
$ cargo xtask std-aborts
std-aborts: a std source compiled for nife ends the process somewhere new:
  sys/net/hostname/unsupported.rs:7: pub fn invented() -> ! { panic!("no hostname on this platform") }
```

and a clean tree:

```
$ cargo xtask std-aborts
std-aborts: 26 process-ending bodies across 79 compiled std sources, all accounted for
```

What the check holds `exit` to, read off the binary rather than off the source, which is the form
of evidence this milestone is short of:

```
$ llvm-objdump -d --demangle std_exerciser/target/aarch64-unknown-nife/release/std_exerciser
000000000040c290 <std::process::exit>:
  40c290: stp  x30, x19, [sp, #-0x10]!
  40c294: mov  w19, w0
  40c298: bl   0x40eaf0 <std::rt::cleanup>      ; flush stdout, announce end of stream
  40c29c: sxtw x0, w19                          ; the exit code
  40c2a0: mov  x8, xzr                          ; SYS_EXIT
  40c2a4: svc  #0

000000000040c2b0 <std::process::abort>:
  40c2b0: brk  #0                               ; still a fault, which is what abort MEANS
```

`exit` flushes and calls `SYS_EXIT`; only `abort` is the `brk`. The bug this check exists for is
`exit` compiling to that same `brk`, under the name of the call that is supposed to be the clean
one.

### BUGS

- It covers `sys/` and nothing else, on purpose, and that is a real gap. `sys` *is* std's
  platform layer: a panic under it says "this platform has nothing to offer", while a panic in
  `path.rs` or `thread/scoped.rs` says "you called this wrong" and says it identically on Linux. A
  sweep of all of std finds about forty of the second kind and none of the first, which is a list
  nobody keeps reading. The cost of the narrowing is that portable std code
  which is only reachable on a platform this thin is invisible here (a `LazyLock` poisoned by an
  earlier panic, say), and finding those still needs somebody reading.
- An accepted entry matches a substring of a line, not a line number. That is what stops a
  nightly's blank line from rewriting the list, and it means one entry can bless two sites when the
  same text appears twice in a file. `sys/exit.rs`'s `crate::intrinsics::abort()` is exactly that
  case: the UEFI arm's last resort and the `_ =>` arm nife used to take are the same string, so
  after this milestone one accepted entry covers a line nobody reaches and a line nobody takes.
  Three entries are deliberately blanket, matching bare `panic!(` in `sys/random/nife.rs`,
  `sys/time/nife.rs` and `sys/pal/nife/clockproto.rs`. Those are the PAL's own files, where every
  panic is one this project wrote on purpose and a new one arrives through review rather than
  through a nightly. A blanket entry over a file we do not own would be the wrong trade.
- It proves reachability of a *body*, never of a *call*. A body compiled into the reachable set
  might still be dead. The check deliberately does not try to decide that, because deciding it is
  reading the call sites, which is the work it exists to prompt rather than to replace.
- It needs a build. The dep-info only exists after `cargo xtask std-exerciser`, which is why the
  check runs at the end of that step rather than in `script/lint`. Run against a stale farm it
  reports the stale farm, honestly and uselessly.
- It never checks that the paths it scans are under `farm_dir()`, and a contaminated build is
  therefore reported as a source defect with file and line numbers. Found 2026-08-18 by milestone
  117's fifth stranger, on its first `script/test` from a fresh clone. `nife-dev` is an account-wide
  `rustup` link, so a clone whose farm has not been built yet compiles `std` out of whichever
  worktree built the farm last. The `-Zbuild-std` dep-info under `std_exerciser/target/` caches
  those absolute paths, and cargo then considers the unit fresh. So re-running reproduces the same
  failure in about thirty seconds and looks like a stable defect rather than a stale one. What the
  stranger saw was two files, two line numbers and two suggested fixes, all of them naming source
  inside another checkout on the machine. It wrote in its journal that both suggested fixes would
  have committed a false statement to `ABORTS_ACCEPTED`, and the only reason it did not was that it
  went looking for why the path was foreign. The recovery is `rm -rf std_exerciser/target`, which
  nothing in the tree says, and the assertion that would have made the message true is one
  comparison against `farm_dir()`. The bullet above says a stale farm is reported "honestly and
  uselessly"; run 5 is the case where it is reported dishonestly, because the paths belong to a farm
  this checkout never built.
- The same stale cache has a third face, and it never reaches the foreign-path check. Found
  2026-09-19 by milestone 168's lane, twice in a row on one worktree. `script/test` failed at
  `std-exerciser: building std_exerciser for aarch64-unknown-nife failed`, with ten errors inside
  the rustup toolchain's own, unpatched std (`none of the predicates in this cfg_select
  evaluated to true` in `sys/alloc/mod.rs`, `sys/io/error/mod.rs`, `sys/thread_local`), while
  `nife-dev` pointed correctly at this worktree's farm and `rustc --print sysroot` answered the
  farm. `rm -rf std_exerciser/target` and a rebuild compiled std from the farm and passed at once.
  So a build under `std_exerciser/target` can pin the plain nightly's `library/` as well as another
  worktree's, and because the build fails before any dep-info is written, `std-aborts`' foreign
  check never runs and nothing prints the recovery. The cause of the pinning was not diagnosed.
  Recovery is the same line: `rm -rf std_exerciser/target`.
- `std-aborts` is a provisional name (milestone 64, 2026-08-18). Names are calef's; this one is
  not ratified.
