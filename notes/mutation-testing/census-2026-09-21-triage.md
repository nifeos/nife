# The 2026-09-21 census's 771 survivors, classified

Milestone 326 (nobody has been assigned to turn a mutation score upward), 2026-09-24. This appendix
of [notes/mutation-testing.md](https://github.com/nifeos/record/blob/main/notes/mutation-testing.md) sorts every missed mutant of the scheduled
census of 2026-09-21 ([run 35589550926](https://github.com/nifeos/nife/actions/runs/35589550926))
by what it is, then closes the largest class that can be closed with a test. Its per-mutant list is
the union of the eight shards' `missed.txt` from that run's artifacts, which expire around
2026-12-20.

The discipline is [new-crate-backlog](new-crate-backlog.md)'s. Every before and after number is
`script/mutation -p <crate>` on this lane's worktree, a kill counts only once the sweep has seen the
mutant die, and an equivalence claim is a mutant the second sweep still reports. The sweeps ran on
macOS; the census runs on Linux, which matters for one crate and is said there.

## The classification

| class | missed | what it is | where it went |
|---|---|---|---|
| a file no host test build compiles or can run | 193 | `uefi_loader/src/arch/` (132) and `stick_maker/src/host/` with its `main.rs` (61) | excluded in `.cargo/mutants.toml`, each with its reason |
| a missing test, in the four crates this lane took | 184 | `portable_executable` 76, `stick_maker`'s deciding files 40, `uefi_loader`'s pure files 50, `paging`'s growth since August 18 | 164 killed, 20 equivalent (below) |
| already triaged by milestone 326's earlier lanes | 153 | recorded equivalents and gaps (114), and 39 killed on 2026-09-21 and 2026-09-24 after the census ran | the appendices named in the main page's table |
| carried from the August baseline triage | 152 | baseline crates at or under their triaged August count, `paging`'s 39 included | [baseline-survivors-grant-plan-to-swish](baseline-survivors-grant-plan-to-swish.md) and its sibling; **not re-derived** |
| untriaged | 89 | `documentation` 47, `component_plan` 11, `ps` 6, `pgrep` 6, `pmap` 5, `firmware_configuration` 5, `sealed_pair` 3, `loaded_image_check` 2, `uptime` 1, and one each over the baseline in `globally_unique_identifier_partition_table`, `entropy_protocol`, `socket_protocol` | milestone 326's Follow-on |

By kind rather than by history, the 771 split three ways:

- 193 are dead to the host.
- 286 are equivalent or a recorded gap: this lane's 20, the earlier lanes' 114, the carried 152.
- 292 were missing tests. 203 are now written (this lane's 164, the post-census lanes' 39), and 89
  are untriaged, so some of those may yet prove equivalent.
 The "carried" row is the weakest number here: a
crate whose count did not rise since August very probably holds the same survivors, and nobody has
checked mutant by mutant.

The dead-code class was the largest single class, as it has been every time this instrument was
read. Two of its three instances were found before this lane (`system_initializer`, and
`uefi_loader/src/main.rs` with its module tree, excluded earlier on 2026-09-24). The third is new:
`stick_maker/src/host/` is `#[cfg]`-selected per host OS, so on the Linux census runner its macOS
and Windows arms are not compiled at all, and its Linux arm erases a block device as root.
`script/coverage` already exempted exactly these two paths with that reason; `.cargo/mutants.toml`
says the two lists move together and they had not.

## The four crates

| crate | census missed | before (this tree) | after | killed | equivalent | excluded |
|---|---|---|---|---|---|---|
| `portable_executable` | 76 | 76 | 5 | 71 | 5 | 0 |
| `stick_maker` | 101 | 40 of 101 mutated | 2 | 38 | 2 | 61 |
| `uefi_loader` (census files) | 50 | 55 | 10 | 45 | 10 | 0 |
| `paging` | 57 | 57 | 45 | 12 | 45 | 0 |

`uefi_loader`'s before is five higher than the census because `handoff.rs` grew a token since; the
census's own 50 split 43 killed and 7 equivalent. Its fourth file, `device_tree_from_acpi.rs`, did
not exist on 2026-09-21 and is not counted here (14 survivors, in the Follow-on).
`stick_maker`'s two `linux.rs` escape mutants moved from missed to timeout, which this note's
convention counts as a kill: `i += 1` becoming `-=` or `*=` on an escape past 255 never advances.

### `portable_executable`: one real wrong-accept

The converter that makes radon's `BOOTRISCV64.EFI`. The one test that read the output checked the
fields a reader looks at first. The new tests pin:

- every header field UEFI reads, and each machine's own `RELATIVE` type;
- the dynamic walk's two ends (`DT_NULL` and the segment), `DT_RELAENT` as the stride, and the
  refused relocation tables;
- the segment layout at its exact edges, and the 94-section header limit;
- a 56-section case, found on the way. Its 2,568 bytes of header sit eight past a 0x200 boundary,
  the only kind of count that tells `DOS_LEN + 4` from `DOS_LEN - 4`.

The wrong-accept: the relocation bound read `memsz`, but a section's raw data is its first
`filesz` bytes. A `RELATIVE` whose word lies in `.bss` had its addend dropped while its fixup stayed
in `.reloc`, so the firmware would add the load base to zero. The error variant's own doc said "file
bytes". Fixed; no linker emits such a relocation for a static PIE, so no shipped image was wrong.

Equivalent: five `|` to `^` over disjoint bit constants (COFF characteristics, DLL characteristics,
the `DIR64 << 12 | offset` entry), an algebraic identity.

### `stick_maker`

Killed:

- sysfs volume assembly: a superfloppy stick, a bare loop device with no volume, udev as the only
  witness for an unmounted partition, and sizes in 512-byte sectors;
- octal escapes that use all three digits, and one past a byte;
- the plist reader's closing-tag match, its `<dict/>`, `<data>`, `<date>` and `<real>`, and a
  refusal's byte offset;
- the macOS superfloppy with only a `VolumeName`, and the model-name tie;
- the Windows bus names, a tie between two equal FAT volumes, and the unit table's last entry;
- the aarch64 and riscv64 boot instructions.

Equivalent: `close_tag`'s `unwrap_or(expected_len - 1)`, twice. The guard before it has already
seen a `>` in `rest`, so `find('>')` cannot miss and the fallback is never evaluated.

### `uefi_loader`

`device_tree_patch.rs` is what tells an aarch64 or riscv64 kernel where its archive is on a UEFI
boot. Hand-built trees now pin:

- each header bound alone, and the legal layouts beside it;
- the reservation block entry for entry, and the rewritten header's fields;
- `NOP`s skipped and dropped, and `/soc/chosen` treated like any other node;
- the replace stopping at `/chosen`'s end;
- a property longer than its block refused rather than sliced;
- one distinct console sentence per refusal.

`output_len` is now exact rather than padded by two unexplained eights, and a test holds it equal
to what `with_initial_ramdisk` writes for the one tree shape that needs all of it. Fifteen of its mutants
had survived because a bound with slack cannot be told from a wrong one. Rung one: four of them no
longer exist.

`handoff.rs`: slot 10, which has no one-digit token, leaves no stray space; the hold token alone is
not indented. `image.rs`: an entry exactly at one segment's end belongs to the next.

Equivalent, 10:

- `find_string`'s `<` to `<=`: at the end, `strings[len..]` is empty, `position` is `None`, and
  `?` returns what the loop exit returns.
- The property bound's `>` to `>=`: a property ending exactly at the structure block's end is
  followed by no `END`, so the next turn of the loop refuses it `Truncated` either way.
- `CMDLINE_LEN`'s sum, eight mutants: every one either grows the constant or shrinks it by at most
  two, and `Framebuffer::MAX_LEN` is 72 against a worst-case token of 63, nine bytes of rounding no
  line can reach. The runtime `assert!` and `machine_discovery`'s "never exceeds its maximum" test
  are what hold the real bound.

### `paging`

Killed:

- a span whose `va` and `pa` sit at different offsets within 2 MiB maps in pages throughout, on
  all three CPU formats;
- `largest_fitting` needs both addresses aligned, not their intersection;
- Sv39's leaf test on each permission bit, and VT-d's page-only `block_entry` and bit-7 superpage;
- `Ia32e` exec bits round-trip, and a device leaf carries `PWT`. PCD alone selects PAT entry 2, UC-, which an
MTRR can weaken to write-combining; PCD with PWT is entry 3, strong UC (the PAT layout is recalled
from the SDM, not re-read).

Equivalent, 45: the baseline's two families, now 44 strong with the x86_64 and VT-d formats' own
members (`|` to `^` over disjoint attribute bits and the address mask; `1 << 0` and `0b00 << 6`),
and one more. `leaf_flags`' `entry & SW_KERNEL_EXEC` is only reached for an entry with `XD` and `US`
clear, and this encoder sets `SW_KERNEL_EXEC` on every such entry, so reading it as always true
changes nothing an encoded entry can show. An entry some other writer made is the one case it would.

## What the rate does

Projected from these sweeps onto the census's own counts, not measured by a census; the next
scheduled run is the check.

| | viable | killed | missed | rate |
|---|---|---|---|---|
| 2026-09-21 as recorded | 10,178 | 9,407 | 771 | 92.4% |
| less the 193 unbuilt | 9,985 | 9,407 | 578 | 94.2% |
| plus this lane's 164 kills | 9,985 | 9,571 | 414 | **95.9%** |
| plus the 39 killed by other lanes after the census | 9,985 | 9,610 | 375 | 96.2% |
| like-for-like, the 38 baseline crates | 6,747 | 6,487 to 6,499 | | 96.1% to **96.3%** |

Like-for-like moves little because `paging` is the only baseline crate this lane touched; the other
three are new since August, which is where the census's survivors were.

## BUGS

- **The coverage and mutation exemption lists still agree by hand at file level.** `script/lint`
  derives the crate-level and `required-features` exclusions; `stick_maker`'s file-level exemption
  is a comment on each side saying "change the two together", which is rung three and drifted once
  already.
- **The "carried" row is inferred from counts, not re-derived.** 152 survivors are assumed to be
  the August triage's equivalents because their crates did not grow a survivor since. A re-run of
  each of those crates against its August list would turn the assumption into a measurement.
- **`uefi_loader`'s `cfg(feature = "screen_hold")` code is mutated only because the package's
  dev-dependency on itself turns the feature on.** That is `script/lint`'s gate, and it held here.
