# Prior art for the file_allocation_table crate

An appendix to [the milestone](../1061-file-allocation-table-outside-review.md). The crates.io
figures come from 2026-11-10, total downloads and then the last 90 days. The rest is a reading of
each repository's source and documentation, not an execution of it.

## Crates nearby

| crate | version | downloads (all / recent) | what a user gets there and not here | what is here and not there |
|---|---|---|---|---|
| `fatfs` (rust-fatfs) | 0.3.6, released 2023-01; 0.4.0 unreleased on `main` | 1,846,593 / 467,122 | `std::io` traits, a `TimeProvider`, OEM code pages through `OemCpConverter`, a cluster size chosen by volume size at format, years of users | FAT12 with write and format under `no_std` with no allocator, `forbid(unsafe_code)`, proofs with a falsification patch each, refusals instead of panics (two panics on hostile images are open issues there, #56 and #72) |
| `lamfat` | 0.4.3 | 33,214 / 30,164 | rust-fatfs's unreleased `no_std` line, republished; this crate's test oracle | as above |
| `embedded-sdmmc` | 0.10.0 | 397,543 / 54,599 | an SD card driver, a block cache, `embedded-io`, fixed 512-byte blocks and `u32` indices | FAT12, long-name creation, format, any sector size up to 4,096, no limit on open handles, proofs |
| `hadris-fat` and `hadris-fat-raw` | 2.5.0 (3.0.0-rc.3 in pre-release); 0.3.0 | 10,798 / 4,553 and 266 / 266 | exFAT, a checker, and in `-raw` codecs free of I/O with Kani proofs run daily; the nearest design | FAT12 through FAT32 write and format in one crate, a falsification patch per harness, a hostile-image guarantee stated per parser, an EFI system partition writer |
| `fat32` and the small FAT32 crates | 0.2.0, archived | 14,090 / 360 | nothing a stranger would miss | everything above |
| exFAT crates (`exfat`, `exfat-fs`, `embedded-exfat` and others) | various | under 10,000 each | exFAT | out of scope by design, as the README says |

A crates.io search for "fat32 no_std" lists `fatfs`, `embedded-sdmmc` and `lamfat` among the first
eight by downloads. The keywords of P2-8 put this crate beside them.

## Where this crate differs

Verification is the distinct claim. hadris-fat-raw also runs Kani proofs; the ACL2 LoFAT and
HiFAT models were co-simulated against mtools and Linux as research, not as a driver; FSCQ and
Cogent proved other file systems. No FAT crate was found that ships a falsification patch per
harness, so that is this crate's claim. Being the first FAT crate with Kani proofs is not, and the
README does not make that claim.

A concrete example: a hostile image whose cluster chain loops back on itself. rust-fatfs's tracker
holds two open panics on crafted images. This crate's harness
`no_chain_walk_outlasts_the_volume` proves that a walk from any start over any table contents ends
within the volume's cluster count, on volumes of one to eight clusters, and its falsification patch
makes that proof fail.

What it costs: no exFAT, no `std::io` traits yet (P2-10), no code-page conversion for short names,
and a smaller user base than any crate above.

## The shapes the ruled items follow

- An options value built by `with_` methods (P1-5) is how this crate's own `FormatOptions` already
  works, and how `std::fs::OpenOptions` works.
- Private fields with a checked constructor and getters (P1-6) are how `chrono::NaiveDate::from_ymd_opt`
  and the `time` crate's `Date::from_calendar_date` refuse an impossible date.
- `trim_ascii_end` (P1-7) is the standard library's name on `[u8]` for the same operation.
- A newtype for an on-disk number (P1-8) is what rust-fatfs and embedded-sdmmc do not do: both pass
  the cluster number or an opaque handle index, so a wrong number fails at run time there too.
- One open error shared by the reader and the writer (P1-9) follows `std::fs::File::open` and
  `File::create`, which share `io::Error`.
