# Review package: `block_roster_contract`

*Name: provisional, minted by lane lane/crate-review-packages-five. The `block_roster_contract` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to clear the crate for its move to its own repository, as step 1 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md),
the contract pilot. It describes the crate as `main` plus pull request #3366 leaves it.

## In one paragraph

The layout of one read-only page that lists the attached block devices: a magic, a count, a
capacity, then one eight-byte entry per device (an ordinal and a transport number). One party writes
it once; any program granted the mapping reads it. The crate is the one definition both sides
compile, so a reader built from another commit refuses a page it does not recognize rather than
misreading it. It is `no_std`, has no dependency and no `unsafe`, and three Kani harnesses prove
that the reader and the writer are total and that a written page reads back. The architect froze its
identity, and `write` no longer panics on any input. It is a leaf: nothing in the crate points back into nife.

## Identity

| item | value |
|---|---|
| crate | `block_roster_contract`, ratified ("a read-only page, deliberately not a `_protocol`") |
| identity values | ratified and frozen: `INTERFACE_ID` 5, `VERSION` 1, `MAGIC` the bytes `blkrosv1`; row 5 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table. The crate header and README state them, and a host test pins all three as literals |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root |
| `publish` | `false`; milestone 1017 keeps the moved contract crates unpublished under §305 (git dependencies on `nifeos/*` only, pinned to a landed commit) until their own release milestone |
| size | one `src/lib.rs` of 586 lines: about 290 of library, 190 of tests, 105 of Kani harnesses |
| package | `cargo package` packs 11 files, 45.7 KiB (14.5 KiB compressed), the three patches included, and builds from the packed copy |
| consumers in nife | the writer in `kernel` (`kernel/src/user/disk_service.rs`), the reader `disk_surveyor` in `components`, and `system_tests` (development only); all by path |

## What was run

This lane ran each check in its own worktree, on this branch.

| check | result |
|---|---|
| `script/crate-standalone block_roster_contract` | clean: manifest, files, citations, names, and the build on 1.88 for the host and the three bare-metal targets; Kani 3 of 3 prove; all 3 falsification patches fail their harness |
| `cargo test -p block_roster_contract` | 10 unit tests and 2 doctests pass; one doctest is the README's usage example |
| `cargo clippy --all-targets`, `cargo doc` with warnings denied | no warning |
| `cargo package` | passes; the one warning is the missing `repository` key, which the move adds |
| `script/names block_roster_contract` | the crate name ratified |
| `python3 helpers/name_release.py --draft block_roster_contract` | 21 names, every one ratified |
| the template's naming checks (`vendor/crate-workflows/helpers/`, `--manifest Cargo.toml` over `src/lib.rs`) | `spelled_out`, `unit_symbols`, `question_shape`, `british_spelling` and `register_names`: 80 names judged, 0 refused each. `abbreviations` (report only): 3 components not words, the baseline's 3 |
| `script/lint`, `script/test` | in the pull request's checks |

Not run, and so not claimed: the stranger test, mutation, `cargo-semver-checks` (a first release
has no baseline) and the template's workflows, which run in the repository.

## Public API

| kind | items |
|---|---|
| layout constants | `MAGIC`, `VERSION`, `INTERFACE_ID`, `HEADER_BYTES` (16), `ENTRY_BYTES` (8) |
| transport numbers | `TRANSPORT_VIRTIO_MEMORY_MAPPED_IO` (0), `TRANSPORT_VIRTIO_PCI` (1), `TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS` (2) |
| entry | `Device { ordinal: u32, transport: u32 }` and `Device::transport_name` |
| writer | `write(page, devices) -> Result<(), TooManyDevices>`, `capacity_of(len)`, the error `TooManyDevices` |
| reader | `Roster::read(page) -> Option<Roster>`, then `len`, `is_empty`, `get`, `devices`, `count_by_transport` |

No nife type, capability or handoff appears in a public signature. `cargo tree` lists no dependency.

What a stranger may find surprising:

- `transport` is a bare `u32` and not an enum, so that a reader keeps an entry whose number a newer
  writer added. `transport_name` answers `None` for such a number, and the documentation says why.
- `get` and `devices` return `Device` by value, where the standard library's `get` and `iter`
  return references. The page holds bytes, not `Device` values, so a reference is not possible
  without `unsafe`.
- The reader ignores the capacity word at bytes 12 to 16 and computes the capacity from the length
  of the slice it receives.
- `write` refuses a page shorter than the header with `TooManyDevices`, the crate's one write
  error: such a page has room for no device, not even for a count of zero. `write` leaves a refused
  page untouched. A page with room for more than `u32::MAX` entries records a capacity of `u32::MAX`.
- `TooManyDevices` implements neither `Display` nor `core::error::Error`, and neither it nor
  `Device` is `#[non_exhaustive]`. The page layout fixes `Device`'s two fields.

## Proofs and falsification patches

Three harnesses, each with one patch under `falsifications/`. Each patch applies to this branch
with no offset and no fuzz (`patch --dry-run -F0 -p1`).

| harness | what it proves |
|---|---|
| `read_is_total_and_every_counted_entry_is_gettable` | for any bytes and any length up to 40, `Roster::read` neither panics nor indexes out of bounds, and an accepted page yields exactly `len` entries |
| `write_is_total_on_any_page_length` | for any length up to 40 and any zero to three devices, `write` neither panics nor indexes out of bounds, succeeds exactly when the page holds the header and the devices, and an accepted page reads back with that count |
| `write_then_read_returns_the_same_devices_in_order` | for any zero to three devices, `write` then `read` returns them in order and no more |

The patch for the second removes `write`'s header-length guard; Kani then reports the
out-of-bounds header store on a page of 0 to 15 bytes. A host test walks every length up to two
entries past the header with zero to two devices.

Not proved: a page longer than 40 bytes, and the `u32::MAX` capacity bound, which no harness can
reach. The README's `BUGS` section says so for the first.

## Names

Every name with a block reads ratified; the move commit strips them. `Device`'s block ratifies its
fields `ordinal` and `transport`.

| name | kind | status | rule check |
|---|---|---|---|
| `block_roster_contract` | crate | ratified | passes |
| `MAGIC`, `VERSION`, `INTERFACE_ID` | constants | ratified | `id` is an accepted word |
| `HEADER_BYTES`, `ENTRY_BYTES` | constants | ratified | the size rule: the unit is spelled out |
| `TRANSPORT_VIRTIO_MEMORY_MAPPED_IO` | constant | ratified | findings 1 and 2 |
| `TRANSPORT_VIRTIO_PCI` | constant | ratified | findings 1 and 2; `pci` is an accepted word |
| `TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS` | constant | ratified | finding 1 |
| `Device`, its fields `ordinal` and `transport` | struct | ratified | passes |
| `Device::transport_name` | method | ratified | passes |
| `TooManyDevices` | error struct | ratified | passes |
| `write`, `capacity_of` | functions | ratified | passes; `capacity_of`'s parameter `len` is the standard library's word |
| `Roster` | struct | ratified | passes |
| `Roster::read` | associated function | ratified | passes |
| `Roster::len`, `is_empty`, `get` | methods | ratified | the standard library's names; `get`'s parameter is finding 3 |
| `Roster::devices`, `count_by_transport` | methods | ratified | passes |

No name holds an address, a mask or a register offset, so the address, mask and register rules of
`register_names.py` and `naming_words.toml` judge nothing here.

### Findings for the maintainer

This lane renames none of the names below. Each is a two-way door, since nothing outside nife
has acted on the crate.

| # | names | finding | recommendation |
|---|---|---|---|
| 1 | the three `TRANSPORT_` constants | ratified before the rule that keeps a source's spelling as `#[doc(alias)]`. The outside world's spellings (`virtio-mmio` and `virtio-pci`, the virtio specification's transports; `nvme`) appear only as `transport_name`'s strings. The gate passes because no doc comment opens with `(NAME, source)` | add `#[doc(alias = "virtio-mmio")]`, `"virtio-pci"` and `"nvme"`; no rename |
| 2 | `virtio` in two constants | `abbreviations.py` reports it as not a word | keep it, since it is the specification's own name; add `virtio` to the template's accepted words |
| 3 | `Roster::get`'s parameter `i` | a single-letter parameter in a public signature; the docs say "the `i`-th device" | `index`; a parameter name breaks no caller |
| 4 | private names: the harness name's `gettable`, and the locals and closure bindings `d`, `r` and `i` | `gettable` is reported as not a word; the rest are single letters | `device`, `roster` and `index` at the move; keep `gettable` or choose `fetchable` |

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair. The
transport names it prints are terms from the virtio specification and common use, not copied text.

## What still blocks the move

The crate itself does not: it is a leaf, with no dependency and no path, citation or
`include_str!` reaching outside it. What blocks the move is in nife, and in the decision below.

| item | owner |
|---|---|
| Clear the move. The clearance is the go for creating `nifeos/block_roster_contract`; nothing publishes and no tag is asked. | architect |
| Convert the three path dependencies (`kernel`, `components`, `system_tests`) to `git` and `rev` with `version = "0.1.0"`. The consumers stay in nife: the kernel's writer and `disk_surveyor`, the reader, which declares the pair of `INTERFACE_ID` and `VERSION`. | move lane |
| `helpers/interface_declarations_check.py` finds each `INTERFACE_ID` by walking the workspace members, so once the crate is a git dependency it reports "the table assigns 5 to `block_roster_contract`, which carries no `INTERFACE_ID`". Milestone 1017 says the declaration check, the Kani reach list, the verify table and milestone 610 (the interface's stability is measured weekly)'s series must read the pinned checkout instead; `helpers/interface_stability.py` names the crate. `helpers/format_values_check.py` guards a magic and version change only under `crates/`, so after the move that guard is the crate's own identity test. | move lane |
| Shipped text speaks of "the kernel" and "wiring time" (the crate header, `Device`, `write`, `TooManyDevices`, the manifest's `description`), where the README says "one party". | move lane |
| The doc comment on `MAGIC` carries a dated "Amendments" entry, and `VERSION`'s doc asks for one per break; the history belongs in the crate's changelog. | move lane |
| `helpers/template_report_baseline.txt` holds rows for the crate. | move lane |
| Run the stranger test, the template's workflows and the falsification job in the new repository, with the patches rewritten to `-p1`. | move lane |

## Open questions

1. Whether a reader should also check the capacity word against the slice length. Recommendation:
   refuse a page whose capacity word exceeds what the slice holds, because that is a writer bug the
   reader can see. It changes a refusal and no signature; the maintainer can decide it.
2. A page with room for more than `u32::MAX` entries records `u32::MAX`, and `write` does not
   refuse it. The maintainer rules this provisionally: refusing would report `TooManyDevices` for a page that
   holds every device the caller passed, and such a page is over 32 GiB.

## Recommendation

Ready for review. This branch fixes and proves the defect, and the identity holds its frozen
values. The architect's clearance is the one decision.

## Decisions for the architect

1. Clear the move to `nifeos/block_roster_contract`, with no tag and no publication. Background: the
   clearance of the review is the go for creating the repository.
