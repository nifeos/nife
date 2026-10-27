# The 2026-10-03 census's survivors, triaged crate by crate

Milestone 637 (triage the crates the 2026-09-21 mutation census measured for the first time), provisional number, 2026-10-03. This appendix of
[notes/mutation-testing.md](../mutation-testing.md) holds the accounting for the survivors of the
scheduled census of 2026-10-03 ([run 37108924347](https://github.com/nifeos/nife/actions/runs/37108924347)):
85 crates, 14,853 mutants, 1,004 missed, 92.7% killed. The per-mutant list is the union of the eight
shards' `missed.txt`, which expire around 2026-12-31.

The method is [new-crate-backlog](new-crate-backlog.md)'s. Every after number is
`script/mutation -p <crate>` on the lane's worktree, a kill counts only once the sweep saw the mutant
die, and an equivalence is a mutant the second sweep still reports. The order is the proposal's, so
the crates the 2026-09-21 triage never reached come first.

## component_plan

12 missed, 10 killed, 2 equivalent. The sweep afterwards reports 2 missed.

| mutant | disposition | test or reason |
|---|---|---|
| `problem` 439 and 452, `<` to `<=` on the outer duplicate-role loops | equivalent | at `i == len` the inner loop starts at `j = len + 1`, never runs, and nothing indexes `caps[i]` |
| `problem` 476, `<` to `==` or `>` on the handoff-role scan of `caps` | killed | `a_capability_named_for_the_handoff_page_is_a_duplicate` |
| `Refusal::message` returning a constant | killed | `no_two_refusals_say_the_same_thing` |
| `plan` 686 and 691, `>` to `>=` or `==` on the two bounds | killed | `a_plan_exactly_at_each_bound_is_built_and_one_over_is_refused` |
| `slot_of` 779, `<` to `<=` | killed | `asking_for_a_slot_nobody_declared_names_the_mistake` (the panic text differs from an index error) |
| `dependents` 926, `>` to `>=` on `MAX_LIVE` | killed | `a_target_named_second_is_found_and_a_full_registry_is_answered` |
| `dependents` 944, `+=` to `-=` or `*=` on the scan step | killed | the same test, whose dependent names the target second |

## ps, pgrep, pmap

19 missed: 16 killed, 3 equivalent. A second sweep of each reports only the three equivalents.

- `ps` (8, all killed): `state_name`'s `READY` and `BLOCKED` arms, `refusal`'s `NoSuchSlot` and
  `WrongObject` arms, and the digit-count subtraction in `write_ms` and `write_thread_id`. The
  tests are `every_state_has_its_name`, `each_refusal_is_explained_in_its_own_words` and
  `the_columns_pad_to_their_widths`. The padding mutants only differ from the original for a value
  of two or more digits, so the test uses several widths.
- `pmap` (5, killed): the `MAP_RO` arm, the two refusal arms, and `write_va`'s two comparisons.
  Zero is the case that needed its own test: with `start <= 17` the strip loop eats the last digit
  and prints `0x` alone.
- `pgrep` (6): `the_mask_is_the_named_bits_and_a_wild_code_selects_nothing` kills `Selector::mask`
  returning a constant and `selects`'s `state < 64` bound (the `<=` mutant shifts by 64 and
  overflows). The three `|` to `^` mutants in `state::EVERY` are equivalent: the four state bits are
  disjoint, so OR and XOR agree.

## firmware_configuration and sealed_pair

8 missed: 4 killed, 4 equivalent.

- `firmware_configuration`: a name that fills all 56 bytes
  (`a_name_that_fills_the_field_is_compared_without_reading_past_it`; the mutant indexes past the
  field) kills `DirectoryEntry::is`'s `>` against `>=`. The four `|` to `^` mutants in
  `DmaCommand::read` and `write` are equivalent: `key << 16`, `SELECT` and `READ` or `WRITE` occupy
  disjoint bits.
- `sealed_pair` (3, killed): `Unreadable`'s `Display` returning an empty string, `explain`'s
  vouched count (`== Vouch::Yes` against `!=`), and the `!disagreements.is_empty()` guard replaced
  by `true`. The tests are `an_unreadable_pair_says_which_way`, an assertion on the sealed line's
  count, and `a_clean_table_beside_a_refusing_kernel_reports_no_disagreement`.

## loaded_image_check and uptime

3 missed: 2 killed, 1 equivalent.

- Writing past the buffer directly kills `uptime`'s `Writer::push_bytes` bound (`<` against `<=`);
  `format` itself can never get there (29 bytes of 40).
- Reading back the `.bss` marker, which `verify` sets to 1 and nothing else touches, kills
  `loaded_image_check::verify` replaced by `()`.
- `stack_works -> true` is equivalent as a value: every path of the function returns true, and its
  job is to touch stack, which only a run inside the guest can show.

## globally_unique_identifier_partition_table

7 missed: 1 killed, 6 equivalent.

- Killed: the `NIFE_BOOT` arm of `types::name` (`every_known_type_has_its_name`).
- Equivalent, by algebra: `ATTR_REQUIRED`'s `1 << 0` against `1 >> 0`.
- Equivalent, by disjoint bits: the three `|` to `^` in `v4_from_random` (a masked nibble OR a
  constant in the cleared bits) and `try_from_ascii` (`hi << 4` OR a nibble).
- Equivalent, by the next test: `parse`'s `block_count < backup_reserved` against `<=`. At equality
  the following comparison, `last_usable_lba >= 0`, refuses the same header.
- Equivalent on a 64-bit host: `Span::buffer_bytes`'s guard `n <= usize::MAX as u64` is always true
  when `usize` is 64 bits. Only a 32-bit host could tell, and no target here is one.

## entropy_protocol and socket_protocol

6 missed, all equivalent. Each is a `|` to `^` where the operands are disjoint by contract.

- `entropy_protocol`: `req` (opcode in bits 63:56, count masked to `0xff`), `bringup_failure`
  (`DEAD` in the high half, a step in the low byte) and `want`'s `n > MAX_BYTES` against `>=`
  (both return 8 at `n == 8`).
- `socket_protocol`: `req` (opcode below 256, socket id shifted by 8), `listen_grant` and
  `udp_bind_grant` (each field `u16` shifted into its own half).

The `|` ones are equivalent for in-contract inputs. An opcode over 255 or a step with `DEAD`'s bits
would tell them apart, and no caller builds one.

## uefi_loader

24 missed: 10 killed, 2 equivalent, 12 recorded as gaps.

- `device_tree_from_acpi.rs` (14): 10 killed, 4 left. The tests cover the header's size fields,
  `unit_name`'s digits either side of nine, `cells64` with a high word, `string_offset`'s panic on
  an unknown name, `prop_empty`'s three words, and the allocation bound against the largest tree.
  The four left are `output_len`'s subtractions and a multiplication inside its slack.
- `device_tree_patch.rs` (2, equivalent): `find_string`'s `at < strings.len()` against `<=`. An
  empty tail finds no NUL and returns `None`, as falling out of the loop does.
- `with_initial_ramdisk`'s `next > struct_end` against `>=` is the other. A property ending exactly at the
  struct end reaches the loop head's `at >= struct_end` and refuses with the same error.
- Gaps (12): `device_tree_from_acpi::output_len` (4) and `handoff::CMDLINE_LEN` (8). Both are
  bounds with deliberate slack, so a mutant that shrinks or grows one by less than the slack
  changes nothing a caller can see. `with_initial_ramdisk`'s bound had the same shape until milestone 326 (nobody has been assigned to turn a mutation score upward)
  made it exact and a test hold the two equal. The same move would close these: size the buffer from
  the layout, or assert the exact sum. The second costs a test that restates the constant, which is
  why it was not written here.
