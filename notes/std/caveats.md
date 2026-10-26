# Honest caveats: what is Unsupported, and why

*An appendix to [`notes/std.md`](../std.md), which is the page to read. This file holds the full
caveat list the main page's BUGS section summarizes, with the reasoning for each. The directory
`notes/std/` and this file's stem are provisional names. Naming is calef's.*

The records this file cites by number:

- milestone 64 (enough `std` to run somebody else's crate)
- milestone 47 (navigation and naming)
- §111 (inert configuration is a read-only page)
- milestone 154 (a process that holds two directory capabilities)
- §41 (the endpoint is the broker)
- §26 (the fault endpoint)
- milestone 51 (wall-clock time)
- §43 (reading the clock is a page)
- §42 (a filesystem declares what it offers and must be truthful)
- milestone 56 (secrets)
- §44 (entropy is a capability)
- milestone 28 (a solid terminal)


## Honest caveats (what is Unsupported, and why)

- `thread::spawn` returns `Unsupported`. The kernel has everything it needs (retype a TCB,
  configure it, start it); what does not exist yet is the std-side plumbing that makes the result
  safe: a TLS story, park/unpark on a kernel primitive, join. Phase one ships without it rather than
  shipping it wrong. The sync primitives are std's single-threaded `no_threads` implementations, and
  the allocator's spinlock is uncontended today but stays correct under future preemption.
- The environment starts with only what a grant seeded, and `set_var` is real (milestone 64,
  `sys/env/nife.rs`). A nife process inherits no variables, because there is nothing to inherit
  from: what a process holds is what it was granted, and a variable is not a capability. So
  `env::var("HOME")` is `None` because nobody gave this program a home. What a program sets on
  itself reads back, because that is what `set_var` means on every platform and nothing about it
  leaves the process.

  `TZ`, `LANG` and `TERM` are seeded from a grant (milestone 47's environment-variable fork,
  DECISIONS §111, `environment_contract`). A process granted an inert-configuration page
  (`rt::CONFIG_SLOT`, a `PageFrame` with `READ`, the same rights-ladder shape as the clock) has those
  three keys in its table from the first line of `main` onward. `pal::nife::init` reads them before
  the program's own code runs. A process granted no such page is seeded with nothing. This is the
  *inert configuration* third of the milestone's own three-way split of what Unix puts in one map.
  Names (`PATH`, `HOME`) still wait on `bind` (milestone 154's two-directory endowment), and secrets
  are answered elsewhere, by an endpoint (§41), never by a string on this table.

  This module exists because `env::vars()` used to abort the process. Without a nife backend, std
  fell through to `sys::env::unsupported`, whose `env()` is `panic!("not supported on this
  platform")`. `getenv` was already answering `None` honestly, so nothing in the type system, the
  build, or a gap list built from `Unsupported` counts said a word; the program simply died. It is
  the same lesson as `Path::is_dir()` returning `false` for every directory: the dangerous refusal
  is not the one that says `Unsupported`, it is the one that answers.
- The path half of `std::env` answers where it must and refuses where it can (milestone 64,
  `sys/paths/nife.rs`), and the split between those two is the whole design. `temp_dir` returns
  `TMPDIR` if the program set one and `/` otherwise, because `PathBuf` carries no error and
  something has to be named. `/` is `sys/fs/nife.rs`'s own answer restated (*"./motd is motd: the
  current directory IS the granted one"*), so a temporary file goes where every other file goes,
  which is the only directory the process has authority over. `split_paths` and `join_paths` are
  ordinary string work over `:`. `current_exe` returns `Unsupported` and `home_dir` is `None`:
  nothing tells a nife process the path it was loaded from, and nobody gave it a home.

  `current_dir` and the leading slash changed on 2026-08-18 (milestone 47's namespace half). `/` is
  now the root of *this process's* namespace, which is the directory it was granted, so `/motd` and
  `motd` are one file and `current_dir()` answers `/` rather than `Unsupported`. That is Plan 9's
  answer and it is less than Plan 9's: a nife process holds exactly one directory capability, so
  there is nothing for the slash to select between and no bind table to select it with. It grants
  nothing, which is the point and is asserted rather than argued. The `std_exerciser` transcript
  reads `/motd` and `motd` and compares the bytes, then asks for `/../motd` and gets
  `InvalidFilename`, because there is no level above the only root there is. The offline run of the
  same binary is the negative control: holding no directory capability, its `current_dir()` still
  refuses, since there is a difference between "you are at your root" and "you have no root" that a
  `PathBuf` cannot carry.

  `chdir` stays `Unsupported`, and now for a narrower reason than "there is no namespace": a
  process's directory capability is fixed at spawn, so `/` is the only place it can be. Moving
  would mean the PAL holding a descent handle as mutable process state and resolving every relative
  name against it, which is the shell's stack one level down and is not built.

  Two of the path functions used to abort the process, which is why the file exists at all. nife had
  no `paths` backend, and the shared fallback's `temp_dir()` is `panic!("no filesystem on this
  platform")` while its `split_paths()` is `panic!("unsupported")`. So
  `tempfile::NamedTempFile::new()` died inside `std::env::temp_dir` before it ever reached
  `tempfile`'s own "not supported" arm, and notes/crates-io-on-nife.md had recorded the wrong one of
  those two as the failure for a fortnight. It is `env::vars()`'s lesson a second time, and the rule
  that falls out of it is short enough to remember: fix the ones that abort, leave the ones that
  refuse.
- `std::process::id()` is `0` and the rest of `std::process` refuses (milestone 64,
  `sys/process/nife.rs`). This was the third `panic!` of the same sweep: `getpid` in the shared
  fallback is `panic!("no pids on this platform")`. There is no process identifier here to report,
  the syscall surface issues none, and `u32` cannot say "there is none", so `0` is the answer,
  chosen because no Unix assigns it to a user process. Every reachable call site in the fifty
  measured dependency closures is a *fork* check (`gix-tempfile` compares an owning pid so cleanup
  runs only in the creating process). nife has no `fork`, so a constant is what makes those
  comparisons right rather than merely quiet. A scheme that wanted cross-process uniqueness from a
  pid would be broken here with nothing reporting it, and a real per-process identity is a
  syscall-surface decision rather than a PAL one.
- `std::process::exit` is a real exit, and the code it carries is dropped on the floor
  (milestone 64). It was the fourth `panic!`-shaped finding of the same sweep and the worst of them:
  `sys/exit.rs`'s `_ =>` arm is `crate::intrinsics::abort()`, so a nife program calling
  `std::process::exit(0)` compiled perfectly and then executed `brk`. The kernel takes that as a
  fault, reports it on the console, and delivers `EVENT_FAULT` with a pc and a faulting address to
  the process's supervisor (§26). A clean exit arrived as a crash, and this is the way almost
  every CLI-shaped program ends.

  Nothing had noticed because the two ways a Rust program ends took different exits and only one was
  wired. `_start` calls the PAL's `rt::exit` on `main`'s return value directly, and
  `std::process::exit` is the *only* caller of `sys::exit::exit` in the whole of std. Both now reach
  the same `SYS_EXIT`.

  The code is still discarded, and that is the kernel's, not the PAL's. `sched::exit()` is
  `depart(abi::fault::EVENT_EXIT, 0, 0)`: the §26 message carries the event and the tid, and the
  two remaining words are a pc and a fault address that a clean exit has nothing to put in. So a
  supervisor can tell exit from crash and cannot tell `exit(0)` from `exit(1)`. §301 (exit status and output) decided the
  wire: `SYS_EXIT`'s first word is the status, and the kernel forwards it in the `EVENT_EXIT`
  message's third word, which a clean exit leaves zero today. Milestone 172 (a capability-native subprocess primitive) builds it. Until it lands, a nife program
  that wants to report *why* it stopped says so on an endpoint it holds, the way every other
  result travels here.
- `fs` is bound, with the gaps listed in [the fs appendix](fs.md). A program granted no directory
  capability still gets `Unsupported` from all of it, and the offline demo checks exactly that. Same
  binary, no slot 4, and `File::open` refuses with `ErrorKind::Unsupported` rather than pretending
  there is an empty filesystem to look in.
- `net` is bound, but with recorded gaps. `TcpStream`, `TcpListener` and outbound `UdpSocket` work.
  The honest Unsupported list is an accepted connection's peer address (the contract's `ACCEPT`
  reply carries no peer, so it reads `0.0.0.0:0`; reporting it is a wire change and a fork, see
  notes/net/std-tcp-listener.md). Non-blocking mode and read/write timeouts are on it (the contract is blocking-only,
  no poll verb). So is DNS via `lookup_host` (no resolver rides the contract, so `ToSocketAddrs`
  handles numeric addresses only, and a program that wants DNS does it as a plain UDP query, as the
  demo does). So is IPv6 (net_stack is IPv4-only), and so are `peek` / socket duplication /
  multicast join-leave (no contract verb backs them). `UdpSocket::recv_from` reports the connected
  peer or the last send destination as the datagram source, because the contract's `RECEIVE` does not
  carry it; that is correct for the request/response pattern the demo uses and recorded here for
  anything that assumes otherwise. Advisory knobs (`set_nodelay`, `set_ttl`, keepalive, broadcast,
  multicast options) accept and return plausible values rather than fail; they change nothing on the
  wire.
- `SystemTime` is real wall-clock time when the program was granted a clock (milestone 51,
  DECISIONS §43, notes/clock.md).
  A std program's wall-clock authority is slot 5 (a `PageFrame` capability naming the clock page,
  with `READ`) plus a read-only mapping of that page at `rt::CLOCK_PAGE`. `SystemTime::now()` is
  the offset it finds there plus the ambient monotonic counter: two loads and an add, no server
  round trip, and nothing the program can write. `Instant` is untouched and cannot be perturbed by a
  clock adjustment, by construction.

  A program granted no clock, or running on a machine that does not know the time, gets a panic from
  `SystemTime::now()`, naming which of the two it was. This is the honest limit rather than a clean
  win. `SystemTime::now()` has no error channel, so the only loud refusal available is a panic, and
  std has no way to represent "I do not know", which means a program cannot ask whether it *can*
  ask. The `Unsupported` shape `fs` and `net` use is not available here. Anything that needs to
  check first reads `clock_contract::state` off the page directly, which is what a `no_std`
  component does. The alternative considered and rejected was returning a frozen `UNIX_EPOCH`, which
  is still reporting 1970 and is exactly the confusion §42 forbids.
- `std::random` is a granted capability, and refuses loudly without it (milestone 56, §44). It used
  to be splitmix64 seeded from the virtual counter, predictable to anyone who could guess
  boot-relative time; that file has been replaced rather than patched. `SystemRng` is now a `CALL`
  on slot 6, answered by a userspace entropy service that is the only thing that can read the
  virtio-rng device. A program granted no entropy capability gets a panic, for exactly the reason
  `SystemTime::now()` does: `fill_bytes` has no error channel, and quietly substituting a
  predictable stream is the lie the milestone exists to remove.

  `HashMap`'s seed is the one caller that still degrades, to the same splitmix64 stream, and that is
  deliberate. Its promise is DoS resistance for a hash table rather than cryptographic strength,
  std's own `unsupported` backend degrades that same function, and a `HashMap` in a program nobody
  granted entropy must still work. Nothing in the file lets the weak path reach `SystemRng`. See
  notes/entropy.md for what the bytes are and, under QEMU, are not.
- stdout and stderr share one endpoint, so they interleave by 16-byte chunk. One endpoint is what
  the contract grants today; milestone 28's terminal contract owns fixing it.
- The `std-src` patches are string-anchored to the pinned nightly's std internals. A rustc bump that
  reshapes a `cfg_select!` dispatcher fails loudly in `std_patch_dispatch` ("anchor not found"),
  which is the intended tripwire: re-point the anchor, do not paper over it. `rust-toolchain.toml`
  pins the channel; the coupling is the price of build-std against a std we do not fork.

## The farm copies `rustc` and its driver rather than linking them

rustc finds its sysroot from where its own image lives. When a farm's `bin/rustc` and
`lib/librustc_driver-*.dylib` are hard links to the pinned nightly's inodes, `rustc --print sysroot`
through the rustup proxy can name the real nightly instead of the farm. A `cargo build -Zbuild-std`
then compiles unpatched std and fails in its `cfg_select!` dispatchers. The mechanism is inferred,
not proven: macOS appears to report one path for an inode with several links, so whichever link ran
last can win.

So `std_src` replaces those two hard links with real copies (`unlink_sysroot_anchors`, about 90 MB
per farm on nightly-2026-10-06) and keeps everything else hard-linked. Measured 2026-10-06 (UTC),
#1784: through the proxy (`rustup run` and `RUSTUP_TOOLCHAIN`, 30 runs each), `rustc --print
sysroot` named a copying farm 60 times of 60 and named the nightly 15 times of 60 against a linking
one. A host test, `the_farm_copies_rustc_and_its_driver_and_links_the_rest`, pins which files are
copies.

## What the pin and the farm trust

Two limits stand under every std program this tree builds. This tree accepts both, and no gate holds either. The [2026-10-26 security audit](../../design/audit-reports/2026-10-26-the-compiler.md)
records the reasoning (findings 6 and 7).

- **`rust-toolchain.toml` names a nightly by date, not by digest.** rustup fetches that date's
  channel manifest from `static.rust-lang.org` over TLS and checks each component against the
  manifest's SHA-256, and nothing here records what the manifest or `rustc -vV` said. Two machines
  that install the same name trust the same server, not the same bytes. The bump workflow defaults
  to merge (ruled 2026-09-24 UTC), so a new nightly reaches `main`, CI and every developer machine
  within about a day, with this tree's own CI as its only reader. That is the ordinary trust of a
  Rust nightly, and recording the commit hash would detect a swapped manifest only after the build
  had already run it.
- **The farm's stamp does not hash the `rust-src` it patches.** It keys on `rustc -vV`, the
  overlay's files and `STD_SRC_PATCH_VERSION`, a number a person bumps by hand when the patch logic
  in `xtask/src/farm/patch.rs` or `generate.rs` changes. A change there without the bump leaves a
  warm farm on the old patched std while the build passes; `rm -rf target/nife-farm` is the cure,
  and CI builds every farm cold. The stamp skips a missing input rather than refusing it. rust-src's
  integrity is rustup's check at install time and nothing after it.
