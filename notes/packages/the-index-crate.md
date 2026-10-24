# The index crate: one resolution, versions, and an image's sources

An appendix to [notes/packages.md](../packages.md). It carries the design behind
`crates/package_index` that [the-index-format.md](the-index-format.md) does not. That page holds
calef's rulings on the index (Q1 to Q5 on #1884, and the canonical-address ruling on #2020), and
[over-the-internet.md](over-the-internet.md) holds how a client fetches. The crate's doc comments
link here.

## What the crate is

A distribution's package index says which packages exist, the digest each must hash to, and where
its bytes may be fetched. §250 (an image names its distribution's package index, and the bytes may
live anywhere) splits what the image's catalog holds together. The index comes from the
distribution's repository. A package's bytes come from wherever the index says, believed only if
they hash to the index's digest (§195 (a reviewed recipe vouches for a package)).

The encoding is a stand-in, not TUF. `Index::parse` reads the catalog's line followed by zero or
more locations. The TUF client of milestone 858 (lab machines update themselves through packages)
replaces that function and nothing above it. `accept` judges fetched bytes with the progenitor
installer's own check, `package_archive::installable_as`, handed the entry as a one-line catalog.

## Resolve once, dial only what was checked

`Location::check` applies Q1's safeguard to one resolution of a listed location's host. Every
resolved address must be one `public_address` admits, or the location is refused. On success the
returned `CheckedLocation` holds that very slice, and its addresses are the only ones a client
connects to for this location.

The shape closes the DNS rebinding escape that milestone 871 (a sixth outsider pass attacks the
confinement claim) booted. A client that checked one resolution and then connected by name resolved
a second time. A rebinding name server answered the check with a public address and the connect
with a private one. Resolving once and dialing only what was checked is curl's `CURLOPT_RESOLVE`,
and Go's `net.Dialer.Control`, where the check runs on the address actually dialed. calef ruled it
on #1901 (2026-10-10 UTC). The host name stays in the `CheckedLocation` for TLS alone: it is the
server name a client sends and the name the certificate must carry, never a name to resolve again.

Any address that is not IPv4 is refused, because `public_address` judges only IPv4, and an address
nothing judged must not reach a dial. An empty resolution is refused too, since there is nothing to
connect to.

A host whose last label is a number must be canonical dotted-decimal IPv4, or it is refused
(the-index-format.md has the ruling and the CVEs behind it). The test for ending in a number is
WHATWG's: the last label is all decimal digits, or `0x` and hex digits. A refusal means nothing
downstream has to agree with the crate about which address a host is, so the resolved-address check
stops being the only guard. `is_host` has already admitted the host, so a part holds only letters,
digits and hyphens, and `u8`'s parser refuses every one of those that is not a decimal number up to
255.

## Versions in Debian's order

calef ruled on 2026-10-18 (UTC), asked how `jig` should order versions: "Debian dpkg order". That is
the order `jig list -u` reports against (milestone 809 (the package client becomes a program)), and
the one the reserved `upgrade` verb will install by.

A version is what a stem carries between its name and its architecture. So it is dpkg's upstream
version with neither an epoch (`1:`) nor a revision (`-1`): a stem is split at its hyphens, and an
epoch's colon has no meaning without the rest of dpkg's syntax. `version::check` admits a version
only if dpkg would read it without complaint. It refuses everything dpkg refuses or warns about: an
empty string, a first character that is not a digit, or any character outside `0-9 A-Z a-z . + ~`.
basalt writes these strings and `jig` reads them, so a string one side could not order is refused
where the index is read, not guessed at. `jig update` then keeps no copy that `jig list -u` would
have to guess about.

`version::compare` is dpkg's `verrevcmp` (`lib/dpkg/version.c`). The strings alternate between runs
of non-digits and runs of digits. Non-digits compare character by character, letters before other
symbols, and `~` before everything, even the end of the string. Digits compare as numbers. So
`1.10` is newer than `1.9`, `1.0~rc1` is older than `1.0`, and `1.0` and `1.00` are the same
version.

## An image's index sources

§250 puts the index's address in the image, and calef's Q2 on #1884 gave it a backup address tried
when the first stops answering. Both live in the image's `package_index_sources` archive entry, as
text the archive build writes. The entry is packed before the measurement table, so the kernel's
table vouches for it as it vouches for `package_archive::CATALOG`. The image, not the program,
names its index, so basalt can compose an image that names another without rebuilding `jig`
(milestone 909 (basalt composes the bootable image from its packages)).

```text
# comments and blank lines are skipped
repository basalt.nifeos.org 443 rolling
repository basalt.test 8443 rolling 10.0.2.9
root isrg-root-x1
location-root sectigo-root-e46
```

- `repository <host> <port> <channel> [<address>]` appears once per index address, in the order
  they are tried. The optional address is where to connect for that host, asked of no resolver, as
  curl's `--resolve` does. It exists for the test image. Under QEMU's slirp every name is asked of
  the host's own resolver, which knows no test host (measured by milestone 892 (a std program at the
  prompt holds the network and a name resolver)). Pinning an IP address instead is refused by
  `pinned_tls_client`. The host stays the TLS server name either way.
- `root <name>` or `root <hex>` is the one root every repository must chain to. §196 (nife carries
  TLS) clause 4 holds one root per source, and question A of the-index-format.md recommended one
  pin. A name is a root the client carries, such as `isrg-root-x1`, §250's production pin. Hex is a
  whole certificate in DER, for a root no client carries, such as the test authority.
- `location-root <name> [<root>]` names a further root the signed index may name for a listed
  location (calef's C2 on #2048). A bare name is one the client carries, such as
  `sectigo-root-e46`. At most `MAX_LOCATION_ROOTS` may appear.

Every line must be read or the whole text is refused, `Index::parse`'s rule. There must be at least
one repository and exactly one root. Every name in this format, the entry's and the keywords
included, is provisional.

## basalt's production sources

An image built for real machines names `basalt.nifeos.org` on 443, on the provisional channel,
pinned to ISRG Root X1, as §250 rules. calef ruled on 2026-10-12 (UTC) that the host is GitHub Pages
under that custom domain. `nifeos.github.io` chains to X1 (measured 2026-10-13 UTC), so the pin
holds there. The second address is `basalt.crickertech.com`, tried when the first stops answering.
§250 clause 6 puts it on another registrable domain, and calef chose `crickertech.com` on
2026-10-21 (UTC). One root covers both (clause 10). The test image carries `helpers/tls-peer` and
the test authority instead, and xtask's `image_index_sources` chooses between the two.
