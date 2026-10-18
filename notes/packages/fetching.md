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
