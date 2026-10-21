# The index's format: a survey, and what is left to rule

An appendix to [notes/packages.md](../packages.md), owed by milestone 801 (packages over the
internet) under §46 (thin primitives or whole subsystems). Its block said nothing outside the tree
was surveyed for the index format and the building lane owes that survey. Written 2026-10-09 (UTC)
by lane `milestone/801-packages-over-the-internet`. This lane made no network call, so every source
is marked: read in this tree, read from a crate's source in the local Cargo registry, or recalled.

## The answer most of this was asking for is already ruled

Two rulings of 2026-10-07 (UTC) on #1805 settle the family. §250 (an image names its distribution's
package index)'s amendment, clause 4: "The index is a TUF repository." And milestone 858 (lab
machines update themselves)'s Fork 9: TUF, with `jig` fetching on rust-tuf over RustCrypto and a
small proven verification core in the progenitor. So the format is TUF's metadata, and the crate
rule 7 asks for is TUF's own definition plus basalt's fields inside it.

What TUF does not say, and what §250 asks of the index, is the open part. Five questions follow the
survey. Two are a format fork (where a package's bytes are named, and "moved to"). One is a
contradiction between rulings. Two are calef's by name.

## The survey

What each system's index lists, where it says a package's bytes are, and how the index itself moves.
"Recalled" means from memory and unchecked; treat those rows as leads.

| System | Index | Digest | Where the bytes are | Index moves by | Authenticity, freshness | Source |
|---|---|---|---|---|---|---|
| Debian apt | `Release` signs `Packages` per suite | SHA-256 per file | `Filename:`, relative to the archive root of whichever mirror the client chose | The client's `sources.list`; `deb.debian.org` is a CNAME | OpenPGP over `Release`, `Valid-Until` | §250 read the split; fields recalled |
| Alpine apk | `APKINDEX.tar.gz` | SHA-1 of the control part, recalled | Relative to the repository URL in `/etc/apk/repositories` | Client configuration | RSA signature inside the archive | recalled |
| Arch pacman | `<repo>.db`, one tarball | SHA-256 and a detached signature per package | `Server =` in the mirror list, relative | Client configuration | Packager keys, database signature optional | §195 read; layout recalled |
| FreeBSD pkg | `packagesite` (a catalog in a tarball) | SHA-256 per package | Relative to the repository URL | `pkg.conf`; SRV records once, recalled | Fingerprint per repository | §195 read; layout recalled |
| Homebrew | Formula JSON from `formulae.brew.sh` | SHA-256 per bottle | An absolute URL per bottle, on GHCR | Changing the API's base URL ships a new `brew` | TLS, and the reviewed formula's digest | §195 and §197 (a package is one archive file) read the trust; URL recalled |
| Haiku | `repo` file plus `repo.info` | SHA-256 per package, in the index only | Relative to the repository's base URL | Client configuration | Not signed, recalled | §197 read the `.hpkg` format |
| Nix | `.narinfo` per store path | NAR hash, and a file hash | `URL:`, relative to the cache, recalled | `substituters` setting | Ed25519 over the narinfo | §195 read |
| crates.io | Sparse index, one file per crate | SHA-256 per version | `dl` in `config.json`, a template | `config.json` at the index root | TLS only | recalled |
| TUF (PyPI's PEP 458) | `targets.json`, delegated roles | Hash and length per target | A targets base URL the client is configured with; per-target `custom` is opaque | A new root can be fetched and verified; repository moves are TAP 4 and TAP 5, recalled | Root threshold, snapshot, timestamp expiry, versions never decrease | 858's appendix read; client source read below |

Two client implementations, read in the local registry this session:

- `tough` 0.24.0 (`src/schema/mod.rs`): a target is `length`, `hashes` and `custom`, and `custom` is
  "opaque to the framework". The repository loader takes `metadata_base_url` and `targets_base_url`
  as two arguments (`src/lib.rs`). Where bytes live is the client's configuration, not metadata.
- `tuf` 0.3.0-beta9 (`src/metadata.rs`): `TargetDescription` is the same three fields.

Neither builds for nife's targets yet (858's appendix: `aws-lc-sys` and `ring`).

## What the survey says about §250

- Every system splits a signed list of digests from the bytes, and every one lets the bytes come
  from more than one host. §250 clause 2 is the mainstream shape.
- Only Homebrew writes an absolute location per package in the index. Everyone else writes a path
  relative to a base the client configures, so moving the bytes means changing every client. TUF is
  in the second group. §250 clause 2 says moving the bytes "ships no image", which the second group
  meets only if the base is not compiled in.
- Nobody but TUF has a "moved to" for the index itself. The rest move the index by editing every
  client's configuration, and Debian hides moves behind a CNAME.

## What calef ruled, 2026-10-10 (UTC)

All five questions were put on #1884 with options, and calef ruled each there; those comments are
the record and quote him. In short, with what milestone 801 built under QEMU and where the rest
lives:

| Question | Ruling | Built in `crates/package_index` | Not 801's, and where it goes |
|---|---|---|---|
| Q1. Where a package's bytes are named | "L1, with the safeguards": a signed, ordered `custom.locations` list per target as additional sources; basalt always keeps a copy in its own `targets/`, the fallback; HTTPS only; no private or link-local addresses; an owner may pin one mirror | `Entry::locations` (HTTPS only), `Index::sources` (listed, then the repository; a pinned mirror alone), `public_address` | Who signs location changes: decided with the signing setup (milestone 858) |
| Q2. What "moved to" means | "M1 plus a backup address": only a signed root names a new location, followed once the new place serves a root chaining to the trusted one; each image carries a second index address | The client tries the image's addresses in order (`package_fetch_exerciser`) | Following a root's move is the TUF client's (milestone 858); §250's wording is clause 6 (question A below) |
| Q3. basalt's root in an image | "K4, a default owner trust line": the root's SHA-256 in the form of §220 (signed builds: a vendor signs, a developer self-signs, and trusting a key is scoped), removable by the owner; §220 clause 1 and §195 (a reviewed recipe vouches for a package) clause 4 amended | nothing | The amendments are the integrator's text; the trust line is proposed as a milestone (`design/roadmap/0894-an-image-carries-its-distributions-root-as-a-default-trust-line.md`) |
| Q4. The path on the host | "Channel prefix, no architecture": `/<channel>/metadata/`, `/<channel>/targets/`, each channel its own TUF repository, all three architectures in one index | `Repository` and its paths; `PROVISIONAL_CHANNEL` is `rolling`, a name calef has not given | Naming the channel is calef's |
| Q5. `jig`'s resolver grant | "Yes, root zone, any repository": `jig` holds the root zone, recorded where granted, with Q1's safeguards; any repository whose root is an owner trust line with a §220 ceiling | The test grants the root zone, and `public_address` is what bounds listed locations | Granting it at the prompt is the proposed std-at-the-prompt milestone; installing from any repository is milestone 809 (the package client becomes a program)'s `add-index` |

The stand-in encoding stays until the TUF client exists: a line is the catalog's line followed by
zero or more HTTPS locations.

## What calef ruled, 2026-10-12 (UTC): a host that ends in a number is canonical or refused

"Agreed", to this recommendation: a host whose last label is all digits but is not canonical
dotted-decimal IPv4 (four parts, each 0 to 255 in decimal, no leading zeros) is refused outright,
with an error naming it. Before, `https://0010.0.0.1/` was read as a DNS name. WHATWG URL and
`inet_aton` read `0010` as octal (8.0.0.1) and others read decimal; that disagreement was
CVE-2021-29921 (Python `ipaddress`) and CVE-2021-29923 (Go `net.ParseIP`), and both now refuse
leading zeros, as Rust's `Ipv4Addr` does. Index URLs come from manifests the distribution writes, so
nothing legitimate is lost, and Q1's safeguard no longer rests on the resolved-address check alone.
Hex (`0x0a.0.0.1`) and short forms (`10.1`) are refused too. #2020 records the ruling and built it:
`Location::parse` returns `LocationRefusal::NonCanonicalAddress(host)`, `Index::parse` passes it out
in `BadLine::location`, and `Repository::new` refuses the same hosts. The lane also refuses a last
label of `0x` and hex digits, WHATWG's other "ends in a number" form, which the ruling's words did
not name.

## Two questions the rulings raise

**Ruled.** calef ruled both on #1884 on 2026-10-10 (UTC), and §250's amendment of that date
records them: A as clause 6 (A1, *"two domains"*), B as clause 10 (*"X1 only"*). Clauses 11 to 13
(C, below) let the signed index name a listed location's root among those the image carries.
The second domain is `crickertech.com` (clause 6). The channel's name is calef's, and it is
provisional.

**A. Does the backup address touch §250's wording?** Yes, in two places. Clause 1 says "An image
carries one fixed name", and Q2 gives it two. Clause 3 pins ISRG Root X1 "to the index host only".
Recommended: amend clause 1 to one index at up to two addresses, the second under a different
registrable domain so one domain's loss cannot take both, and let clause 3's pin cover both
addresses. If no: the backup cannot be on another domain, and a domain loss still strands a machine.

**B. Which root must a listed HTTPS location's certificate chain to?** §196 (nife carries TLS)
clause 4 holds one root per source and no system store, and Q1 makes every location HTTPS.

| Option | What it costs |
|---|---|
| R1. The index's own pin, ISRG Root X1 | A location on another authority fails and the client falls back to `targets/`. GitHub's release asset host was measured on Let's Encrypt (§250) |
| R2. A small root store for package bytes only | The store §196 clause 4 refused, and the circularity it names |
| R3. Encryption without verifying the certificate | The digest already decides integrity, so TLS buys only privacy from a passive observer, and a client that accepts any certificate is a pattern worth not having |

Recommended: R1. It keeps one root per source and costs nothing but a fallback. The exerciser pins
listed locations to the test authority now, which is R1's shape. If no: R2 or R3 is a decision on
§196 clause 4 first.

## C. How `jig` reaches a GitHub Release asset (asked on #2048, 2026-10-13 UTC)

**Ruled.** calef answered *"a"* on 2026-10-12 (UTC), C2 below; §250's third amendment records it
as clauses 11 to 13, and milestone 801's block has what was built. The boot test's six redirect
probes were falsified on aarch64 on 2026-10-13 (UTC), twice. With the redirect pinned to the
location's root instead of the index's, the off-root probe was fetched and the good one refused.
With `MAX_REDIRECTS` at 2, the second redirect was followed. Both went red, and green again
reverted. The question as it was put follows.

calef ruled on 2026-10-12 (UTC) that basalt's packages are GitHub Release assets and its index is
on GitHub Pages (milestone 801's block). An asset's `download_count` is then the first adoption
signal (milestone 576 (how many systems are out there, and what do they run)).

This corrects a premise. B's ruling, *"X1 only"*, rests on its table's line that GitHub's release
asset host was measured on Let's Encrypt. That holds for the host the bytes come from, not for the
URL a client is given.

Measured 2026-10-13 (UTC) with `openssl s_client` and `curl --http1.0`:

| Host | Chain ends at | Role |
|---|---|---|
| `nifeos.github.io` (Pages, so `basalt.nifeos.org`) | ISRG Root X1 | the index, and basalt's `targets/` copy |
| `github.com` | Sectigo Public Server Authentication Root E46, sent cross-signed by USERTrust ECC (cross certificate valid to 2038-01-18) | the stable download URL, which answers `302` |
| `release-assets.githubusercontent.com` | ISRG Root X1 | the redirect's target |

The redirect is to a signed URL whose path is 930 bytes and which expired 43 minutes after it was
fetched. `download_count` counts requests to `github.com`, so a client that skips that host is
not counted. `jig` today pins X1 for every host, and `pinned_tls_client` follows no redirect, so it
passes over the listed asset and installs from basalt's `targets/` copy: it works, and counts
nothing. Neither `api.github.com` nor `codeload.github.com` helps; both chain to the same Sectigo
root.

| Option | What it costs | Verdict |
|---|---|---|
| C1. `jig` carries a second root, Sectigo E46, for every listed location, and follows one redirect | One root (about 600 bytes, recalled) in every image. The redirect: a request buffer larger than 512 bytes, a `Location` header read, Q1's address check again on the new host. A listed location then has two roots, which §196 clause 4 refuses | Works, but wider than it needs to be |
| C2. The index names each listed location's root, from the roots `jig` carries by name (`isrg-root-x1`, and E46 under a provisional name); unnamed means X1. One redirect is followed, HTTPS only, its target held to Q1's address check and to X1 | C1's code plus one token per location in the stand-in format. This is the per-location root clause 10 left open as "a later field", and it keeps one root per source | Recommended |
| C3. The index lists the redirected URL | It lasts under an hour, its path is 930 bytes against `PATH_MAX`'s 256, and fetching it is not counted | Refused |
| C4. Keep X1 only; `jig` fetches from Pages, and counting waits for a host with logs | Nothing to build. The Release counts then show only downloads made by hand, which undoes the measurement half of the 2026-10-12 ruling | The answer if no |
| C5. No certificate check for listed locations, since the digest decides | Already refused in B: it voids the HTTPS-only safeguard | Refused |

Why C2. The roots stay compiled in, so a signed index can only choose among roots the image
already trusts, never add one. A listed location keeps one root, which is §196's rule. If GitHub
changes authority again (it moved `github.com` from DigiCert to Sectigo in 2024, recalled), basalt
edits the index and fetches fall back to the Pages copy until it does: counts are lost for a while,
installs never are. E46 rather than USERTrust ECC because E46 is the root of the issuing chain and
outlives the cross certificate; its own expiry was not read for this. Prior art, recalled
rather than read: crates.io's download endpoint counts and then redirects to `static.crates.io`, the
same shape, and `cargo` follows it under a full root store.

Would C2 still win at equal cost? Against C1, yes, since it is narrower for one more token. Against
C4, which is cheaper, it wins on the ruling it serves rather than on effort. Reversibility: no image
carrying any of this has shipped, so today every option is a code change; once images ship with
E46, removing it costs a new image.

If no: C4, and basalt's publish keeps listing the asset, since the copy on Pages already serves
`jig`. Blocked until ruled: `jig` counting anything. Nothing else waits.

## What this does not decide

The TUF client and verifier are milestone 858's items 4 and 5, and the trust table is milestone
666 (a signed build installs up to its key's ceiling)'s. `crates/package_index`'s stand-in
encoding waits on the TUF client, and nothing above `Index::parse` should change when it lands.
