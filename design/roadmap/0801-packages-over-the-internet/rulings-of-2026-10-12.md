# Milestone 801's rulings of 2026-10-12: where the index is served, and how a Release is reached

An appendix to [milestone 801 (packages over the internet)](../0801-packages-over-the-internet.md),
which its prose budget has no room for. Both rulings are in force.

## Ruled 2026-10-12 (UTC): where `basalt.nifeos.org` is served

calef was asked whether the host should be a VPS running Caddy, GitHub Pages, Cloudflare, or S3 and
CloudFront. He answered *"b"*, GitHub Pages. Asked how adoption would then be measured, he answered
*"Packages on Releases, index on Pages"*. Later the same day: basalt must not become the
organization's Pages site, since `www.nifeos.org` will be its own site from
`nifeos/nifeos.github.io`; basalt is a project site of `nifeos/basalt` at its own custom domain.

- The index (`/rolling/metadata/`) is served by GitHub Pages at `basalt.nifeos.org`, a CNAME to
  `nifeos.github.io`, whose certificate chains to ISRG Root X1 (measured 2026-10-13), so §250's pin
  holds unchanged.
- Package bytes are GitHub Release assets, one Release per publish, each listed in its index line
  as a location. §250 clause 2 already lets bytes live anywhere. An asset's `download_count` is the
  first adoption signal (milestone 576).
- basalt's `publish` job (nifeos/basalt#13) builds this. It also keeps clause 9's copy under
  `targets/` on Pages, the fallback when the Release cannot be reached.
- The image's production sources exist: `package_index::BASALT_SOURCES`, packed when an image is
  built with `NIFE_INDEX_SOURCES=basalt`. Every check
  still boots the test peer.

## Ruled 2026-10-12 (UTC): a listed location names its root, and one redirect is followed

Question C, asked on #2048 with the options in `notes/packages/the-index-format.md`. calef answered
*"a"*, the recommended C2, and §250's amendment of that date records it as clauses 11 to 13.

- The signed index may name, per listed location, the root its certificate must chain to, among the
  roots the image carries: `<root>=https://...` in the stand-in encoding. A location naming none
  chains to the index's root. The image carries further roots on `location-root` lines in its
  sources; basalt's names `sectigo-root-e46`, the root `github.com` chains to.
- A fetch from a listed location follows exactly one redirect. It must be an absolute HTTPS URL,
  its host is held to Q1's public-address check and to the index's root, and a second is refused.
- The digest check is unchanged.

Built 2026-10-13 (UTC). `pinned_tls_client::listed` is
the fetch, which `jig` and `package_fetch_exerciser` both call. `package_index` reads the root
token, `Sources::root_for` chooses it, and `pinned_tls_client::SECTIGO_ROOT_E46` is the root,
checked byte for byte against Sectigo's and macOS's copies. An ignored host test fetched ripgrep's
Release asset through it: `github.com` under E46, a `302` with a 931-byte path, then the bytes from
an X1 host. `a_package_is_fetched_through_the_index_by_name_over_tls_and_judged_by_its_digest` runs
six redirect probes on all three architectures. One good redirect is fetched and admitted. Refused:
an unnamed root, a root the image does not carry, a second redirect, a private address and a host
off the index's root. Falsified on aarch64 twice: a redirect pinned to the
location's root fetched from the off-root host, and two redirects allowed fetched through both.
