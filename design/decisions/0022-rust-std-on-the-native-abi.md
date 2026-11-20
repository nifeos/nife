---
status: AMENDED
raised: 2026-07-28
decided: 2026-07-28
ratified_by: calef
---

# 22. Rust `std` on the native ABI, the Hermit way (milestone 27)

(the create/truncate half of the `Unsupported` list is superseded below.)

Decided and built 2026-07-28. Full write-up in notes/std.md; this records the decision and the
forks inside it.

**The decision: implement std's platform layer (`sys`) directly on the capability ABI, not a POSIX
shim under the Unix one.** This is Hermit's shape (std on a non-POSIX unikernel ABI). DECISIONS §15
(the native ABI) already priced the alternative (Redox's relibc-first road) at "later, if ever, and
at no cost to defer". A std program draws its heap from an untyped budget at slot 0, SENDs stdout to
an endpoint at slot 1, and reads `Instant`/`SystemTime` from the virtual counter. It gets honest
`Unsupported` from `thread::spawn`, `fs`, and `net` until the servers that back them exist
(milestones 30 and 32). No new system call and no new capability method. The PAL is a client of the ABI
as it already stands, the same surface `allocator_exerciser` proved. `panic!` prints and faults
(panic=abort; unwinding is never linked), which is this ABI's honest `abort()`.

Why now: the first wall an application hits on nife is "no std". And milestone 23 (a
capability-routed component OS with live replacement)'s vendor-component ambition needs components
writable by people who are not kernel people. std on the native ABI widens "runs real workloads" to
most of crates.io that stays off files and sockets. It does so without smuggling in the POSIX
assumptions (no fork, no open-by-path, no ambient anything) the ABI deliberately excludes.

**The one genuinely new thing is build machinery, and its forks were settled by measurement, not
taste.** `-Zbuild-std` reads std's source from the sysroot of the rustc it invokes. So a patched std
means a toolchain whose sysroot is patched. Three approaches were on the table; the empirical result
chose:

- *Symlink farm* (link a fake toolchain, symlink lib, real patched src): **rejected, measured to not
  work.** rustc derives its sysroot from the resolved location of `librustc_driver`. A symlinked
  dylib resolves back to the real toolchain, so build-std read the unpatched src.
- *In-place patch of the shared rustup toolchain*: rejected. It mutates a shared, rustup-managed
  directory (a surprise `rustup update` would clobber, and it clobbers what other projects build
  against). The "never clobber" discipline warns against that.
- *Hardlink-clone the toolchain* (`cp -al` bin+lib, real copy of just `src`, patch that): chosen.
  The clone's `librustc_driver` lives inside the clone, so rustc resolves the clone as its sysroot.
  Blocks are shared, so the disk cost is near zero. The real toolchain is never touched. `cargo
  xtask std-src` builds and links it as `nife-dev`.

**Target specs, not real targets** (roadmap's "a spec first, a real target later if ever"): custom
JSON with `os = "nife"`, `panic-strategy = "abort"`, softfloat, and `singlethread = true`. That last
one is honest for phase one, one thread of execution per process, so std uses its `no_threads` sync
and single-`static` TLS. That setting flips off when `thread::spawn` becomes real. The ABI numbers
and the heap algorithm are generated verbatim into the patched std from `crates/abi` and
`crates/user_heap`. So they have exactly one definition and cannot drift.

Accepted costs, recorded. `SystemTime` is monotonic-since-boot rather than wall-clock (no RTC).
`std::random` is a non-cryptographic splitmix64 (no entropy source). Stdout and stderr interleave on
one endpoint. The `std-src` patches are string-anchored to the pinned nightly's std internals. That
coupling fails loudly on a rustc bump (the intended tripwire) rather than silently. Proven by a real
std program (`Vec`, `String`, `HashMap`, `println!`, `Instant`) spawned as a workload and checked
byte for byte on both ISAs (the §19 parity gate).

Amendment (phase two, 2026-07-28): `std::net` binds to the socket contract. `std::net::TcpStream`
and outbound `std::net::UdpSocket` now work, backed by net_stack over the §25 (socket identity)
socket contract. The `net honestly unsupported` line of phase one is retired. The PAL
(`sys/net/connection/nife.rs`) is a pure client of the frozen contract, no new system call and no new
capability method. It holds a `Stack` endpoint (slot 2) and a frame untyped (slot 3), mints a shared
frame per socket, and drives net_stack with `socket_proto` `CALL`s. The wire constants are generated
verbatim from `crates/socket_proto/src/lib.rs` into the patched std, the same anti-drift discipline
as the ABI and heap crates. A std program does networking only if it holds those two slots. Without
them `std::net` returns `Unsupported`, which is "no ambient network" (§10 (process model)) made
visible from inside a process. The same `std_exerciser` binary proves both. Spawned without the net
slots, it runs the offline transcript. Spawned with them (and a running net_stack), it does a real
UDP DNS query and a TCP echo round trip, each asserted byte for byte on both ISAs. `TcpListener` is
not a gap: `bind` is `OP_LISTEN` and `accept` is `OP_ACCEPT` into a second id. A program without a
listen grant gets `PermissionDenied` rather than `Unsupported`. That is the difference between an
authority it was refused and a platform that cannot. The honest gap in its place is an accepted
connection's peer address. `accept` must return a `SocketAddr` and `OP_ACCEPT`'s reply carries no
peer, so it answers `0.0.0.0:0`. Remaining gaps carried as `Unsupported`: non-blocking mode and
timeouts (blocking-only contract), DNS resolution (`lookup_host`; numeric addresses only), and IPv6.
One finding reported up: net_stack ties a socket's local port to its socket id. So reopening a
closed id reuses its port and can stall against slirp. The fix is ephemeral local ports in
net_stack, a contract-side change (notes/std.md).

Amendment (phase two, 2026-07-29): `std::fs` binds to the FS-service contract, and a path means
"under the directory I hold". `std::fs::File` now works, backed by the §27 (the filesystem service)
FS service. The `fs honestly unsupported` line of phase one is retired for a program that was
granted a directory. The PAL (`sys/fs/nife.rs`) is again a pure client of a frozen contract, no new
system call and no new capability method. `crates/fs_proto` is generated verbatim into the patched std.

The design question, and the answer. `File::open` takes a path and this system has no global
namespace, so the binding had to decide what a path *means*. Per §27, open-by-path exists only
inside the server, resolved against the one directory node the client's endpoint is bound to. So the
mapping is: a std program holds a directory capability at slot 4, and `File::open("foo")` means
"foo, under the directory I was granted." Everything else follows. It is enforced client-side before
a byte reaches the wire, so a would-be escape becomes a legible error rather than an `ENOENT`:

- An absolute path names the root of the caller's own namespace (milestone 47 (navigation and
  naming), 2026-08-18). Any `..` that would climb above it is refused as
  `ErrorKind::InvalidFilename`, with a message naming which case it was. `/` in a system where every
  program holds its own directory capability is Plan 9's answer, not a global root. So two programs
  in two subtrees resolve the same absolute token to two different files, and neither can name the
  other's. A nested path is a chain of attenuated descents (milestone 122 (a directory handle `std`
  can hold)), one `OPENDIR` per component, each hop asking for `DESCEND` plus what the final verb
  needs. The grant is exactly as tight as it was, because a child's rights are its parent's
  intersected with the request. Deliberately **not** `PermissionDenied`: nothing consulted a
  permission. There is no name here for what was asked, because no capability designates it. Mapping
  a capability refusal onto EPERM would smuggle in exactly the POSIX fiction this milestone exists
  to avoid. A name that is expressible but absent stays an ordinary `NotFound`, which is what makes
  the difference legible.
- A program with **no directory capability gets `Unsupported` from all of `std::fs`**, the same
  shape the net half uses without a `Stack` capability. Detecting that cannot touch the shared page
  (an ungranted program has none mapped). So the probe is a payload-free `FSTAT` on an impossible
  handle. The kernel refuses the invoke or the server answers `-EBADF`, and only the latter means a
  filesystem is reachable.
- The slot convention now has a gap, and the gap is load-bearing. A program granted a directory but
  no network holds slots 0, 1, and 4, because empty 2 and 3 are how `std::net` knows it has no
  network. `Spawn.grants` fills from zero and cannot express that. So the kernel gained
  `sched::grant_at`, the same explicit-slot move `Tcb::CAP_INSERT` already offers a userspace loader
  (§26's fault slot uses it). notes/abi.md §4 records the convention.

Bound: open, read, write, seek, `metadata`/`len`, close on `Drop`, plus `metadata`/`read`/`exists`
by name. **Honestly `Unsupported`, because the contract has no verb for them**: creating a file and
truncating one, directory iteration, `mkdir`/`unlink`/`rename`, symlinks and hard links,
`canonicalize`, permissions, file times, locks, and `duplicate`. *(The create and truncate half of
this list is superseded by the phase-2 amendment two paragraphs down; the rest still holds.)*
Because creating and truncating are unsupported, `std::fs::write` and `File::create` are Unsupported
by construction, and writing means opening a file the image already carries. Proven on both ISAs
(§19 (architectural parity is a tenet)) by the same `std_exerciser` binary, now with three behaviors
chosen by its grants alone. Its stdout is compared byte for byte with the file's own bytes spliced
in from the shared fixture. So one assertion covers disk, block server, FS server, contract, PAL,
and endpoint.

Two things reported up rather than built (see §27's amendment and notes/std.md). The first is adding
`CREATE` and `TRUNCATE` verbs to the contract, which is what `std::fs::write` needs. The second is
the overlap between the wire's negated-errno space and the kernel's invoke-error space (-1..-8),
where `-2` is both `ENOENT` and `WrongObject`.

Amendment (milestone 31 (a capability shell) phase 2, 2026-07-30): `File::create` and
`std::fs::write` work. The first of those two reported items is built (§27's amendment carries the
contract side). So the PAL binds `create`/`create_new` to `CREATE` and `truncate` to `TRUNCATE`, and
the "creating a file and truncating one are honestly Unsupported" line above is retired. The order
in `File::open` is POSIX's, and it matters. Open, then create only if the open reported `NotFound`
and the caller asked for it, then truncate after a successful open. `std::fs::write` is
`create(true).truncate(true)`. So getting that order wrong would leave the old tail behind on
exactly the path that exists to *replace* a file's contents. That is the day-costing confusion §27
records being corrected four times. A `create_new` over a name that exists closes the handle the
probing open minted and returns `AlreadyExists`, rather than leaking it for the life of the process.
The error path is the one nobody exercises, so it is the one that leaks.

Creating a *file* was never what §27 kept host-side. That was creating a *filesystem*, which needs
uuid and getrandom. `Transaction::create_node` is not std-gated, so a file is made on-device without
entropy ever becoming a userspace dependency. The read of §27 that conflated the two is corrected
there.

Still Unsupported, each because no verb backs it: directory iteration, `mkdir`/`unlink`/`rename`,
symlinks and hard links, `canonicalize`, permissions, file times, locks, and `duplicate`. And a
program holding a per-file grant rather than a directory (§27's caretaker) sees the narrowing
through ordinary `std::fs` errors. The one granted name opens, any other is `NotFound`, and a write
through a read-only grant is `ReadOnlyFilesystem`. No std API had to change to express that, which
is the point of having bound the PAL to a capability contract rather than to a namespace.
