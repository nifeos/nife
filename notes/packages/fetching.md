# Fetching a package by name

The appendix to [notes/packages.md](../packages.md) for milestone 198 (a package manager) rung
3a's fetch, built 2026-09-26 in the progenitor and moved into `jig` by milestone 809 (the package
client becomes a program) on 2026-10-10. An operand with no `/` is a name, by the prompt's rule for
a command word:

```
$ jig install nosuch
  refused: this image's catalog names no such package, so nothing was fetched
$ jig install uptime
  refused: this image's catalog does not vouch for those bytes; nothing was sent to the installer
$ jig install greeting@0.1.0
  fetched and installed; generation 2 is live
$ packages/greeting/0.1.0/greeting
  hello from a package this image never carried
```

## How `jig` fetches

`jig` asks its copy of the image's catalog first (`package_archive::cataloged_stem`, on the page
`grant_plan::Manifest::catalog` maps). A name the image vouches for nothing by costs no network.
Then it opens a `std::net::TcpStream` to the package source (`socket_contract::fixture`), sends
`GET /<stem>.nifepkg` and reads the reply with `http_response`. The body goes into one run of
`jig`'s own pages, sized from the declared length.

Before anything is sent, the package must be the one asked for, and the catalog must vouch for its
digest (`package_archive::installable_as`, milestone 809's item 8). A source can serve a *different*
package the catalog also vouches for, and the lying source the gate runs is refused here. Then it is
an ordinary install, on `jig`'s installer endpoint (§270 (a package manager holds an installer
endpoint, not the spawn endpoint)). The progenitor checks its own copy against its own catalog, as
for a file.

## Why `jig`, and what it cost

Until milestone 809 the progenitor fetched, for want of a way to tell a program *which* package.
Milestone 205 (how a foreign program is told what to do) gave a program an argv on 2026-09-27. The
progenitor's fetch had put `http_response`'s head reader, a parser of network input, in the most
trusted process, before any digest was checked. Now that parser runs in `jig`, and the
progenitor's only network-shaped input is a package's bytes, judged by digest as a file's are.
`package_archive::Package::parse` still runs there on unvouched bytes, as it always did.

## A program no image carries

`greeting` (`fixtures/src/greeting.rs`) prints one line. `fixtures/Cargo.toml` lists it as
`packaged_only`, which `xtask`'s `declared_programs` reads. So it is built with every fixture and
packed by no archive, and `packages/greeting*.recipe.toml` package it for all three architectures.

The prompt cannot show its absence. It is no `grant_plan::Program`, so before it is installed its
bare name is refused either way, and after, the live generation answers for it (DECISIONS §229 (how
a bare name at the prompt reaches an installed program), B2). `script/swish-check` reads the archive
on the host before the boot instead, and stops if the archive has it.

## The gate starts the package source

`script/swish-check` fills `target/package-source-<arch>/` with two packages. One is `greeting`.
The other is the tampered `uptime` the disk also gets, served under the genuine name. The gate
points `helpers/package-http-peer` there with `NIFE_PACKAGE_SOURCE`, which QEMU passes to the peer
it starts per connection. So a lying mirror is one line: a well-formed exchange of a well-formed
package that only the catalog can refuse.

x86_64 fetches over the `e1000e` its runners attach (milestone 494 (a driver for the network card
a PC actually has)), since 2026-10-05. `q35` has no virtio-mmio bus, so the kernel builds that
stack itself and grants the progenitor its endpoint and its lease
(`kernel::user::boot_e1000e_network`). The progenitor takes the lease exactly as it does from a
stack it built, and hands `jig` the same endpoint on all three. Until then the x86_64 leg installed
`greeting` from the disk and omitted the two fetch lines. After the reboot, removing `uptime`
leaves `greeting` running.

The kernel drives an `e1000e` at boot only when the part is one a gate has driven and its link is
up, and says which refusal it took. xenon's I219 is neither yet: its bring-up runs FreeBSD's
MAC-register steps, which nothing here has executed, so its booted system has no network until
milestone 494's bench boot proves the part. A link with no DHCP server behind it still holds the
prompt back, as on the virtio path (milestone 590 (the booted system starts its network stack)'s
`BUGS`).

## The HTTP client's rules

`crates/http_response` is the pure-logic half of the client: the request line, and a reader for the
response that never holds the body. Plain HTTP is enough on a LAN. §196 (nife carries TLS) rules
HTTPS for the internet rung, and under §195 (a reviewed recipe vouches for a package) a package's
digest, not its transport, decides whether its bytes may run.

The reader is incremental and holds only the head. A response arrives one socket read at a time,
at most `socket_contract::DATA_MAX` bytes each, and the body is a package that may be a megabyte.
That is the borrowing shape `package_archive` uses, for the same reason: the first consumer has no
allocator.

The head is parsed by `httparse`, the parser under `hyper`. It is `no_std`, has no dependencies and
no allocator, and is fuzzed upstream (OSS-Fuzz). Its `Partial` status tells the reader the head has
not ended yet. The policy around it stays this crate's, because a general HTTP parser is
deliberately more lenient than a package client should be:

- `Content-Length` is required. HTTP/1.0 lets a server end a body by closing the connection, but a
  client that accepts that cannot tell a complete package from a truncated one, and the net server
  reports a peer's close as a failed receive after a bounded wait, not as an end. A digest would
  still catch the truncation; refusing it says why.
- `Transfer-Encoding` is refused, chunked included. A `GET` sent as HTTP/1.0 is never owed a
  chunked reply, so a server sending one is not speaking to us.
- Lines end in CRLF. `httparse` also accepts a bare LF, and a client that does can be made to
  disagree with a proxy about where a line stops. A duplicated `Content-Length` must agree with
  itself.

The limits it carries, in full:

- The head parse is not proved here. Upstream's fuzzing and its use under `hyper` vouch for it, and
  `fuzz/fuzz_targets/http_response_feed.rs` checks that how the reads fall never changes the answer.
  No Kani harness covers the head: the reader is not on the verification path, so
  design/decisions/0046-dependency-rule.md takes the parser rather than proving one, and the harness
  the hand-written parser had never finished in a CI shard's minutes.
- `httparse` parses into a caller-supplied array and this crate has no allocator, so a head with
  more than `MAX_HEADERS` lines is refused even under the byte cap. The array lives on the stack for
  one parse (4 KiB).
- `httparse` does not resume, so the head is parsed again on every read until it ends. That is
  bounded by the head cap, quadratic in it for a peer that sends one byte per read, and not measured.
- Header values other than the two it reads are skipped, so a malformed `Date` costs nothing; a
  malformed line with no colon is refused.

## What proves it

Green on aarch64, riscv64 and x86_64 (OVMF) on 2026-09-26. Each change below was made once on
aarch64, and each turned its line red:

- skip the catalog lookup: `nosuch` reaches the source and gets a 404;
- skip the fetched package's digest check: the lying `uptime` installs;
- drop the body: `greeting` is refused;
- place the program with a byte flipped: it is refused when run;
- make `remove` drop every program: `greeting` is refused after the reboot;
- give the second boot a fresh disk: every line after the reboot fails;
- empty `packaged_only`: the seed refuses the archive.

On x86_64, seeding no `greeting` package failed its disk line. A first line, `greeting` typed bare
and expecting "no such program", stayed green with `packaged_only` emptied. That is how the archive
check replaced it.
