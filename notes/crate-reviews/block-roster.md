# Review package: `block_roster`

*Name: provisional, minted by lane lane/crate-review-packages-five. The `block_roster` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the five packages together
outgrow the page.*

## In one paragraph

The layout of one read-only page that lists the attached block devices: a magic, a count, a
capacity, then one eight-byte entry per device (an ordinal and a transport number). One party writes
it once; any program granted the mapping reads it. The crate is the one definition both sides
compile, so a reader built from another commit refuses a page it does not recognize rather than
misreading it. It is `no_std`, has no dependency and no `unsafe`, and two Kani harnesses prove the
reader. It leaves as a shared crate, because the kernel and the programs that read the page will
live in different repositories, and milestone 1017 (the contract crates leave in this order) makes
it the first contract crate to leave. What stands between it and a review is the ratification of
its identity value and one panic in `write`.

## Identity

| item | value |
|---|---|
| crate | `block_roster`, ratified ("a read-only page, deliberately not a `_protocol`") |
| version | `0.1.0`; no tag in this clone |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root |
| `publish` | `false`; milestone 1017 (the contract crates leave in this order) keeps the moved contract crates unpublished under §305 (git dependencies on `nifeos/*` only, pinned to a landed commit) until their own release milestone |
| size | one `src/lib.rs` of 492 lines: about 270 of library, 155 of tests, 65 of Kani harnesses |
| identity values | `MAGIC` (the bytes `blkrosv1`), `VERSION` 1, `INTERFACE_ID` 5 |
| consumers in nife | `kernel`, `components` and `system_tests` (development only), by path |

## Public API

| kind | items |
|---|---|
| layout constants | `MAGIC`, `VERSION`, `INTERFACE_ID`, `HEADER_BYTES` (16), `ENTRY_BYTES` (8) |
| transport numbers | `TRANSPORT_VIRTIO_MEMORY_MAPPED_IO` (0), `TRANSPORT_VIRTIO_PCI` (1), `TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS` (2) |
| entry | `Device { ordinal: u32, transport: u32 }` and `Device::transport_name` |
| writer | `write(page, devices) -> Result<(), TooManyDevices>`, `capacity_of(len)`, the error `TooManyDevices` |
| reader | `Roster::read(page) -> Option<Roster>`, then `len`, `is_empty`, `get`, `devices`, `count_by_transport` |

What a stranger may find surprising:

- `transport` is a bare `u32` and not an enum, so that a reader keeps an entry whose number a newer
  writer added. `transport_name` answers `None` for such a number. This is a deliberate choice for
  a wire value, and it reads well once the documentation says so, which it does.
- `get` and `devices` return `Device` by value, where the standard library's `get` and `iter`
  return references. The page holds bytes, not `Device` values, so a reference is not possible without
  `unsafe`. `get` keeps the standard library's spelling with a meaning one step away.
- The reader ignores the capacity word at bytes 12 to 16. It computes the capacity from the
  length of the slice it is given. A page whose capacity word disagrees with its length still
  reads.
- `TooManyDevices` implements neither `Display` nor `core::error::Error`, and `Device` has public fields, so
  a field added later breaks callers. The page layout fixes both fields, so that is unlikely.

## Proofs and falsification patches

Two harnesses, each with one patch. `script/crate-standalone` proved both and saw each patch fail
its own harness.

| harness | what it proves |
|---|---|
| `read_is_total_and_every_counted_entry_is_gettable` | for any bytes and any length up to 40, `Roster::read` neither panics nor indexes out of bounds, and an accepted page yields exactly `len` entries |
| `write_then_read_returns_the_same_devices_in_order` | for any zero to three devices, `write` then `read` returns them in order and no more |

Not proved: `write` on a page shorter than the header. That is where the defect below lives.

## Names

All 17 public names are ratified. `VERSION` and `INTERFACE_ID` are the names every contract crate
uses, by the 2026-11-01 ruling on `counter_frequency_contract`. The fields `ordinal` and `transport`
have no block; the draft tool does not list fields.

| name | kind | reading |
|---|---|---|
| `MAGIC`, `VERSION`, `INTERFACE_ID` | constants | the same names every contract crate uses |
| `HEADER_BYTES`, `ENTRY_BYTES` | constants | the unit is spelled out |
| `Device` | struct | read as `block_roster::Device` it says what it is; its fields are `ordinal` and `transport` |
| `Roster` | struct | the crate's own noun |
| `write` | function | writes a roster into a page |
| `read` | associated function | the pair of `write` |
| `capacity_of` | function | answers how many entries a page length holds |
| `len`, `is_empty`, `get` | methods | the standard library's names; `get` returns by value as noted above |
| `devices` | method | yields each `Device` in the page |
| `count_by_transport` | method | counts the devices on one transport |
| `TooManyDevices` | error struct | names the refusal |
| `transport_name` | method | returns the transport's printed name |

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair.
The transport names it prints (`virtio-mmio`, `virtio-pci`, `nvme`) are terms from the virtio
specification and common use, not copied text.

## What the move still needs

| item | owner |
|---|---|
| Ratify the identity value `INTERFACE_ID = 5`. Milestone 1017 asks for the id values to be ratified before the first contract crate leaves, and this crate is that pilot. | architect |
| The doc comment on `MAGIC` carries a dated "Amendments" entry; the version history belongs in the crate's changelog or commit messages, not the item. | move lane |
| Shipped text speaks of "the kernel" and "wiring time" as if the reader knew nife; the README already says "one party", and `src/lib.rs` should say the same. | move lane |

## Proposed items

1. `write` panics on a page shorter than `HEADER_BYTES` when the device list is empty. The capacity
   check passes (zero devices fit in zero capacity), and the header stores then slice out of
   bounds. A scratch program saw the panic at lengths 8, 12 and 15. Proposed fix: return `Err(TooManyDevices)`
   when `page.len() < HEADER_BYTES`, and a harness that `write` is total on any length, with its
   falsification patch. The kernel always passes a whole page, so nothing in nife hits it today.
2. `write` stores the capacity with `as u32`, which truncates on a slice of more than 2^32 entries.
   No page is that large; a `u32::try_from` refusal would make the bound explicit.

## Open questions

1. Whether a reader should also check the capacity word against the slice length. Recommendation:
   refuse a page whose capacity word exceeds what the slice holds, because that is a writer bug the
   reader can see. This changes a refusal and no signature, so it can come in a later release; the
   maintainer can decide it.

## Result of `script/crate-standalone`

Run on the crate as it stood on `main`, the `names` step failed and the rest passed: the build on
1.88 for the host and the three bare-metal targets, both harnesses and both patches. The crate with
its ratified names applied (#3282) has not been run through `script/crate-standalone`.

## Recommendation

Blocked by the identity value `INTERFACE_ID = 5` and proposed item 1. After both, ready for review.
