# Security audit, 2026-10-19: the supply chain, read from the package inward

**Kind:** security. **Lens:** the supply chain, which no audit has taken: the nine external
packages the window added and the gate that should see every graph they enter, then the path by
which bytes this tree did not build become authority on the target (the image's catalog, the index
copy `jig update` writes, and what a vouch endows). **Findings:** fixed 3, minted 0, accepted 2.

The finding to carry off is a gate that read a typed list. `script/supply-chain` ran cargo-deny over
nine workspaces by name and never over `jig/`, the package manager and the one shipping program that
fetches from the network, or over `c_library/`, which failed it. The gate now discovers every
committed lockfile and refuses one it cannot read. On the target, a vouch is decided by whoever
writes the index copy, which is `jig`, and on `main` a vouch still endows a native package with the
network its note asks for. Milestone 892 (a std program at the prompt holds the network and a name resolver) carries the refusal. Nothing here moves the verdict on fatal
risk 7.

## Why this lens

`script/audits` on `main` at `180566bef` reported security DUE on every count trigger against the
2026-10-03 baseline: milestones built 288 to 454 (+166, fires at 15), components 197 to 234 (+37,
fires at 8), ABI constants 76 to 97 (+21, any change fires) and external packages 181 to 190 (+9,
any change fires). Issue #2258 is the scheduled workflow saying so.

The lens rotation in [the index](README.md) has taken the kernel, the assembly, the shared pages,
counterparty input, newly minted authority, userspace confinement, trust boundaries and the system call
surface twice. Supply chain has been on the "not yet taken" list since the README was written. The
external-packages trigger fired for the first time since 2026-09-29, and the judgment trigger below
is a package manager taking the network, so the two point at the same place.

### The delta, counted the way `script/audits` counts

Distinct registry packages across every committed lockfile but `vendor/`, diffed between
`996bb792a` (the commit that landed the 2026-10-03 report) and `180566bef`. Nine added, none removed:

| Package | Lockfile | Enters through | Ruled |
|---|---|---|---|
| `httparse` | root, `jig`, `fuzz`, `pinned_tls_client`, `pinned_tls_exerciser` | `crates/http_response`, the head reader `jig` runs on a package source's reply | calef, 2026-10-18 (UTC), milestone 784 (reuse what the tree wrote) item (a) |
| `rayon`, `rayon-core` | `std_exerciser` | its first third-party dependency (milestone 812 (`std::thread::spawn` runs real threads in one address space)) | #1892; `deny.toml` bans it everywhere else |
| `crossbeam-deque`, `crossbeam-epoch`, `crossbeam-utils`, `either` | `std_exerciser` | `rayon`'s own graph | with `rayon` |
| `ruzstd`, `tar-no-std` | `notes/packages/container-format-measurements/fuzz` | a harness measuring two candidate package containers | no ruling needed: nothing builds or ships it (finding 3) |

### The two uncountable triggers, answered

Has a new component taken device or network authority? **Yes, twice.** `jig` (milestone 809 (the package client becomes a program), built
2026-10-10 UTC) holds the network stack, a resolver of the root zone, the clock and entropy for TLS,
and the installer endpoint, and it parses a package source's HTTP reply and the index's text before
any digest is checked. Milestone 892, in flight on #2405, would grant installed programs the network
and resolver zones at install. Both are this lens. The second, device authority, is
`designware_mobile_storage`, radon's SD/MMC block server (milestone 53 (the board's own peripherals: network and storage on real silicon), 2026-10-06), which holds a
register page and a transfer region and no DMA; it is outside this lens and is the next audit's
(below).

Has this booted on a new machine class? **No.** radon gained a peripheral, not a class, and argon is
a Jetson TK1 that does not boot the aarch64 kernel yet.

## What was read, and what came back clean

**The new packages, at the boundary they cross.** `http_response` hands the head to
`httparse::Response::parse` with default features off, so `no_std` and no dependencies. The head is
capped at `HEAD_MAX` (2,048 bytes) and `MAX_HEADERS` (128), `Content-Length` is required, must agree
with itself, and is parsed with `checked_mul` and `checked_add`, and a body longer than declared is
refused. In `jig`'s `fetch`, the declared length sizes the staging run through
`spawnproto::image_pages`, a `div_ceil` that cannot overflow, bounded by `IMAGE_MAX_PAGES` (1,024
pages, 4 MiB) before a page is retyped, and each body slice is copied through `get_mut`, so a source
that lies about its length is refused rather than written past. `rayon` and its four are banned by
`deny.toml` everywhere but `std_exerciser`.

**`deny.toml`'s one ignored advisory.** RUSTSEC-2023-0071 (the Marvin timing attack on RSA
decryption) is ignored on the condition that nothing here decrypts or signs with `rsa`. `jig` now
links `rsa` through `pinned_tls_client`. A grep of every `.rs` outside `vendor/` finds `rsa` only in
`cryptography_provider/src/verify.rs`, verifying, and the provider's `KeyProvider` refuses to load any
private key. The condition holds.

**CI's own supply chain.** Every `uses:` in `.github/workflows/` is pinned to a 40-hex commit SHA,
and `script/lint` refuses one that is not.

**`vendor/`.** `script/vendor-verify` reproduced redoxfs 0.9.1 from the published tarball plus the
divergence patch, 45 files. `vendor/relibc` is a seeded fork at `893a3b9133ac`, recorded as not a pin
and not covered by `vendor-verify` or `vendor-watch`, with the reason, in `vendor/README.md`.

**The install path's decision on bytes.** `package_archive::installable_as` and
`package_index::accept` are one rule on two sources: a package is admitted only if it is the one asked
for and its whole-file digest is the catalog's or the index entry's. `jig` checks before it sends and
the progenitor checks its own copy again (`system_initializer::activation::indexed`), and an index
copy that does not parse whole, or names a stem twice with two answers, vouches for nothing. A note
cannot ask for the installer, the catalog, the index store, the activation directory, the reboot
object or `sync` (`grant_plan::image_can_carry`), so no package can be endowed with the means to
vouch.

## Findings

### 1. FIXED: the supply-chain gate never read `jig` or `c_library`

`script/supply-chain` named nine manifests in a `for` line. `jig/` and `c_library/` each landed on
2026-10-10 with a lockfile of their own and were never added, so for nine days nothing checked the
package manager's graph (`rustls`, the RustCrypto primitives, `rsa`, `sha2`, `httparse`) for an
advisory, a yank, a license or a source. Run by hand, `jig` was clean. `c_library` failed: its two
path dependencies are wildcards to cargo-deny, because the crate was not marked `publish = false`.

Fixed in `5e5f10204`. The manifests are discovered from `git ls-files '*Cargo.lock'`, the set
`script/audits` already counts, so the trigger and the gate read one set. `vendor/` and the
measurement harness are skipped with their reasons in the script's header, and any other lockfile
with no `Cargo.toml` beside it fails the run (falsified with an intent-to-add probe). `c_library` is
`publish = false`. All eleven graphs pass.

### 2. ACCEPTED: on `main`, the index's host decides which native packages reach the network

`grant_plan::image_manifest` endows vouched bytes with everything their note declares, and for a
native program that includes `network`. Since milestone 801 (packages over the internet), the index copy `jig update` writes is a
vouch the progenitor accepts (ruling I2, 2026-10-06 UTC). The index is fetched over TLS pinned to
ISRG Root X1, a public authority, and carries no signature until milestone 858 (lab machines update themselves through packages)'s TUF client. So the
chain from "the owner installed a package by name" to "that package connects out" runs through the
index host's certificate and `jig`'s integrity, and the owner is shown nothing about the network at
install.

Accepted because the fix is ruled and built: milestone 892's refusal half
(`ImageRefusal::NetworkNotGranted`, calef's ruling "a" on #2011) refuses the network to vouched bytes
until the owner's grant is recorded. It is on #2405, which is waiting on the architect. The
`BUGS` entries in [notes/packages.md](../../notes/packages.md) and
[notes/packages/jig.md](../../notes/packages/jig.md) now name the index copy as a vouch and say who
writes it.

### 3. ACCEPTED: two counted packages sit in a graph no gate reads

`ruzstd` and `tar-no-std` are in the external-packages count through a lockfile whose manifest is
`Cargo.toml.in`, so neither cargo nor cargo-deny can read it. Accepted: the harness measures two
container formats for [container-format-measurements.md](../../notes/packages/container-format-measurements.md)
and nothing builds, links or ships it. If either is chosen as nife's container, it enters a real
graph by a §46 (thin primitives or whole subsystems) ruling, and the discovery in finding 1 gates it
from that commit. The reason is recorded at the skip in `script/supply-chain`.

### 4. FIXED: `jig`'s own page did not say it is inside the trusted base

[Milestone 809's block](../roadmap/0809-the-package-client-becomes-a-program.md) records it: until a
signature over the index (I3), "`jig` is inside the trusted base for installs". The progenitor
installs whatever the index copy vouches for, and `jig` alone writes the copy, from network input it
parses first. [notes/packages/jig.md](../../notes/packages/jig.md), the page a reader of the feature
meets, said nothing of it. Its `BUGS` now does. The same entry says where milestone 892's owner's
yes sits: if `jig` collects it, the trusted base covers network grants too. That widens little,
since a compromised `jig` already holds the network, but it is the line 892's confirmation has to
draw.

### 5. FIXED: `SECURITY.md` had no install path in scope, and said there was no network security

`SECURITY.md`'s scope named the repository's supply chain and not the target's. A way to get bytes
installed as vouched, or endowed past their note and the owner's grants, is now in scope. "No network
security of any kind" was false since milestone 501 (a TLS client that speaks to one pinned peer)'s pinned TLS client; it now says what there is.
The audit count is ten.

## Is any confinement claim false as stated?

No. Finding 2 is the documented rule (a vouch grants the note) meeting a vouch source that arrived
later; the `BUGS` entry stated the rule and now states the source.

## Fatal-risk implications

None minted, so nothing for `design/fatal-risks/` to cite. Finding 2 bears on risk 7 (confinement)
only through who may vouch, and it closes with milestone 892.

## What was deliberately not examined

- The rest of the window: twenty-one ABI constants and thirty-seven components (`jig` is a
  workspace of its own and not among them), unread by any security audit. The next audit should take that delta as its lens, with
  `designware_mobile_storage`'s device authority first.
- The compiler. The nightly toolchain from rustup, `patches/std-nife` over `rust-src` under
  `-Zbuild-std`, and `toolchain-bump.yml`, which moves the pin unattended.
- The TLS stack's internals (`rustls`, `rustls-webpki`, the RustCrypto primitives) beyond the
  advisory gate and the Marvin condition.
- relibc's generic code, which no upstream watch covers by design.
- basalt, which pins nife and builds the image. It is another repository.

## Method, so the negatives can be judged

One reader, 2026-10-19 (UTC), on `main` at `180566bef`. The package delta is the lockfile diff
above. cargo-deny 0.20.2 (the pinned version) was run by hand on every committed manifest before and
after the fix. The install path was read in `jig/src/main.rs`, `crates/http_response`,
`crates/package_archive`, `crates/package_index`, `crates/grant_plan/src/image.rs` and
`crates/system_initializer/src/activation.rs`, and against milestone 892's branch read-only for
finding 2's fix.

## What wants a lane of its own

The security audit of the window's ABI and component delta, named above. Nothing else.
