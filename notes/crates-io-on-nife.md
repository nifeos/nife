# Somebody else's crate: what crates.io does against the nife `std`

*(Milestone 64, measurement phase, 2026-08-04. Milestone 27 (rust) built a `std` platform layer on the
native ABI (notes/std.md) and claimed it widened real workloads to "most of crates.io that stays off
fs and threads". This note is the attempt to find out whether the qualifier is doing as much work as
it sounds like. Fifty crates were taken off crates.io and built against the patched `std`; this is
what happened and why.)*

The short answer: the qualifier was pessimistic, and the table was measuring the wrong thing.
`std::fs` answering `Unsupported` for 32 of 54 functions is not what stops crates from building.
39 of 50 probes built with no change at all, including `regex`, `serde_json`, `tokio`'s
current-thread runtime, `rayon`, `clap`, `chrono` and `walkdir`. Of the 11 that failed, eight
failed on one crate, and it is not part of `std`.

> The split is 43/7 (re-derived 2026-08-18, milestone 64's third pass, by `script/crate-probes`).
> Bare `script/crate-probes` gives 43 built and 7 failed; `--no-backend`, without
> `entropy_backend`, gives 39/11. A crate in two failure classes counts once. How each number was
> derived is in [crates-io-on-nife/probe-results.md](crates-io-on-nife/probe-results.md).

The measurement, and the reasoning behind every pass over it, is in
[crates-io-on-nife/](crates-io-on-nife/README.md).

The long answer has a sting in it, and it is the block's own BUGS entry made concrete: a crate
that compiles is not a crate that works. `tempfile` builds and links, and every one of its
operations returns "operation not supported on this platform" at runtime, because it has an explicit
fallback arm for platforms it does not know. Nothing in a green build says so. See
[What compiles and still does nothing](#what-compiles-and-still-does-nothing).

## How to reproduce this

```sh
script/crate-probes                  # all fifty: the table and the split
script/crate-probes regex tokio      # two of them
script/crate-probes --no-backend     # the same fifty without entropy_backend
script/crate-probes --keep           # leave the generated probe crates and their build logs
```

PROVISIONAL NAME (milestone 64 (std), 2026-08-18): names
are an architect's and this one is not ratified.

It needs the network, so it is not a CI gate and `script/test` does not run it. It builds for aarch64 only, deliberately:
the PAL speaks the capability ABI rather than an ISA, so a second target would re-measure the same
thing at twice the cost, and the parity that matters is `std_exerciser`'s, which `script/test` runs
on both.

A failing probe is rebuilt for the host, and reports `BODY` rather than `FAIL` if the host build
fails too. That is the harness saying "my own call site is wrong, ignore this row", which is the
check fifty hand-written `main`s could not give anyone.

The recipe the harness automates (one throwaway `[[bin]]` crate per probe, whose `main` must call
the crate so the linker is asked to link it) is in
[crates-io-on-nife/probe-results.md](crates-io-on-nife/probe-results.md).
## What happened, by crate

Fifty crates, resolved versions as of 2026-08-04, built for `aarch64-unknown-nife`. 39 built with no
change, from the pure-computation floor (`itertools`, `memchr`) through `regex`, `serde_json`,
`tokio`'s `rt`, `rayon`, `clap` and `gix-hash`. Twelve were rebuilt as executables that call the
crate for real, and all twelve linked.

The 11 failures fall into four classes, and the class matters more than the crate:

- Class A, `getrandom` has no `nife` backend (8 of 11), closed 2026-08-17. `entropy_backend`
  supplies `getrandom`'s documented custom hook: depend on it, set the `getrandom_backend="custom"`
  cfg in the workspace's `.cargo/config.toml`, and write `use entropy_backend as _;` in the binary.
- Class B, the crate falls through to `unix` (3 of 11): `filetime` and `gix-sec` end their platform
  ladder in "else it is unix". Whether a crate compiles here depends on whether its author wrote a
  fallback, not on how hard the crate is.
- Class C, a C library or C sources (3 of 11): `ring`, `zip` via `zstd-sys`, and `diesel` with
  `sqlite`. These are jobs for the C seam (notes/c-seam.md).
- Class D, a socket and fd model that does not exist (1 of 11): `rocket`, via `socket2` and `mio`.

The full table, the per-class errors and the `getrandom` decision are in
[crates-io-on-nife/probe-results.md](crates-io-on-nife/probe-results.md).
## What compiles and still does nothing

`tempfile` 3.27.0 selects its platform like this:

```rust
#[cfg_attr(any(unix, target_os = "redox", target_os = "wasi"), path = "unix.rs")]
#[cfg_attr(windows, path = "windows.rs")]
#[cfg_attr(not(any(unix, target_os = "redox", target_os = "wasi", windows)), path = "other.rs")]
mod platform;
```

`other.rs` is six functions, and every one of them is:

```rust
Err(io::Error::new(io::ErrorKind::Other, "operation not supported on this platform"))
```

So `tempfile` builds, links, and returns an error from `NamedTempFile::new()`. That matters far
beyond `tempfile`, because gitoxide's atomicity story runs through it: `gix-lock`'s commit calls
`self.inner.persist(&resource_path)`, which is `gix-tempfile`, which is `tempfile::persist`, which
on nife is `not_supported()`. A `gix` that built cleanly would fail to write a single ref.

> That paragraph was wrong about which failure came first, and the correction is worse than the
> claim (2026-08-18, milestone 64's third pass). Until that day `NamedTempFile::new()` did not
> return an error: it aborted the process. `tempfile::env::temp_dir()` delegates straight to
> `std::env::temp_dir`, nife had no `sys/paths` backend, and the shared fallback's `temp_dir()` is
> `panic!("no filesystem on this platform")`. The program died there, before `other.rs` was ever
> reached. Reading a crate's fallback arm and stopping is how that was missed: the analysis was
> right about `tempfile` and never asked what `std` did on the way in.
>
> It is fixed as of that date (`sys/paths/nife.rs`), and fixing it changed nothing about the
> conclusion above, which is the point worth keeping. `temp_dir` now answers, `other.rs` now gets
> reached, and `other.rs` still refuses everything. The abort became the error this section
> always described.

The lesson for milestone 99 and 66 is a sequencing one. Do not read a passing build as a working
crate, and do not order the work by what fails to compile: `tempfile` never appears on a build
failure list and is on the critical path for git.

## The prioritized gap list

This is the deliverable milestones 99 and 66 consume. It is not the order the milestone 27 table
suggests, because that table counts functions and this counts demand.

The method: for each probe, take `cargo tree -e normal --target aarch64-unknown-nife.json` (normal
edges only, so `cc`, `autocfg`, `vcpkg` and every proc-macro crate are excluded, since those run on
the host and can call anything they like), then grep every package's `src/` for call sites of the
std APIs the PAL refuses. The "probes" column is how many of the 50 dependency closures contain at
least one call site.


| rank | gap | PAL today | probes | packages | note |
|---|---|---|---|---|---|
| 1 | `getrandom` backend | CLOSED 2026-08-17 | 8 failed outright | `rand`, `uuid`, `ring`, all of `gix` | not a std gap; the `entropy_backend` workspace crate (named by calef, 2026-08-18). This cell said `patches/getrandom-nife`, which has never existed under that name |
| 2 | `std::os::unix` fallthrough | absent, declined | 21 | 34 | mostly benign (behind `cfg(unix)`); fatal in `filetime`, `gix-sec`. See below |
| 3 | `thread::spawn` | `Unsupported`, decided | 20 | 33 | `rayon`, `crossbeam`, `tokio`, `diesel`. The fork was resolved by DECISIONS §105 (declined) (2026-08-22): for want of a customer, not for want of a mechanism. See below, and notes/thread-spawn-fork.md |
| 4 | `env::var` | CLOSED 2026-08-17 | 16 | 24 | `chrono` (TZ), `clap`, `gix`, `figment`; `vars()` used to panic |
| 5 | `fs::create_dir(_all)` | CLOSED, milestone 64 | 11 | 17 | bound on `MKDIR`. `create_dir_all` also needed rank 19a |
| 6 | `available_parallelism` | `Ok(1)` | 11 | 7 | answers honestly |
| 7 | `fs::read_link` | `Unsupported` | 10 | 10 | no symlinks in the contract |
| 8 | `File::set_len` | CLOSED 2026-08-17 | 10 | 9 | `TRUNCATE` existed all along; only the size word was missing |
| 9 | `fs::symlink_metadata` | bound | 9 | 13 | the row was stale: std routes it to `lstat`, which this PAL binds |
| 10 | `std::os::fd` | absent | 9 | 12 | `mio`, `memmap2`, `is-terminal` |
| 11 | `fs::read_dir` | CLOSED, milestone 64 | 8 | 13 | bound on `OPENDIR` and `READDIR` |
| 12 | `fs::remove_file` | CLOSED, milestone 64 | 7 | 12 | bound on `UNLINK` |
| 13 | `fs::remove_dir(_all)` | CLOSED, milestone 64, with `remove_dir_all` at milestone 122 (directory) | 7 | 8 | bound on `RMDIR`; `remove_dir_all` needed no verb once nested paths resolved |
| 14 | `fs::hard_link` | `Unsupported` | 7 | 4 | no verb |
| 15 | `Permissions` | `readonly` is `false` | 7 | 8 | authority is a capability, not a mode bit |
| 16 | `env::temp_dir` | CLOSED 2026-08-18 | 11 | 9 | it was a `panic!`, not a refusal; re-counted from 7 |
| 16a | `env::split_paths` | CLOSED 2026-08-18 | 8 | 4 | also a `panic!`; no row existed, and no namespace in it |
| 16b | `process::id` | CLOSED 2026-08-18 | 5 | 5 | also a `panic!`; `gix-tempfile`'s fork check |
| 17 | `process::Command` | no PAL at all | 6 | 10 | `gix-command`, `gix-credentials` |
| 18 | `env::current_dir` | `Unsupported`, declined | 6 | 5 | refuses honestly; it is the namespace question |
| 19 | `Metadata::modified` | CLOSED 2026-09-19, by path | 5 | 7 | bound on `GETMTIME`, which milestone 47 (navigation)'s `touch` added on 2026-08-24; the reason column said a wire change was needed for three weeks after it stopped being true. Through an open `File` it still refuses (the verb takes a name); see notes/std.md |
| 19a | `Path::is_dir` on a directory | was always `false` | (not counted) | | closed with the five above; `create_dir_all` needed it |
| 20 | `fs::set_permissions` | `Unsupported` | 4 | 3 | |
| 21 | `TcpListener` | CLOSED 2026-08-18 | 4 | 11 | the LISTEN verb landed at milestone 107 (socket); the reason column was stale for a fortnight |
| 22 | `ToSocketAddrs` / DNS | numeric only | 4 | 5 | |
| 23 | `Metadata::created` | `Unsupported` | 3 | 3 | |
| 24 | `set_nonblocking` | `Unsupported` | 3 | 4 | contract is blocking-only |
| 25 | `fs::rename` | CLOSED, milestone 64 | 2 | 2 | bound on `RENAME`; undercounted, see BUGS |
| 26 | `fs::copy` | CLOSED 2026-08-17 | 2 | 2 | needs no verb: an open, a read/write loop, two closes |
| 27 | `fs::canonicalize` | `Unsupported` | 2 | 1 | |
| 28 | `File::set_times` | `fs::set_times` CLOSED 2026-09-19; `File::set_times` still `Unsupported` | 2 | 1 | `SETMTIME_AT` sets by name, needing `dir::WRITE` and `dir::SETTIME` (§112 (touch's two behaviors need two rights)); a handle has no name, so the `File` form is design/roadmap/0504-an-mtime-for-an-open-file.md |
| 29 | `File::try_clone` | `Unsupported` | 2 | 1 | a handle is one session's token (§27) |
| 30 | `File::lock`/`try_lock` | `Unsupported` | 2 | 1 | `gix-tempfile` |
| 31 | read/write timeouts | `Unsupported` | 1 | 1 | |
| 32 | `Metadata::accessed` | `Unsupported` | 0 | 0 | nobody asked for it |

### The later passes, and what each found

Milestone 64 worked the list in four passes. The full reasoning for every closure and every
refusal is in [crates-io-on-nife/later-passes.md](crates-io-on-nife/later-passes.md).

- The second pass (2026-08-17) closed `getrandom`, `env`, `File::set_len` and `fs::copy`, and found
  `fs::symlink_metadata` already bound. `env::vars()` had been a `panic!`. It declined
  `std::os::unix`, `thread::spawn` (a design fork), and the rows nothing in §27 (the filesystem service) backs. It also
  declined everything that needs a namespace to resolve a path against.
- The third pass (2026-08-18) asked what the neighbors of each refusal do, and found three calls that
  aborted the process: `env::temp_dir`, `env::split_paths` and `process::id`. A gap list built from
  `Unsupported` counts cannot see a `panic!`. The rule it followed: fix the ones that abort, leave
  the ones that refuse.
- The fourth pass (2026-08-18) made that reading a gate, `cargo xtask std-aborts`, which at once
  found a fifth: `std::process::exit` was a trap instruction, so a clean exit arrived as a crash.
- Ranks 5, 11, 12, 13 and 25 were bindings, not verbs: the FS server already implemented every one.
  Milestone 64 bound them, and `Path::is_dir` with them. A refusal outlived its reason by two
  milestones, and nothing caught it until fifty crates were asked what they wanted.
## BUGS

- The runtime half is still not measured, for somebody else's crate. Everything in the tables
  here is compile and link; no probe has ever been booted, so no line in them is evidence that a
  *crate* works, and `tempfile` is proof that the distinction is real rather than pedantic.
  What the second pass added is runtime evidence for **the PAL surfaces it closed** (`env`,
  `set_len`, `copy` are asserted by `std_exerciser` under a real directory capability on both ISAs),
  which is a different claim. Booting a crates.io crate under a stated endowment remains the missing
  half and the milestone's own acceptance criterion. It needs a crates.io dependency in a program
  this tree builds, which is a §46 decision rather than a lane's.
- **A gap list built from `Unsupported` counts cannot see a `panic!`**, and this note went two
  passes without noticing. Three std calls aborted a nife process while compiling perfectly
  (`env::vars`, then `env::temp_dir`, `env::split_paths` and `process::id`), and the ranked list
  below could not have contained any of them, because a function that aborts never returns the
  refusal the census greps for. The method that found them is not a list at all: read every module
  the PAL falls through instead of binding, and read what its *neighbors* do.

  This is now a gate, `cargo xtask std-aborts` (milestone 64's fourth pass), and building it
  found a fifth that the reading could not have: `std::process::exit` was `intrinsics::abort()`,
  in a file that is not a module dispatcher at all. The gate's own blind spot is that it covers
  `sys/` only; see notes/std.md, "What still ends a nife process", for why that boundary and what
  it costs.
- The census counts call sites, not reachable calls. A `fs::read_dir` inside a `#[cfg(windows)]`
  block counts. The over-count is roughly uniform across rows, so the *ordering* is trustworthy and
  the absolute numbers are not. The precise version (make each `unsupported()` a distinct undefined
  symbol and let the linker report reachability) needs a farm rebuild per probe and was not worth it
  to reorder a list that would not reorder.
- `fs::rename` is undercounted, badly. It sits at rank 25 with two probes, and gitoxide renames
  constantly; it just does it through `tempfile::persist` rather than by naming `fs::rename`. Any
  row here can be wrong in the same direction whenever a crate wraps the call. Treat the ranks as a
  starting order, not a budget.
- Fifty crates is not crates.io. They were chosen to span categories and to include milestone
  99's and 66's own leaves, which is a deliberate bias toward the things this project is about to
  need. A random sample would look different and would be less useful.
- Feature flags change the answer. `flate2` was probed with `rust_backend`; its default C
  backend would land in class C. `mio` was probed with no features and passes; with any it does not.
  `clap` was probed with `std` only. Where a crate has a pure-Rust option, the probe took it, and
  that is a choice this note is making on the reader's behalf.
- **One version of one nightly.** The pinned toolchain of 2026-08-04 and the crate versions resolved
  that day. `getrandom` in particular is mid-transition across 0.2, 0.3 and 0.4 in one dependency
  graph.

## What this says about milestone 99 (gitoxide) and milestone 66 (Vaultwarden)

**Milestone 99's decision holds, and the measurement strengthens it.** The block says gitoxide's
gaps "are this project's own roadmap rather than a compatibility project". That is what was found:
`gix`'s blockers are a `getrandom` backend, `gix-sec`'s uid lookup, and `tempfile`'s persist, plus
the five fs bindings above. Every one of them is either PAL surface this tree wants anyway or a
small patch under `patches/`. Nothing in gitoxide's tree wants `fork`, and nothing wants a C library.

Three things the block does not currently say, which the measurement adds:

1. `getrandom` is the first task, not an incidental. Nothing in `gix` builds without it.
2. `tempfile` is the second, and it is invisible to a build-failure list. `gix-lock` cannot
   commit a ref until `tempfile::persist` works, which means `fs::rename` matters far more than its
   rank-25 position says.
3. **`gix-command` and `gix-credentials` call `process::Command`**, for which there is no PAL at
   all. Whether the staged `git init`/`commit`/`log` path reaches them is unknown; if it does, it is
   a design fork (this kernel spawns by capability) rather than a porting task, exactly as the block
   says for C git.

**Milestone 66 is further away than milestone 99, and the measurement says by how much.** Rocket
fails at `socket2` before any Vaultwarden code is reached, `diesel` needs `libsqlite3`, and `ring`
needs a C toolchain for this target. Those are three separate subsystems (a readiness IO model, the
C seam at library scale, and a socket contract with `listen`/`accept`), and none of them is a `std`
gap that widening the PAL would close.
