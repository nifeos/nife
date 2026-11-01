# Stage 7: directories, rename and long-name creation

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what stage 7 built and the evidence.

## What stage 7 built

- `Volume::create_file` takes any legal name. A legal upper-case 8.3 name takes one slot. Any other
  name (mixed case, spaces, several dots, up to 255 UCS-2 units) takes a run of long-name slots
  and then a short entry, with its checksum, written last. The alias carries a number: `~1` first, and
  the next number up while another entry of the directory holds it, with the base shortened to
  keep the tail inside eight bytes (`ALONGN~9`, `ALONG~10`, `ALON~100`). A freed alias returns to
  use. A name that differs only in case from an existing one returns `AlreadyExists`.
- Placement. `RunFinder` takes the first run of free slots that is long enough, and an end
  marker completes the run. A directory short of slots grows by a cluster; the fixed FAT12 and FAT16
  root cannot. The slot after an end marker gets an end marker of its own, so stale bytes there
  stay invisible.
- `Volume::create_directory` allocates a cluster, fills it with `.` and `..` and zeros, then writes the
  entry; a failure gives the cluster back. A `..` whose parent is the root holds 0, and a FAT32
  root named by its cluster number is the root.
- `Volume::remove_directory` refuses a file, `.`, `..` and a directory that holds anything else, then
  frees the slots and the cluster.
- `Volume::rename(entry, directory, name)` renames in place or moves to another directory. It
  writes the new entry, then frees the old slots; the data does not move. A moved directory gets a new `..`. It refuses a name another entry holds (a change of case alone passes), a
  directory moved into itself or below itself, `.`, `..` and the label.
- `WriteError` gains `NotADirectory`, `DirectoryNotEmpty` and `MovedIntoItself`.

No nife type appears in a public signature.

## The evidence

| check | result |
|---|---|
| Kani, two new harnesses | `every_numbered_alias_carries_its_number` covers every name up to the alias bound and every number from 1 to 999,999: eleven legal bytes, one to six base bytes, and the digits read back as the number (three minutes). `the_run_finder_returns_the_first_run_of_free_slots` covers every sequence of six slots and runs of one to three: nothing used inside the run, the run begins after a used slot or at the start, and no run means no end marker and a used slot in every window |
| falsification | a finder that forgets a used slot, and a tail written with its digits reversed; each turns its harness red at its own assertion |
| unit tests | run placement across gaps, aliases from `~1` to `~999999`, collisions and reuse of a freed alias, directories with dot entries, removal only when empty, a failed make that gives its cluster back, renames that keep the bytes, `..` rewritten on a move, refusals for moves into itself and for dots and the label, stale entries, case-only renames |
| oracle tests | on FAT12, FAT16 and FAT32 volumes rust-fatfs formatted: fifteen names with one alias base, a 255-unit name, non-ASCII names, nested directories, files moved across directories and a directory moved with its contents, then everything removed. rust-fatfs lists the same tree and `fsck.fat -n` accepts the volume at each stage. On FAT32 with 512-byte clusters above cluster 65535, directories keep their high half through make and move, and a new directory in a four-sector cluster over stale bytes reads empty |
| mutation | the ledger rules on the survivors that tests cannot kill: two gaps (the 65,536-entry ceiling, homed in the block's BUGS) and three equivalents (search-bound arithmetic and a disjoint-bit OR) |

## Names minted

Ratified 2026-11-01 (UTC): `Volume::create_directory`, `remove_directory` and `rename`, the `WriteError`
variants above, and the module `directory_writer`. `create_file` keeps its name and widens its names.
