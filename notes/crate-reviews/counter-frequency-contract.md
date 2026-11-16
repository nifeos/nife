# Review package: `counter_frequency_contract`

*Name: provisional, minted by lane lane/crate-review-packages-five. The `counter_frequency_contract`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the five packages
together outgrow the page.*

The architect reads this to clear the crate for its move to its own repository, in step 2 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as `main` leaves it. The outside review behind it is
[milestone 1073](../../design/roadmap/1073-counter-frequency-contract-outside-review.md), and every
item of that review has landed or is settled.

## In one paragraph

The layout of a 16-byte read-only page that tells a process how many counter ticks make a second,
on the architectures with no register that says so: the magic `TIMEBAS1`, then the rate as one
little-endian `u64`. The crate also holds the page's fixed virtual address per architecture and the
band of rates worth believing (100 kilohertz to 100 gigahertz), which the kernel's three boot paths
share. It is `no_std` with no dependency of any kind, normal, build or development. Unlike the
other contract crates, it has `unsafe`, because the reader names a raw address. Two Kani harnesses
prove the reader's magic compare and the round trip, each with a falsification patch. Every name is
ratified, and each unit is spelled out. No file in the crate names nife, a path in this tree or a
private machine.

## Identity

| item | value |
|---|---|
| crate | `counter_frequency_contract`, ratified on #3059 |
| version | `0.1.0`; no tag and no repository |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root |
| `publish` | `false`; milestone 1017 keeps moved contract crates unpublished until their own release milestone, and the manifest comment says the line stays until the release that publishes |
| size | one `src/lib.rs` of 383 lines, a README, two license texts and two falsification patches |
| identity values | frozen: `MAGIC` (`TIMEBAS1`), `VERSION` 1, `INTERFACE_ID` 9, `PAGE_CONTENT_BYTES` 16 (the magic at offset 0, the little-endian rate at offset 8) and `PAGE_VIRTUAL_ADDRESS` `0x0000_7000_0000_0000` on `x86_64` and `0x0000_0038_0000_0000` on `riscv64`. A layout break ships as a new magic and version together. The crate header and README state them, and a host test pins them as literals. Frozen 2026-11-11 (UTC) on #3516 |
| consumers in nife | `kernel`, `user_mode_runtime`, `process_builder`, `components` and `system_tests` (development only), by path |
| copied by `xtask` | `cargo xtask std-src` copies `src/lib.rs` into the patched standard library as `sys/pal/nife/counterfreqproto.rs`. It finds the file through `cargo metadata` (`xtask/src/farm/copied.rs`), so a pinned crate is read from Cargo's checkout and the move changes nothing there |

## Public API

| kind | items |
|---|---|
| layout | `MAGIC`, `VERSION`, `INTERFACE_ID`, `PAGE_CONTENT_BYTES` (16), `PAGE_VIRTUAL_ADDRESS` (`0x7000_0000_0000`, or `0x38_0000_0000` on `riscv64`) |
| rate band | `MINIMUM_PLAUSIBLE_HERTZ`, `MAXIMUM_PLAUSIBLE_HERTZ`, `is_plausible(hertz)` |
| writer | `build_page(hertz) -> [u8; 16]` |
| reader | `CounterFrequencyPage`, `unsafe fn new(virtual_address: u64)`, `hertz() -> Option<u64>` |

The two offset constants, `MAGIC_OFFSET_BYTES` and `HERTZ_OFFSET_BYTES`, are private and follow the
family rule for page fields (ruled 2026-11-14 UTC).

What a stranger may find surprising:

- `CounterFrequencyPage::new` takes an address as a `u64` and is `unsafe`; the caller promises 16
  mapped, unchanging, byte-aligned bytes for the value's whole life. The type is `Copy`, `Send` and
  `Sync` by `unsafe impl`, on the argument that the page has one writer before any reader exists.
  A `const` assertion refuses a target whose pointers cannot hold `PAGE_VIRTUAL_ADDRESS`.
- `hertz()` answers `Some` for any page with the magic, so a page built with a rate of 0 reads as
  `Some(0)`. The crate documents a zeroed frame as "unknown", and the reader applies
  `is_plausible` itself; `user_mode_runtime::counter_hertz_checked` does, on both architectures.
- `PAGE_VIRTUAL_ADDRESS` exists on aarch64 too, with the `x86_64` value, although aarch64 maps no
  page and reads `CNTFRQ_EL0` instead.
- `Debug` prints the address, not the rate.

## Proofs, falsification patches and tests

| harness | what it proves | falsification patch | verifies in |
|---|---|---|---|
| `hertz_answers_none_exactly_when_the_magic_differs` | over every 16-byte frame, `hertz` is `None` exactly when the first eight bytes differ from `MAGIC`, and otherwise the little-endian `u64` of the next eight | turns it red at its own assertion | 0.76 seconds |
| `a_built_page_reads_back_its_rate` | `build_page(x).hertz()` is `Some(x)` for every `u64` | turns it red at its own assertion | 0.58 seconds |

Kani 0.68.0 with CBMC 6.11.0 verifies both over the staged standalone copy in 8 seconds of wall
time at a peak of 288 MiB. The template head's `falsify.py` replays both patches, and each turns
its harness red. The `unsafe` surface is one `from_raw_parts` behind `new`'s contract, and no proof
covers it, since Kani cannot see a caller's promise about a mapping. The harnesses assume nothing.

Seven host tests cover the round trip, a zeroed frame, a wrong magic, the band's edges, a rate
outside the band reading back as written, the layout offsets and the frozen identity values. Four
doctests run, the README's example among them. The end-to-end test, which compares the number a
user process reads with the one the kernel measured, is `system_tests`'s and stays in nife, and the
README says so.

The mutation baseline row in `.cargo/mutants-baseline.txt` reads 16 caught, 0 missed, 0 timed out
and 7 unviable. That census predates both harnesses and the out-of-band test, so it undercounts the
tests that now catch mutants. A fresh run belongs in the 08:00 to 13:00 UTC window, over the dry
run's staged copy:
`systemd-run --user --scope -p MemoryMax=12G cargo mutants --manifest-path <staged>/Cargo.toml -j 2`.
The move carries the row into the repository's `.github/mutants-exempts` as a comment, and the
repository's first weekly run lists survivors.

## Names

#3059 ratified all 13 public names on 2026-11-01. Names spell each unit out (`_HERTZ`) and use no
SI symbol, because uppercase erases the case that tells `MHz` from `mHz`. The template head's seven
naming checks judge 51 names and refuse none, and its abbreviations report finds no component that
is not a word.

| name | kind |
|---|---|
| `MINIMUM_PLAUSIBLE_HERTZ` | constant |
| `MAXIMUM_PLAUSIBLE_HERTZ` | constant |
| `CounterFrequencyPage::hertz` | method |
| `hertz` | parameter of `is_plausible` and `build_page` |

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair.
`CNTFRQ_EL0` and `CPUID.80000007H:EDX[8]` are register names cited as facts.

## The template's gates over the staged copy

`script/crate-move --dry-run` stages the crate with the pinned template `v7` and runs its checks.
Over the crate's own files it reports nothing: the copy, the two patches applying exactly, the
build on 1.88 for the host and the three bare-metal targets, the standalone check, the prose caps,
the file and comment ratchets, orphan modules, markdown links and the falsification records all
pass.

The template head runs more than `v7`. Over the same staged copy, every check of the head's
standalone job passes: the vocabulary, passive voice (none in 578 README words), history phrases,
acronyms, banned figures, the function-length ratchet, the seven naming checks, the service-model
prose check, the README badge row, acronym order, inflated wording and section order. Clippy with
warnings denied passes on stable and on 1.88, for the host with every target and for each
bare-metal library, and so does the `--cfg kani` clippy pass. `cargo doc` with warnings denied
passes.

The dry run reports three findings, all in the `v7` scaffold and none in the crate: an architect's
name in `.github/pull_request_template.md`, a path into this tree in `CONTRIBUTING.md`, and five
lines that name nife (one each in `.githooks/commit-msg`, `deny.toml` and
`docs/stranger-test.md`, two in `rustfmt.toml`). The template head's scaffold names nife
nowhere, and it adds `_typos.toml`, which `v7` lacks; the head's spelling check passes over the
crate's own files.

## Settled with the maintainer

Both are two-way doors while no caller outside nife exists, and each stands until a later release
changes it.

1. `hertz()` answers `Some` for a rate outside the band, and the reader applies `is_plausible`.
   The stricter behavior, `None` outside the band, puts the band in one place, which is the reason
   `is_plausible` exists; it would change one host test, the first harness and its patch. A later
   release can take it, and the maintainer decides it.
2. `PAGE_VIRTUAL_ADDRESS` exists on aarch64, because the host build takes the same arm and tests
   need a value. Removing it later breaks an aarch64 caller, and none exists.

## What the move still needs

Nothing in the crate. The crate has no dependency, so it leaves in step 2 of milestone 1017's
order, ahead of `address_space_layout`. The move lane stages the crate at the template release
current on the day it moves, then makes the nife-side pull request the dry run lists: the five
manifests switch to the tagged commit, and the mutation baseline row leaves.

## Recommendation

Ready for the architect's review. The identity does not change. The crate stays in this tree, and
no repository appears, until the architect clears this review.
