# Stage 6: allocation and write

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what stage 6 built, the evidence, and the one design choice an architect may want to
revisit before the crate publishes. Stage 6 lands in two pull requests; this page describes the
first, which changes the table and nothing above it.

## What the first pull request built

- `WritableSectorDevice`, the write half of the sector interface. It extends `SectorDevice` with
  `write_sector(index, buffer)`, where `buffer` is always one sector. A device that cannot write
  does not implement it, and every reader keeps working over `SectorDevice` alone. A blanket
  implementation covers `&mut D`.
- `TableEntry::encode`, the inverse of `decode`, and the table's `store`, which puts a value into
  an entry's bytes. A FAT12 entry keeps its neighbour's nibble and a FAT32 entry keeps its reserved
  top four bits.
- Table writes. `Volume` stores an entry in every live copy of the table (all of them, or the one
  `BPB_ExtFlags` names on FAT32), one sector read and written at a time. A FAT12 entry that
  straddles two sectors writes the later sector first.
- Allocation and release over `LinksMut`, the table's read and write, so the proofs run them over
  a model table:
  - `Volume::allocate_cluster(extending)` takes a free cluster, marks it end-of-chain, and links it
    after `extending` when the caller names the last cluster of a chain. A failed link gives the
    cluster back. The search starts at the cluster after the last one handed out, or at the `FSInfo`
    hint on the first call, wraps once, and takes only an entry that is free.
  - `Volume::free_chain(first)` frees a chain in order and returns the count. It stops at the first
    entry that is not a link or an end marker without touching it, so a bad or reserved cluster
    keeps its marker.
  - `Volume::truncate_chain(first, keep)` cuts a chain to `keep` clusters. It marks the new last
    cluster end-of-chain before it frees the tail, so a crash leaks the tail and never leaves a
    chain pointing at a free cluster.
- `FsInfo` and `FsInfoError`. The parse checks the three signatures and reports a free count or a
  next-free cluster only when it could be true of the volume. `Volume::fs_info` reads the hints,
  `Volume::sync` writes them back after changes (touching only the two hints, and never a sector
  whose signatures are wrong), and `Volume::count_free_clusters` counts them from the table.

No nife type appears in a public signature.

## The choice an architect may revisit

The write half could have been a `write_sector` method on `SectorDevice`, or a second trait. This
stage takes the second. It keeps the read-only paths (milestone 140 (mount a drive this system did not create), whose read-only stratum, the
fuzz targets, the macOS image tests) from needing a method they cannot honestly implement, and a
read-only device never has to return an error for a call that should not exist.

Prior art, from memory and not re-read for this page: rust-fatfs takes one `Read + Write + Seek`
bound for every operation, so a read-only volume needs a wrapper that fails writes; embedded-sdmmc
puts `read` and `write` on one block-device trait. The cost of a second trait is one more name in
the public API. The choice is reversible until the first publication, the name is provisional, and
nothing outside the crate uses it yet.

## The evidence

| check | result |
|---|---|
| Kani, seven new harnesses | `a_stored_entry_reads_back_and_disturbs_nothing_else` and `encoding_inverts_decoding` cover every type, cluster, byte window and value. `allocation_takes_one_free_cluster_and_nothing_else`, `a_release_frees_only_its_own_chain` and `a_cut_keeps_exactly_the_head_of_the_chain` run over a model table of one to four clusters with every entry chosen by the solver. `every_sector_parses_to_hints_inside_the_volume` and `a_store_changes_only_the_two_hints` cover every 512-byte `FSInfo` sector |
| falsification | each harness has a patch that turns it red at its own assertion: a FAT12 store that keeps the wrong bits, a bad entry encoded one past its marker, a parse that believes any free count, a next-free hint stored four bytes late, a search that treats a reserved entry as free, a release that frees before it looks, and a cut that walks one cluster too far |
| unit tests | allocation from a hint, a wrap, a full volume, bad and reserved entries never taken, every entry of a 350-cluster FAT12 table stored without disturbing a neighbour, an entry straddling two table sectors, a chain freed and cut at every length, a broken chain, a looping chain, a refused write that leaves nothing leaked, and the `FSInfo` signatures and edges |
| oracle tests | on FAT12, FAT16 (512 and 4096-byte sectors) and FAT32 volumes rust-fatfs formatted, this crate allocates a chain of 40 clusters and one of 7, cuts the first to 12, and frees both. After each step rust-fatfs counts the same free clusters, on FAT32 the `FSInfo` count equals the true one, and at the end the boot area and both tables are byte for byte what they were. `fsck.fat` accepts the empty volume and the freed volume. A scrambled run of 400 grow, cut and free steps keeps the count and every chain's length against a model |

Kani bounds what it unrolls. The allocation harnesses cover four clusters at most. The operations'
own loops are bounded by the cluster count, and the oracle tests run them over volumes of tens of
thousands of clusters.

## Names minted

Every name in this stage is provisional, with a `Name: provisional` block in its source.

- `WritableSectorDevice` and its method `write_sector`.
- `TableEntry::encode`.
- `AllocationError` (`Chain`, `VolumeFull`, `NotTheEndOfAChain`).
- `Volume::allocate_cluster`, `free_chain`, `truncate_chain`, `count_free_clusters`, `fs_info`
  and `sync`.
- `FsInfo` (`parse`, `free_clusters`, `next_free`) and `FsInfoError` (`TooShort`, `BadSignature`).
- The modules `allocation` and `fs_info`.
