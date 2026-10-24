# The proof: one binary, three behaviors

*An appendix to [`notes/std.md`](../std.md), which is the page to read. This file holds what
`std_exerciser` does under each grant, and which kernel tests spawn it. The directory `notes/std/`
and this file's stem are provisional names. Naming is calef's.*

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
  does a real UDP DNS query to slirp's resolver and a TCP echo round trip to slirp's guestfwd peer,
  both through `std::net` and both asserted. The kernel test `std_net_runs_over_the_socket_contract`
  spawns it this way.
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
