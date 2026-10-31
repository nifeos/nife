# Stage 6: allocation and write

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what stage 6 built, the evidence, and the one design choice an architect may want to
revisit before the crate publishes. Stage 6 landed in two pull requests: the first changed the table and
nothing above it, and the second wrote files.

## The first pull request: the table

- `WritableSectorDevice`, the write half of the sector interface. It extends `SectorDevice` with
  `write_sector(index, buffer)`, where `buffer` is always one sector. A device that cannot write
  does not implement it, and every reader keeps working over `SectorDevice` alone. A blanket
  implementation covers `&mut D`.
- `TableEntry::encode`, the inverse of `decode`, and the table's `store`, which puts a value into
  an entry's bytes. A FAT12 entry keeps its neighbor's nibble and a FAT32 entry keeps its reserved
  top four bits.
- Table writes. `Volume` stores an entry in every live copy of the table (all of them, or the one
  `BPB_ExtFlags` names on FAT32), one sector read and written at a time. A FAT12 entry that
  straddles two sectors writes the later sector first.
- Allocation and release over `LinksMut`, the table's read and write, so the proofs run them over
  a model table:
  - `Volume::allocate_cluster(extending)` takes a free cluster, marks it end-of-chain, and links it
    after `extending` when the caller names the last cluster of a chain. A failed link gives the
    cluster back. The search starts after the last cluster handed out, or at the `FSInfo` hint on
    the first call. It wraps once and takes only an entry that is free.
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
stage takes the second. Read-only users (the fuzz targets, the macOS image tests, and milestone
140, mount a drive this system did not create, whose first stratum only reads) then never need a
method they cannot honestly implement.

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
| unit tests | allocation from a hint, a wrap, a full volume, bad and reserved entries never taken, every entry of a 350-cluster FAT12 table stored without disturbing a neighbor, an entry straddling two table sectors, a chain freed and cut at every length, a broken chain, a looping chain, a refused write that leaves nothing leaked, and the `FSInfo` signatures and edges |
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

## The second pull request: files

- `Volume::create_file(directory, name)` makes an empty file with an 8.3 name (upper-case letters,
  digits and the 8.3 punctuation) in the first free slot of a directory, and returns its
  `NamedEntry`. It refuses an illegal name, a name the directory holds, and a full fixed root. A
  chain directory that fills grows by a zeroed cluster. The slot after an end marker gets an end
  marker of its own, so stale bytes there stay invisible.
- `Volume::open_file_writer(&entry)` returns a `FileWriter` with `write`, `seek`, `position`,
  `size_bytes` and `set_len`. A write extends the chain and the size, rewrites a partial sector
  with the slack past the old end zeroed, and rewrites the directory entry when the size or first
  cluster changed. A volume that fills part way returns the bytes that fit and then `VolumeFull`.
  `set_len` grows with zeros or shrinks by freeing the clusters past the new end, entry first.
  An error ends the writer.
- `Volume::remove_file(&entry)` marks every slot of the entry free, then frees its chain. A stale
  entry (the slot no longer holds that name) returns `EntryChanged`.
- `NamedEntry` now knows its place: the directory, the first slot of its long-name run and its
  short slot. `Volume::is_clean` reads the clean-shutdown bit of table entry 1 (FAT16 and FAT32).
  The first change after open or sync clears it and `sync` sets it again, but only if it was set
  before: a volume found dirty stays dirty.
- `WriteError` is the one error of create, write, resize and remove.

| check | result |
|---|---|
| Kani, two new harnesses | `a_partial_sector_write_keeps_the_head_and_zeroes_the_slack` proves the sector merge (head kept, slack zeroed, new bytes in place) over every 16-byte sector, since the merge takes any slice length. `a_write_extends_the_chain_to_the_size_and_stays_inside_it` runs the real cursor and chain extension over a model of three one-sector clusters: two writes of up to 700 bytes, the second at any position in the file. The size, the position, the touched sectors and the chain length all come out right and no other cluster is in use. It takes about five minutes. One harness over both did not close in 20 minutes, so the proof is split; the oracle tests join the halves on real volumes |
| falsification | one patch zeroes the slack a byte late, the other shrinks the size on an overwrite; each turns its harness red at its own assertion |
| unit tests | odd chunk sizes, overwrite with slack, clamped seek, grow and shrink, remove and slot reuse, directory entries as files, a full fixed root, a refused write, a damaged chain |
| mutation | `cargo mutants --in-diff` over this pull request tests 287 mutants: 239 caught, 30 unviable, and 18 that `notes/project-metrics/mutation-triage.csv` rules on. Thirteen are equivalent (bounds that only cap a loop, a cursor restart that reaches the same cluster, a full-sector write through the merge path) and five are gaps homed in the block's BUGS: the 65,536-entry directory ceiling is untested |
| fuzz | the parser target found a listing that went on after an error once it ran on the indexed scan; the done flag now lives in the scan, with a unit test |
| oracle tests | on FAT12, FAT16 (512 and 4096-byte sectors) and FAT32 volumes rust-fatfs formatted, the test writes ten files of sizes around the sector and cluster boundaries in odd chunks. rust-fatfs lists and reads the same names and bytes, counts the same free clusters, and `fsck.fat -n` accepts the volume after the create, after an overwrite across a cluster boundary, an append, a shrink, a grow and a delete, and after every file is deleted. The clean bit reads dirty mid-change and clean after `sync`. A FAT32 root grows past a cluster, and a FAT16 root refuses the 513th file |

## Names minted in the second pull request

All provisional: `WriteError`, `FileWriter`, `Volume::create_file`, `open_file_writer`,
`remove_file` and `is_clean`, and the modules `file_write` and `file_writer`.
