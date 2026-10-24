# `jh7110_clock_and_reset`, `non_volatile_memory_express`, `screen_console`

*Name: ratified 2026-10-18 (calef, "Approve all"). Refused `clock-and-reset-nvme-screen-console`.*

The last lane of milestone 326 (nobody has been assigned to turn a mutation score upward) part 3,
on 2026-09-21, and its re-derivation of what the part left. It verifies these three crates' rows in
[notes/mutation-testing.md](../mutation-testing.md).

## 2026-09-21: `jh7110_clock_and_reset`, `non_volatile_memory_express`, `screen_console`, milestone 326 (turn a mutation score upward) part 3

Three more crates, off the seventeen that milestone 326's Follow-on section left outstanding after
the 2026-09-20 lanes. The lane re-derived the list against this note and each crate's history
before picking, per the rule the block gained on 2026-09-21:

- `counter_frequency_contract`, `cpu_set`, `environment_contract`, `login_contract`,
  `thread_wake_handshake` and `uefi_loader` were already at 100% of viable mutants. Other work had
  closed their census-era survivors in the intervening week.
- `documentation` was set aside as a lane of its own: 98 survivors, 1056 mutants, and a
  resolver-feature history spanning [documentation](documentation.md) and
  [first-weekly-runs](first-weekly-runs.md).
- `firmware_configuration`, `portable_executable`, `sealed_pair` and `stick_maker` were new enough
  that no census had measured them, and were left for a lane that can give them attention.

The three below each had a real, moderate, unrecorded survivor count and no complicating history.

(Corrected 2026-09-24: the scheduled census of 2026-09-21, run 35589550926, measured `uefi_loader`
at 356 viable mutants, 48.9% killed, 182 missed. It had been 34 viable at 100% on 2026-09-19. The
crate grew, and "already at 100%" held only for the smaller crate.)

The discipline is [new-crate-backlog](new-crate-backlog.md)'s. Every number was re-derived with
`script/mutation -p <crate>` on this lane's own worktree, not trusted from the census. Every kill
was verified by re-running the sweep and watching the mutant die. Every equivalence claim is a
mutant the second run still reports.

| crate | before | after | killed by a test | equivalent | excluded | recorded gap |
|---|---|---|---|---|---|---|
| `jh7110_clock_and_reset` | 12 | 0 | 12 | 0 | 0 | 0 |
| `non_volatile_memory_express` | 26 | 7 | 19 | 7 | 0 | 0 |
| `screen_console` | 9 | 2 | 7 | 2 | 0 | 0 |
| **total** | **47** | **9** | **38** | **9** | **0** | **0** |

`screen_console`'s `before` does not match the 2026-09-19 census: 7 survivors there, 9 here. That is
the shape the Follow-on section's rule warns about. The crate grew between the census and this lane
(116 mutants now, against roughly 55 tested at the census). Re-deriving, instead of triaging the
census number, is what caught it.

### `jh7110_clock_and_reset`: 12 survivors, all 12 killed

Before: 53 caught, 12 missed, 0 timeouts, 3 unviable. After: 65 caught, 0 missed, 3 unviable. It
matches the 2026-09-19 census row for row.

Three were one blind spot: every reset id this crate ever asks for lands in word zero.
`Domain::reset_bit`'s `word = (id / 32) as u64 * 4` survived `*` to `/`. The two offsets it feeds,
`assert_offset` and `status_offset`, survived both `+`s becoming `-`. The STG domain has 23 resets,
so `id / 32` is `0` for every id this crate has, and `0 * 4` and `0 / 4` are both `0`. The
arithmetic is general on purpose: the module doc names the SYS domain's 126 resets as the reason.
`a_reset_word_past_the_first_multiplies_the_word_index_by_four` constructs a 128-reset `Domain`
directly (a public struct, no fixture needed) and asks for id 35, which is word 1.
`(35 / 32) * 4 == 4`, and all three mutants disagree with that.

Two were the same blind spot one function over. `reset_mask` is private, and every caller of
`was_already_up` happened to leave it 0 or unneeded.
`reset_mask_recovers_the_bit_that_changed_between_the_two_assert_words` calls it directly. It is a
private `const fn` on the crate root, reachable from `tests` like every other private helper here.
With `before = 0b1010, after = 0b0010` the answer under `^` is `0b1000`. That disagrees with all
four mutants (`with 0`, `with 1`, `^` to `|`, `^` to `&`) in one assertion.

Three were `was_already_up`'s reset half, untested from either direction. Every fixture with
`clocks_running() == true` also had the reset fully released. So `!self.had_reset || ... == 0` was
always `false || true`, and deleting the `!` (giving `true || true`) changes nothing. Two new reports
close it:

- One with the reset bit still set in `reset_assert_before`. It kills the deleted `!`. It also
  kills `&` to `^`, because at that operand pair `&` and `^` compute the same nonzero value.
- One with the target bit clear but an unrelated bit set elsewhere in the word. It kills `&` to
  `|`: `&` ignores the noise bit and `|` does not.

Two were `discover_as`'s bounds guard on a name index read out of the tree. No fixture had carried
a tree malformed the way the guard exists to catch. `Some(i) if i < n => i` survived with `i < n`
relaxed to `true` and with `<` widened to `<=`. Every existing `.dts` fixture's `reg-names` list
names exactly as many windows as `reg` has.

A new fixture, `jh7110-clkgen-vendor-mismatched-names.dts`, gives the vendor clkgen node two `reg`
windows but three `reg-names` entries. `"stg"` is last, at index 2, one past the two the array
holds. `discover` over it must fall back to the corroborated `STG_BASE` constant, not read the
zeroed slot an off-by-one would land on.
`a_reg_names_entry_past_the_end_of_reg_is_refused_rather_than_read` checks that.

### `non_volatile_memory_express`: 26 survivors, 19 killed, 7 equivalent

Before: 137 caught, 26 missed, 0 timeouts, 10 unviable (84.0% of viable). After: 156 caught, 7
missed, 10 unviable (95.7%). It matches the 2026-09-19 census row for row.

`Command::write` had never been called by a test. `transfer_command`'s own test,
`the_data_plane_refuses_a_block_the_namespace_does_not_have`, always passes `write: false`. So every
mutant in `write`'s body survived: the LBA split under `>>` and `<<`, and the block count minus one
under `-`, `+` and `/`. `a_write_command_encodes_the_spec_dwords` mirrors the existing read-command
test dword for dword, and closes all three.

`CqState::head` had callers, but each only asserted `head() < entries`, which a constant `0` or `1`
satisfies. `head_reports_the_actual_next_slot_not_a_constant` asserts the actual sequence: `0`, then
`1`, then `2` across two pops.

`cc_enabled` had no caller at all, so its whole body survived: both `|`s, both `<<`s, and the two
whole-function stubs. `cc_enabled_sets_en_and_the_admin_queue_entry_sizes` reads the EN bit and
both entry-size fields back out of the returned word. That kills the two stubs and both `<<`-to-`>>`
mutants. The two `|`-to-`^` mutants are equivalent, below.

`parse_identify_namespace` had two boundaries never tested at the boundary itself:

- The truncation check, `data.len() < 384`, had only been tried far below it (`&[0u8; 100]`), which
  cannot tell `<` from `<=`.
  `identify_namespace_accepts_the_minimum_length_and_refuses_one_byte_less` sits exactly on it.
- The LBA-format-table offset, `128 + 4 * flbas as usize`, had only been tried at `flbas == 0`,
  where `128 + 4*0` and `128 - 4*0` are both `128`.
  `identify_namespace_reads_the_lba_format_table_at_flbas_not_format_zero` uses `flbas == 1`. It
  plants a different, still-valid LBADS byte at format 0's slot, so reading the wrong one is caught
  by value.

`IdentifyNamespace::blocks_per` had two survivors from a test that never varied its `unit`. It
called `blocks_per(4096)` only. 4096 is a multiple of every mask the three mutants could produce
(`||` to `&&`, and the mask's `- 1` to `+ 1` or `/ 1`), so none showed.
`blocks_per_masks_by_the_actual_block_size` adds two calls:

- `blocks_per(0)` kills `||` to `&&`. Once the mask term is forced to `0`, only `0 == 0` keeps that
  branch true.
- `blocks_per(1536)` kills both arithmetic mutants at once. 1536 shares a bit with the wrong masks
  513 and 512, but not with the correct 511.

`Handoff::pack`'s `dstrd` shift and `Handoff::unpack`'s minimum-queue-depth boundary were each
untested at the one case that shows them. The round-trip test used `dstrd: 0`, where `<< 32` and
`>> 32` both give `0`. `pack_shifts_dstrd_left_not_right` uses `dstrd: MAX_DSTRD` (8) and reads the
high word back. `unpack`'s `entries < 2` had only been tried at `entries: 1`, which `<` and the
mutant `<=` both refuse. `SqState::new`'s doc says why 2 is the spec's floor: a one-slot ring cannot
tell full from empty. The same assertion in `the_handoff_survives_three_words_and_refuses_nonsense`
now also checks that `entries: 2` is accepted.

Seven are equivalent, all the same shape: `|` to `^` on bit fields packed into disjoint parts of a
word by construction. A left shift by `n` zeroes the low `n` bits of its result. A value that only
occupies those `n` bits can never share a bit with it, whether because it is narrower or because it
was shifted by at least `n` too. So `|` and `^` compute the same word on every input these functions
combine, not only the ones a test tries.

- `cc_enabled`, 2 mutants. `1` (bit 0), `6 << 16` (bits 17 and 18) and `4 << 20` (bit 22) are fixed
  literals with no shared bit between any pair.
  `cc_enableds_three_fields_share_no_bit_so_or_and_xor_agree` checks all three pairs. The function
  takes no argument, so these are the only operands it will ever have.
- `Command::create_io_cq` and `Command::create_io_sq`, 3 mutants. The left side is `qid as u32` (or
  the literal `1`); the right is `(entries as u32 - 1) << 16` (or `(cqid as u32) << 16`). A `u16`
  promoted to `u32` never sets a bit past 15. A value shifted left by 16 never sets a bit below 16.
  That holds for every `u16` qid or cqid.
  `queue_creation_words_pack_disjoint_halves_so_or_and_xor_agree` checks it at `0xffff`, the widest
  either half can be.
- `Handoff::pack`, 2 mutants. `entries` takes bits 0 to 15, `blocks_per as u64 << 16` bits 16 to
  31, and `dstrd as u64 << 32` bits 32 to 63. The same shift argument applies twice.
  `pack_words_three_fields_share_no_bit_so_or_and_xor_agree` checks each field at its widest value.

### `screen_console`: 9 survivors, 7 killed, 2 equivalent

Before: 104 caught, 9 missed, 0 timeouts, 3 unviable (92.0% of viable). After: 111 caught, 2
missed, 3 unviable (98.2%). It does not match the 2026-09-19 census's 7, because the crate grew
(see above).

`ScreenConsole::span` had no caller in the suite, so both stub mutants survived. The tests do call
`Framebuffer::span` and `Aperture::span`, different methods, which is what let this one hide.
`console_span_is_the_underlying_screens_span` calls it directly.

`clear`'s whole body had never been inspected. Every test that calls `clear` then writes text
covering the whole grid, so a stubbed `clear` and the real one produce the same final picture.
`clear_paints_the_whole_surface_in_the_background_colour` poisons the buffer first, then checks
every pixel right after `clear`, before anything else runs.

The `b'\r'` match arm had never been sent a carriage return. Deleting it falls through to the
default arm, a no-op. Because the match does not return in that case, it then draws a blank glyph
at the un-reset column and advances past it. That is a different picture from "reset the column,
draw nothing". `a_carriage_return_resets_the_column_without_drawing_a_glyph` sends `"a\rb"` and
checks that `b` landed at column 0, over `a`, not at column 1.

`scroll`'s guard, `live > pixels.len() || band >= live`, had no branch tested at its boundary.
Every fixture's pixel buffer (4096 * 16 bytes, fixed) is far larger than any `live` region these
tests compute. So `live > pixels.len()` was never true, and its `==` and `>=` mutants survived.
Every fixture had at least two rows, so `band >= live` was never true either, and the `||`-to-`&&`
mutant survived beside it.

- `scrolling_a_one_row_console_leaves_it_alone_rather_than_blanking_it` builds a one-row console,
  where `band == live` by construction, and writes past its last column. Under the `&&` mutant the
  early return is skipped. `scroll` then paints the entire live region to the background color
  before the next character is drawn, blanking everything the console just wrote. The test checks
  that the two characters written before the wrap are still there.
- `scroll_accepts_a_buffer_that_is_exactly_as_long_as_the_live_region` sizes a buffer to exactly
  `live` bytes. It is a `const`, because the crate is unconditionally `no_std` and has no `Vec`.
  It checks the buffer is scrolled, not refused, which `>` and `==` or `>=` disagree about at
  exactly that point.

Two are equivalent, the disjoint-bit-field shape again. `Aperture::to_words`'s two words each pack
a `u32` (`width`, `stride`) in the low half against a value shifted left by 32 (`height`, `order`)
in the high half. `to_words_two_halves_share_no_bit_so_or_and_xor_agree` checks it at `u32::MAX` on
both sides, the widest either half can be.

### No defect in shipped behavior was found in any of the three

Every survivor across the three crates closed as a missing test or a demonstrated equivalence. None
needed an exclusion or a recorded gap. None is a behavior question left open, the way
`timetable::Unbacked`'s `BUGS` section is.

The Follow-on section's "seventeen crates" does not reconcile with what this lane found. Walking the
tree's mutation-scoped crates (`script/mutation --list`, 66 crates) against
`.cargo/mutants-baseline.txt` finds 29 crates outside the baseline, not 26. The baseline file lists
the 38 baseline crates and carries every rename applied since. Some of the 29 are new since the
2026-09-14 census: `firmware_configuration`, `portable_executable`, `sealed_pair` and `stick_maker`.
And `component_plan`, `credentialer`, `loaded_image_check`, `pgrep`, `pmap`, `ps` and `uptime`
were apparently missed by the count the block's "seventeen" came from. The already-closed crates
checked at the top of this section do not explain the gap either. They are
`counter_frequency_contract`, `cpu_set`, `environment_contract`, `login_contract`,
`thread_wake_handshake` and `uefi_loader`, plus `timetable` and `work_steal_slot` from earlier
parts: exactly the ones "seventeen" should already have excluded.

What was left on 2026-09-21, re-derived rather than assumed:

- A real, moderate survivor count, not yet re-measured against the current tree: `component_plan`,
  `credentialer`, `loaded_image_check`, `pgrep`, `pmap`, `ps`, `uptime`.
- A lane of its own, for size and history: `documentation`, at 98 survivors when last measured, 1056
  mutants, and a resolver-feature story spanning two appendices.
- Never measured by a census, so no `before` number exists: `firmware_configuration`,
  `portable_executable`, `sealed_pair`, `stick_maker`.

Twelve crates, not fourteen or seventeen. A lane taking the next piece should re-derive the list
with `script/mutation --list` against the baseline file, and trust neither number.

(Corrected 2026-09-24: the 2026-09-21 census has since measured all twelve. Missed plus timeouts:
`stick_maker` 103, `portable_executable` 76, `documentation` 97, `component_plan` 18, `ps` 7, `pmap`
7, `pgrep` 6, `firmware_configuration` 5, `credentialer` 3, `sealed_pair` 3, `loaded_image_check`
2, `uptime` 1. `uefi_loader`, listed above as closed, reads 186.)
