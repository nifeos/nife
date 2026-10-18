---
status: DECIDED
raised: 2026-10-17
decided: 2026-10-18
ratified_by: calef
---

# 306. The web's authority list is a package and a capability, and `jig`'s own channel never depends on it

Raised while milestone 809 (the package client becomes a program) was settling `jig add-repository`.
calef ruled that verb takes just a URL, then asked why nife does not ship the usual authority list.
He ruled on all three asks on 2026-10-18 (UTC): *"Use those four rules."*, *"Y"* and *"Yes"*.

## The question

A general HTTPS program on nife (a future `curl`, a browser at the prompt) must decide which
certificate authorities to believe for a host nobody named in advance. §196 (nife carries TLS) clause
4 holds roots per source and forbids a system-wide trust store, for the package client. That
reasoning does not extend to a program whose job is any host.

## The ruling

1. **The web's authority list is an ordinary basalt package.** Its content is Mozilla's CA list, the
   one most systems ship. It is versioned, vouched by digest under §195 (a reviewed recipe vouches
   for a package), and installed and updated by `jig` like any other package.
2. **It is a capability, not ambient.** A program gets it only if its manifest names it and its
   grant plan allows it, the same shape as the network and directory grants. `ls` never holds it. A
   program that talks to one host keeps a pinned root and does not ask for the list.
3. **It updates through `jig update` over basalt's pinned channel**, so a distrusted authority
   leaves at the next update, not the next image.
4. **`jig`'s channel to basalt never depends on anything `jig` installs.** It stays pinned to the
   image's roots (ISRG Root X1 and Sectigo E46) and never consults the list. The list depends on the
   channel and the channel never depends on the list, which removes §196's circularity instead of
   walking around it. A distrusted authority can never break updating the list.
5. **`jig add-repository` checks an added index against the list.** An added index is trusted at the
   channel exactly as a website is, by the list's authorities. §195's digests still decide what runs.
   Milestone 809's item 7 builds it once the list exists.
6. The list keeps Mozilla's partial-distrust data, and the verifier enforces it. Its source is
   Mozilla's NSS data (`certdata.txt`) or a derivative that keeps each root's server distrust-after
   date and trust level. The curl extract as it stands cannot be the source.

The list's package, the manifest field that names it and the verifier are a milestone's work.

## What is in force around it

- §196 clause 4: roots per source, no system store, for the package client.
- §250 (an image names its distribution's package index) clauses 3, 10 to 13: the channel is pinned to
  ISRG Root X1, and an image carries one further root, Sectigo E46, for GitHub Release downloads. The
  pin is compiled into the program (`PinnedPeer::basalt_index`, milestone 501 (a TLS client that
  speaks to one pinned peer)), and that client has no root store to add a second root to.
- §208 (installing a package is granting it): a package's data is a read-only directory a session
  binds by name, which is the form the list takes.
- Milestone 387 (a TLS stack, and the fork is not "OpenSSL or not") holds that a trust store should
  not be ambient: a client granted exactly the roots its one peer chains to cannot be induced to
  trust anything else.

## Alternatives refused

| | Shape | Cost | What others do |
|---|---|---|---|
| A. A frozen store in the image | The list ships in the image and is readable by programs that ask | Frozen until the next image, so a distrusted authority persists on every installed machine (DigiNotar 2011 and DST Root X3 2021 are the cases milestone 387 cites). Ambient if every program may read it | Debian (`ca-certificates`), Fuchsia and OpenBSD (`/etc/ssl/cert.pem`) ship a store, as do macOS and Windows. Debian updates it as a package; the others with the OS (recalled, not read) |
| B. Per-program pinned root only | No list; each program carries the one root for its one peer | Nothing to curate. Cannot serve a program whose host a person or a page chooses | Matches §196 and `jig` |
| C. No general HTTPS client | Nothing on nife fetches an arbitrary host | Cheapest, and forecloses every program that is not a package client, including the real workload with a real user that `AGENTS.md` principle 1 ranks first | Every system named above ships a store because it runs a browser or `curl` (recalled) |

For `add-repository`, the image's roots alone (X) were refused: an owner whose index sits on another
authority could not add it with a URL. Either set (Z) was refused: what authenticated a source would
vary by host, and the weaker set would be the real check.

The lines marked recalled were not checked against a source. Fuchsia's and OpenBSD's layouts were
not read.

## Measured, 2026-10-17 (UTC)

- Mozilla's list as curl extracts it (`curl.se/ca/cacert.pem`, last modified 2026-09-25 03:12 GMT):
  121 roots, 129,143 bytes of DER, 188,900 bytes of PEM. This corrects the "roughly 150" in milestone
  387 and the dependency census. The size is not a question for package data.
- Entrust, in the curl extract: dropped whole. The extract contains no Entrust root. In NSS's
  `certdata.txt` (current tip, fetched the same day, 172 trust objects), all four Entrust roots
  (`Entrust Root Certification Authority` G2, G4 and EC1, and `Entrust.net Premium 2048 Secure Server
  CA`) have server trust `CKT_NSS_MUST_VERIFY_TRUST` and email trust `CKT_NSS_TRUSTED_DELEGATOR`. The
  extract's contents are consistent with keeping only server-trusted roots (`GlobalSign Root CA`, also
  `MUST_VERIFY`, is absent too). A verifier fed the extract
  refuses an Entrust-issued certificate for a website, which matches NSS's server trust level.
- Where the extract loses data: four trust objects carry a server distrust-after date, and
  the extract drops that field. Two of them remain server-trusted and are in the extract without
  their date: `GlobalSign Root CA - R3` (distrust after 2026-04-15 23:59:59Z, already past) and
  `IdenTrust Public Sector Root CA 1` (2024-11-30 23:59:59Z). A verifier built on the extract accepts
  a certificate issued under either root after its date, which NSS refuses. Ruling 6 closes this.
- `certdata.txt` has no field or text that names a name constraint (a search for the phrase finds
  nothing), so the package source needs the distrust-after date and trust level, not a constraint
  field.
- The local `/etc/ssl/cert.pem` on the maintainer's Mac (OpenBSD, `$OpenBSD: cert.pem,v 1.22.2.1
  2021/09/30`) holds 128 roots in 333,483 bytes, a 2021 snapshot on different curation.

Not checked: whether `webpki-roots` or a direct `certdata.txt` build preserves the date in the form
`rustls-webpki` consumes. Nor whether `rustls-webpki` accepts 121 anchors without a cost worth
measuring (milestone 501's costs are for one root). Nor how long a Mozilla removal takes to reach a
basalt release. The owner decides whether and when a machine updates (§250's 2026-10-07 amendment), so a
machine that never updates keeps a distrusted authority.

## What it costs

- Basalt owns a package whose content is a security decision for every machine that grants it. The
  recipe is reviewed (§195 clause 1), but the review is of Mozilla's change, not of each authority.
- Two root sets coexist: the image's two, frozen until the next image, and the list. §250's BUGS
  already record the frozen set's problem: X1's cross-signatures expire 2032-09-02. This ruling does
  not change it.
- Granting the list lets a program trust any listed authority for any host. A browser is the program
  that needs the wide grant, so it is flagged like a root-zone resolver ask (milestone 892 (a std
  program at the prompt holds the network and a name resolver), "Item 3, ruled").
- Ruling 5 makes owners who add indexes by URL depend on the list. Moving back to the image's roots
  would break indexes they added.
- Not settled here: how a URL-only `add-repository` obtains the added repository's TUF root. The TLS
  check authenticates the host, not the repository's signing root. That is milestone 809's open
  question 1.

## BUGS

- The measurements are one day's, taken from Mozilla's tip and a third-party extract. The figures move
  with every Mozilla release.
- No code path in the tree uses a root list, so none of this has been run.
