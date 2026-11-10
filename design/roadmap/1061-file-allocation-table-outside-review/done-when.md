# Done when

An appendix to [the milestone](../1061-file-allocation-table-outside-review.md).

- The callers pin a template revision at or past `ced625f4` (P0-2).
- The release commit has a green full `mutation` run, with each survivor killed by a
  test or exempted with its reason (P0-1).
- The first-publish token is in place at the architect's go to publish (P0-3).
- The weekly `settings drift` job is green, with `check / mutation-diff` required (P1-3).
- `grow_directory` zeroes a cluster before it links it, and a test over a device that fails after a
  chosen write count shows no reachable cluster with stale bytes (P1-1).
- The README's `BUGS` and the crate overview state the rename exception to the crash-ordering claim
  (P1-2).
- A default format chooses its cluster size by volume size, one sector stays an explicit choice,
  and the pinned digests and the README's size examples match (P1-4), or the README's `BUGS`
  refuses the item with its reason.
- `VolumeLayout::new(options, clock)` takes a `VolumeLayoutOptions` (P1-5).
- `CivilDateTime` has private fields, a checked `new` with a test for each impossible date it
  refuses, and six getters (P1-6).
- `VolumeLabel::trim_ascii_end` exists with no alias of `trimmed`, and its test pins the standard
  library's meaning (P1-7).
- Every method that takes a directory takes a `DirectoryCluster`, and
  `NamedEntry::directory_cluster` returns `None` for a file (P1-8).
- `open_file_reader` and `open_file_writer` return `OpenError<E>`, both verify the slot at open,
  and one test pins that `EntryChanged` means the same in each (P1-9).
- nife's callers of the changed items (`xtask/src/cloud_image.rs`,
  `components/src/system_installer.rs` and the two fuzz targets under `fuzz/fuzz_targets/`) build
  against the release.
- Each P1 item closed by a pull request in the crate's repository that cites this block by number.
  The release review has ratified or renamed every provisional name of P1-5 to P1-9.
- The checks the review ran still pass at the release commit: `cargo test` with no failure,
  `cargo clippy --all-targets` and `cargo doc` clean, the three bare-metal builds, and
  `cargo package` with no warning. The `proofs` run is green there.
- A second stranger run on the release candidate, from the README alone, finds no false claim, and
  `docs/stranger-test.md` records it.
- Each P2 item is an issue in the crate's repository, with this block's wording, so a later minor
  release can take them in any order.
- Nothing goes to crates.io, and nobody places a tag, without a separate go of the architect.
