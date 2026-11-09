# P2: after the first release

An appendix to [the milestone](../1059-block-roster-contract-outside-review.md). Every item here is
two-way and the maintainers' provisional call.

## P2-1. The publishing metadata misleads a searcher

`keywords` in `Cargo.toml` carries `shared-page`, a term nobody searches, and `categories` carries
`data-structures`, which this crate is not. Change: keywords `no-std`, `block-device`, `storage`,
`virtio` and `nvme`; categories `no-std`, `no-std::no-alloc`, `os` and `hardware-support`; and a
`documentation` field pointing at the crate's page on docs.rs. `publish = false` stays until the
publishing hold lifts. S.

## P2-2. `rust-version` asks for more than the code needs

The manifest says 1.88. The crate builds and tests on 1.85, the floor of edition 2024, with
`--ignore-rust-version`. Change: lower it to 1.85, with the README badge and the "Minimum Rust
version" section, which the standalone check compares against the manifest. Lowering is
compatible. S.

## P2-3. The package ships the repository's plumbing

`cargo package` ships 38 files, among them `.github/`, `.githooks/`, `CODEOWNERS` and `deny.toml`.
Change: an `include` list of `src/**`, `falsifications/**`, `README.md`, `CHANGELOG.md` and the two
licenses, which ships what a user reads. S.

## P2-4. The crate does not take the weekly fuzz run

The crate parses bytes from outside the program and has no `unsafe`, which is the case the
template's opt-in is for. Change: a `fuzz/` directory in the layout of `cargo-fuzz`, with one
target that feeds arbitrary bytes to `Roster::read` and walks `devices()`, and one that round-trips
an arbitrary device list through `write`; then `public-crash-inputs: true` in
`.github/workflows/fuzz.yml`. This also reaches pages longer than the 40 bytes Kani checks. M.

## P2-5. Nothing proves a page longer than 40 bytes

The README's BUGS says a longer page is not machine-checked. The in-bounds argument rests on
arithmetic, not contents, so two small harnesses cover it: one over `capacity_of` for any `len`
(that `HEADER_BYTES + capacity_of(len) * ENTRY_BYTES <= len` whenever `len >= HEADER_BYTES`), and
one that reads a 4096-byte page with a symbolic header and concrete entries and fetches every
counted entry. Each arrives with its falsification patch. M.

## P2-6. The docs do not say what a reader may trust

`ordinal` and `transport` on `Device` are unchecked `u32` values. A reader that indexes its own
table by `ordinal` must bound the index itself, and the page is a snapshot with no hot plug.
Change: one paragraph in the crate header and in the doc of `Device`. S.

## P2-7. The repository page says nothing about block devices

The GitHub description reads "block_roster: a standalone nifeos crate: proven with Kani, released
through the shared workflows", with a homepage on docs.rs under the old crate name. Both come from
the template's defaults, and the weekly settings-drift job would flag a hand edit. Change, in the
template first: a per-repository override of the metadata (the description from the manifest, the
topics `block-device`, `virtio` and `nvme` added), then one settings run here. S in the template.

## P2-8. Two local names are single letters

The review package's fourth finding: the loop binding `d` in `write`, the closure parameter `d`
in `count_by_transport` and the test bindings `r` become `device` and `roster`. Private. S.
