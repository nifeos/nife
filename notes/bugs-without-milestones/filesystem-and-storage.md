# Filesystem and storage

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  briefs/README.md | unmeasured: lane ceiling under CI gating, redo rate of delegated tasks, brief payoff beyond one task shape | Measure the delegation briefs: lane ceiling and redo rate | M  |
|  briefs/gate-a-lane.md | unmeasured whether the brief stops clause-dropping | Measure whether gate-a-lane stops clause-dropping | M  |
|  components/src/disk_partitioner.rs, disk_surveyor.rs, system_installer.rs | logical block size assumed 512 (4Kn disk read wrongly) and installed fs server holds whole disk; needs a block-size and base-block field on the blk wire | blk wire carries logical block size and base block | M  |
|  design/decisions/0056-verb-table.md | `fs_file_caretaker` answers EBADF to seven directory verbs where ENOTDIR is very likely right; behavior kept on purpose, wire change owed. | Directory verbs on a file handle answer ENOTDIR | S  |
|  design/decisions/0307-how-long-a-name-may-be.md (rule 5 bullet) | `jig install` caps a package name at 16 bytes until "the grant milestone" lands; no milestone found. | Widen the grant name so long package names install | M  |
|  design/roadmap/0122-a-directory-handle-std-can-hold.md | A revoked FS endpoint reads as `PermissionDenied`; kernel `NoSuchSlot` and server `EPERM` are both `-1`; clean fix is a tag in the reply word | Disjoint kernel and server error spaces in the FS reply | M  |
|  design/roadmap/0122-a-directory-handle-std-can-hold.md | Per-component IPC cost unmeasured; `File::open` on a path to be created walks twice (OPEN then CREATE) | Measure the walk, then stop `File::open` create walking twice | M  |
|  design/roadmap/0122-a-directory-handle-std-can-hold.md | Retained `Dir` lifetime and revocation unspecified (cites 108, which is BUILT and does not answer it); holding many `Dir`s at once untried | What a held Dir does when its directory is revoked | M  |
|  design/roadmap/0138-file-io-throughput.md | `TRANSFER_BLOCKS` and `CACHE_SLOTS` never swept against each other; 64 slots not measured on a multi-file workload (merged two entries) | Joint sweep of block-batch and cache-slot size | S  |
|  design/roadmap/0138-file-io-throughput.md | Space cost of the 8 KiB record not re-measured on incompressible payload | Re-measure record-level space cost on a backup payload | S  |
|  design/roadmap/0244-the-largest-crate-in-the-tree-is-proved-by-nothing.md | supervision_proto, swap_proto, virtio excluded for the same reason and never measured (note: 0244 is REFUSED) | Measure the mutants of the other three excluded crates | S  |
|  design/roadmap/0255-the-arch-layer-the-prover-cannot-see.md | x86_64/iommu.rs is not compiled under the prover on any runner; riscv64 IOMMU is 0432's | x86_64 IOMMU under the prover | M  |
|  design/roadmap/0515-the-installer-a-stick-runs-to-put-itself-on-the-disk.md | SATA (AHCI) and RST/VMD disks are not covered; "nothing owns an AHCI driver" | An AHCI driver as a confined server | L  |
|  notes/benchmarks/read-path-block-contract-and-metadata-cache.md (bullet 2) | 64-slot metadata cache never measured with many distinct open files | Measure the metadata cache under many open files | S  |
|  notes/benchmarks/read-path-block-contract-and-metadata-cache.md (bullets 2 and 3) | `blk::TRANSFER_BLOCKS` never swept on its own; a larger record level with step 4 never re-measured | Sweep block transfer size and record level with step 4 in place | S  |
|  notes/benchmarks/read-path-record-and-request-size.md (bullet 1) | space cost of level 1 never measured on incompressible data (the backup case) | Measure record-level space cost on incompressible payload | S  |
|  notes/dir-capability.md | No verb reports what rights a handle carries | A rights-query verb on the file contract | M  |
|  notes/dir-capability.md | A missing directory in a grant prints "progenitor out of memory"; needs a second sentinel on the spawn result endpoint | An honest word for a grant whose directory is missing | S  |
|  notes/dir-capability.md | A grant more than one level down is refused; the chain of caretakers is not built (328 leaves this to BUGS) | Chain of caretakers for deep grants | M  |
|  notes/grant-expression.md | Six directory verbs answer `EBADF` where `ENOTDIR` is likely right; a wire-visible contract decision | Rule on `ENOTDIR` versus `EBADF` for directory verbs | S  |
|  notes/nifefs.md | a one-block reader sees only the first 12 files and the format does not announce it | Directory length in the superblock | M  |
|  notes/nifefs.md | duplicate file name is not an error, first wins | Refuse duplicate names in nifefs | S  |
|  notes/proof-retrospective.md | Nothing mutates the subject code to see which harness goes red | Mutate subject code, not harnesses: does a proof harness fail when its subject breaks | M  |
|  notes/user-proofs.md | editor editing ops (`insert_char`, `insert_newline`, `backspace`, `delete_at_cursor`) are the richest untrusted-input surface and out of Kani's reach | Make the editor's editing operations provable | L  |
|  notes/walk-cost.md | `fs_walk` waits forever on a program that aborts (no end-of-stream marker); kernel `drain_sink` has the same gap | Make a walk and a drain end when the program aborts | S  |
|  redoxfs_server/src/bin/redoxfs_server.rs | window 0 is shared by the shell, login, provisioner and progenitor, kept apart only by being blocked (milestone 599 (A frame per filesystem client channel) BUILT covers jobs only) | Give the boot's long-lived filesystem clients their own windows | M  |
|  roadmap 0606 | an aborting program never sends end-of-stream, so `fs_walk` and `drain_sink` wait forever | An aborting program sends its end marker | M  |
|  roadmap 0790 | `VIRTIO_BLK_WRITE_ABANDON` may also be test-only; not checked for the feature gate | Gate the second blk attacker role | S  |
