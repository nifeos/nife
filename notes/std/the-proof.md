# The proof: one binary, three behaviors

*An appendix to [`notes/std.md`](../std.md), which is the page to read. This file holds what
`std_exerciser` does under each grant, which kernel tests spawn it, and the small `std` programs
beside it. The directory `notes/std/` and this file's stem are provisional names. Naming is
calef's.*

The records this file cites by number:

- milestone 122 (a directory handle `std` can hold)
- milestone 31 (a capability shell)
- §27 (the filesystem service)
- milestone 64 (enough `std` to run somebody else's crate)
- milestone 47 (navigation and naming)
- §19 (architectural parity is a tenet)


## The proof

`std_exerciser/src/main.rs` is an ordinary Rust program, no `no_std`, no `unsafe`, and two
`#![feature]` gates that are both about an API's stability upstream rather than about this
platform: `std::random` (rust-lang/rust#130703) and, since milestone 122, `std::fs::Dir`
(rust-lang/rust#120426). A program on any target calling those opts in the same way. It is
one binary with three behaviors, chosen by the authority it was granted: on start it probes for
a directory capability (`File::open` on the fixture name) and then for the network (a single
`UdpSocket::bind`), and the results branch it.

- Granted a directory (slot 4 and the shared page, alongside a running FS service): the open
  succeeds. The program reads the file with `Read` and again with `read_to_string`, stats it,
  overwrites the image's `scratch` file and reads it back, and gets refused on `/etc/passwd`,
  `../motd`, and `sub/motd`. Since milestone 31 phase 2 it also creates a name the image does not
  carry with `std::fs::write`, writes it a second time with a *shorter* payload, and asserts the
  read-back equals the shorter one. Without `TRUNCATE` that second write would leave the first one's
  tail behind, which is the whole of §27's four-times-corrected bug pinned at the top level rather
  than only in a host test. `create_new` over the name it just made is `AlreadyExists`, and creating
  `/tmp/escape` or `../escape` is refused exactly as opening them is, so `CREATE` did not widen what
  a client can reach. Since milestone 64 it then walks the namespace verbs. It makes a directory,
  lists the granted directory and finds both the fixture (a file) and the directory it just made
  (marked as one), and descends into that directory and finds it empty. It gets refused unlinking a
  directory and rmdir-ing a file. It renames the file it created and asserts the *source name is
  gone* as well as the destination's contents, then removes both. Since milestone 122 it then
  descends. It reads a file one directory down and another two down, builds a small tree of its own
  and lists it. It opens every file that listing named through the `path()` the listing handed back
  (the pair that used to break). It gets refused a path that walks through a file, opens a file
  under a held `std::fs::Dir` and gets refused `..` through it. It renames between two directories
  in one message, and removes the tree with `remove_dir_all`. The tree it lists is one it built,
  deliberately, and not the fixture's `sub`. Milestone 47's directory-capability attacker is granted
  exactly that directory and writes into it. So `sub`'s contents depend on which tests ran first in
  this boot, so the transcript does not assert them. `sub/inner` and
  `sub/deeper/leaf` are safe to *read* because the post-run host check pins them. It cleans up
  before it starts rather than after, because `NIFE_KEEP_REDOXFS=1` runs the suite against an image
  a previous boot wrote. The kernel test
  `std_fs_reads_a_file_through_a_granted_directory_capability` spawns it this way.
- Granted the network (slots 2 and 3, alongside a running net_stack): the bind succeeds. The program
  does a UDP round trip to slirp's own TFTP server and a TCP echo round trip to slirp's guestfwd
  peer, both through `std::net` and both asserted. The kernel test
  `std_net_runs_over_the_socket_contract` spawns it this way.
- Granted neither (only slots 0 and 1): both probes return `Unsupported`. The program runs
  the offline transcript, exercising `Vec` (10,000-element collect against the untyped heap),
  `String`, `HashMap` (the random seed), `Instant` (asserted monotonic and advancing), and the
  honesty of `fs` and `net`. The kernel test `std_tests::a_whole_std_program_runs_on_the_native_abi`
  spawns it this way.

The same binary doing three things by its grants alone is the point of "no ambient authority": the
code never chose to have a network or a filesystem, its capability table did. All three tests
reassemble the byte stream off the endpoint and compare it byte for byte, on both ISAs out of each
arch's own initrd (the parity gate, DECISIONS §19). The fs transcript splices the file's own bytes
into the expected buffer from the shared fixture. So that one comparison covers the whole path:
disk, DMA-confined block server, FS server running an engine we did not write, the file contract,
the PAL, and the stdout endpoint. One binary also keeps the initrd inside its nifefs directory limit
(`nifefs::MAX_FILES`, 31 entries when this was written and 76 since 2026-08-01). `cargo xtask test`
builds the demo for both targets first, so both initrds carry it; both test legs attach a virtio-net
NIC (`NIFE_NET`) with the guestfwd echo peer and the RedoxFS image as the second disk.

One boot has one FS service, because the block server owns the RedoxFS device: a second wiring would
put a second driver on the same virtio slot and re-bind its interrupt. So `fs_service` remembers
what it wired, and the hand-written client's test and the `std::fs` test share one instance;
whichever runs first receives the two readiness sentinels (each is sent once) and the other sees
`None` and skips those assertions. That keeps the two tests order-independent, which matters because
nothing guarantees which of them the harness runs first.

## Why each check is shaped as it is

`std_exerciser`'s comments give the constraint at each check. The reasons behind them are here.

On every run:

- The first `thread::sleep` retypes its timer and notification from slot 0.
- At the prompt the shell always sends an argv, so the name the program was run by comes first.
  The kernel harness grants none, so its transcript prints `[]`.
- `SystemTime::now()` needs only the clock page's offset and the counter register. The check is the
  clock service's own sanity window, which 1970-plus-uptime fails. `SystemTime` and `Instant` are
  read from the same counter, so the wall clock advancing does not mean the monotonic one jumped.
- The entropy endpoint names no device; only the entropy service reads the virtio-rng. Comparing
  two draws is what says the bytes came off a device.
- `std_service::start_on_full` maps the inert-configuration page unconditionally, as it maps a
  clock. A process granted no page gets the same honest absence `PATH` shows.
- `std::env::vars()` is counted, because a count proves the call returned. More than the three
  seeded keys would be ambient authority. `set_var` must take, because a library configured through
  the environment is ordinary.
- `join_paths` and `split_paths` are pure string work with no platform in them. `current_exe`
  refuses because nothing tells a process the path it was loaded from, and `home_dir` is `None`.

Granted a directory:

- A path that would leave the held directory is `InvalidFilename`, because nothing checked a
  permission: there is no such name here. An expressible name that is absent is `NotFound`, and the
  difference is what makes the refusal legible.
- Write-a-temp-then-rename is how a careful program replaces a file. The destination must appear
  with the source's contents and the source's name must go in one step, so both are asserted.
- Two remove verbs, each refusing the other's kind, are the safety property. A single "remove
  whatever you find" is what makes `rm -r` dangerous.
- `create_dir_all` recovers from `AlreadyExists` by asking whether the name is a directory, so it
  is idempotent only if a directory can be stat'd.
- `set_len` is checked growing as well as shrinking, because `ftruncate` does both. `copy` must
  deliver the source's bytes, which a copy loop with a bad offset fails. The source must survive,
  because the wrong way to write `copy` on a contract with `RENAME` is to reach for it.
- The fixture's `motd` was stamped by the host tool in real seconds, so a binding that returned 0, a
  size or a handle number fails. A file nife makes is stamped by the FS server's counter, which
  restarts at each mount, so it reads as early 1970 and only its order means anything. The
  half-second in the time it sets pins truncation, since the wire carries whole seconds.
  `GETMTIME` works on a directory, and `DirEntry::metadata` asks by name for a directory as for a
  file, so the listing's entry and the name must report one time.
- The time refusals each mark a place where a success would be a lie. Reading through an open
  handle is refused because the contract asks by name. Setting through a handle would stamp
  whatever holds the name now. No verb sets an access time, so the mtime beside it must not change
  either. The granted directory has no name to ask by.
- The fixture's `deeper` exists so a second descent has somewhere to go and `dir::DESCEND` has
  something to withhold. A move between directories is one `RENAME` carrying a handle on each side.
  `Dir::open(".")` costs no message, because it is what the endpoint is bound to.
- Without `ENUMERATE` a named file still opens: reaching what you can name is `READ` and `DESCEND`,
  and `ENUMERATE` is only the power to find out what else is there. A `grep -r` that matches zero
  files because it could not look is the worst failure such a tool can have.

Granted the network:

- The UDP check uses slirp's TFTP server because 10.0.2.3:53 is not a resolver. slirp NATs it to
  the host's name server, and the check then flaked at about 2.5% per query. The same hazard had
  two call sites, `socket_test_client` and this program, and each needed the fix.
- A failed check faults rather than printing a status, so the transcript stays byte-stable and a
  failure reads as a missing line.
- `denied refused` matters most: a program that could bind an ungranted port would make inbound
  authority decoration, and no other line would show it. Exclusivity, `in use refused`, is what
  makes a port grantable. A listener that went deaf after one connection would pass a one-round gate
  and fail the Samba-shaped workload of milestone 55 (Time Machine: SMB3 with Apple's extensions,
  and mDNS). The guest and the host prober are one gate only together.
- A round keeps accepting through `WouldBlock`. On sifive-u54 and rva22s64 one 15 s window opened
  while the prober still held an earlier connection, and round 0 failed with nobody having tried
  (milestone 649 (every client of a network stack shares its socket numbers), cpu-matrix,
  2026-10-07 UTC). `ACCEPT_TRIES` covers the prober's longest hold, about 23 s, with room to spare.

## The small `std` programs

`std_exerciser/src/bin/` holds programs too small or too specific for the main binary.

- `std_echo` is the smallest `std` program whose manifest note says it hears words. `std_exerciser`
  is too big to travel as an image, and `ripgrep` is ten times that. Its line proves the shell read
  the note and sent the argv, the progenitor sized a `std` region from the argv bit and built the
  layout, and `args_os()` read the page. The `{:?}` form keeps a byte that is not text visible.
- `std_grep` behaves like `rg` at the prompt for the designation half, since `rg` cannot travel as
  an image and is not built in CI. Every name it opens was designated by a word and granted
  read-only. Its errors print the kind alone: `Unsupported` is "no directory was granted",
  `NotFound` is "no such name here".
- `std_heap_contention` reads as a ratio, not a speed: the heap is one lock, so four threads on four
  cores taking no less time than one is contention. A full allocation cache holds 34,816 bytes, and
  64 of them are twice the budget `system_tests` grants.
- `std_rayon`'s count matches the one the four-thread half of the exit test reaches by hand.
  `rayon::broadcast` returns only once every worker has run its closure, so the distinct workers it
  reports are threads that really ran.
- `std_reach` is installed by answering `jig install`'s question `y`, so the installed-programs
  table records a `grant` line for the network and one for the zone. Neither of its checks needs a
  name server the runner cannot provide.
- `std_threads` is the program any Rust user would write, with nothing nife-specific in it. Joining
  the first thread early and using the shared state after shows the address space outlived it.

## What the offline transcript's lines guard

`std_tests::a_whole_std_program_runs_on_the_native_abi` grants a heap, a stdout endpoint, a clock
and a configuration page. Several lines of its transcript guard a call that once compiled and then
killed the program, so a test that only checked for no crash would have passed:

- `wall clock ok` (milestone 51 (wall-clock time)): `SystemTime::now()` reached a real time through
  a read-only page and the ambient counter, inside the clock service's sanity window, not 1970 plus
  uptime.
- `config seeded` (milestone 47 (navigation and naming), DECISIONS §111 (inert configuration is a
  read-only page)): the kernel assembled the inert-configuration page
  (`std_service::start_on_full`, `CONFIG_SLOT`, `CONFIG_PAGE_STD`) and `pal::nife::init` read back
  exactly `TZ`, `LANG` and `TERM`.
- `env ok` (milestone 64 (enough `std` to run somebody else's crate)): `std::env::vars()` returns,
  holds exactly the three seeded keys, and `set_var` takes.
- `paths ok`: `temp_dir` is the granted directory and `TMPDIR` steers it, a path list survives a
  join and a split, and `current_dir`, `current_exe` and `home_dir` still refuse.
- `entropy ok` (milestone 56 (secrets, credentials, and the entropy to make them safe)):
  `std::random` reached a virtio-rng device through one endpoint, and two draws differ.
- `exiting through process::exit`, and the two assertions after the transcript: `std::process::exit`
  ends the program as an exit, not a trap. `sys/exit.rs` is not a `sys/<module>/mod.rs` backend, so
  `cargo xtask std-aborts` is what finds such fall-throughs.

## How the tests read a std program's stream

`std_tests::drain_sink` is the one decoder for every sink in the suite. It ends only on the sink's
end-of-stream marker, which the runtime's `cleanup` sends after `main` returns. A program that
panics under `panic = "abort"` traps instead. Every byte it printed, the panic message included,
reaches the endpoint and is never read. The caller blocks until the watchdog fires, and the
transcript reads as empty, which is indistinguishable from a program that never started. A program
can work around it with a panic hook that prints and then calls `process::exit`, as
`cryptography_exerciser` does. `drain_sink` stays for callers whose starter does not hand back the
writer's thread, and each of those is a hang waiting for its first abort.

`std_tests::drain_sink_from` is for a caller that knows the writer's thread (milestone 514 (the HVF
std listener hangs in some full runs)). Without it, a writer that aborts leaves the reader parked,
and the suite's watchdog fires a minute later and calls it a lost wakeup. The calling thread binds a
notification and arms a timer on it every `WRITER_CHECK_MS`. A bound notification ends a `RECEIVE`
(§101 (notification objects), the `x4 == BOUND` tag), so the receive returns a sink message or a
tick. On a tick it asks whether the writer's thread still resolves, and a name is generational, so
"gone" means gone. It is a poll, not an event, because an unsupervised thread's death signals
nobody. Making it an event would mean spawning the writer supervised, which changes who reaps it.
`echo` prints each line as it arrives, indented under the test's own line, so the host relay can
stamp it on the inbound prober's clock (`xtask/src/inbound.rs`). The comparison is made on the bytes
either way.

The tick is a wake, and a wake is progress to the suite's lost-wakeup watchdog
(`testing::watchdog_tick`). So a writer that is alive but stuck runs to the test's own budget (90 s
by default) and is reported as a livelock, where without the tick the watchdog would trip at 60 s.
The dump that follows names the stuck thread either way.
