# Ledger

An appendix to [the milestone](../1105-pci-outside-review.md).

Names minted by this work: none.

- 1 LEFT: the crate leaving nife, which moves the fixture tests; the move's preparation.
- 2 LEFT: the same move preparation (manifest, README, citations, the regenerated patch).
- 3 DONE before this work, by `ae5b830f3`: `a_bar_mask_with_a_hole_is_refused`,
  `a_64_bit_bar_whose_upper_half_reads_zero_is_refused`, `a_bar_with_no_writable_bits_is_refused`,
  `a_bar_whose_end_overflows_is_refused`, and the proof `a_bar_size_is_where_the_writable_bits_begin`
  with its patch.
- 4 DONE before this work: `a_64_bit_bar_in_the_last_slot_is_refused_without_probing_past_it`,
  `a_bridge_has_two_bar_slots` and `an_unknown_header_layout_has_no_bars`. A bridge probes its two
  slots and no other header probes any.
- 5, 6 LEFT: renames of public names, each for the architect's ratification.
- 7 LEFT: the documentation rewrite, with the move.
- 8 PARTIAL: `the_msi_walk_terminates_on_any_device` arrives with its falsification patch
  (`the_msi_walk_ends_on_a_cyclic_list` is its host twin). The factoring of the three walks into one
  and the ceiling proof are not done.
- P2 `cap_len`: DONE (`a_vendor_capability_shorter_than_its_type_needs_is_skipped`).
- P2 `intx_interrupt_number` precondition: LEFT; `intx_pin_through_bridge` is a new public name.
- P2 `Bdf` fields: DONE by masking in `ecam_offset` and `requester_id`
  (`a_device_or_function_past_its_field_is_masked_to_it`); the proof no longer assumes the ranges.
- P2 `#![forbid(unsafe_code)]`: DONE. The fuzz targets are LEFT (a fuzz opt-in is a decision).
- P2 stale patch headers, the verification table rows and the mutation survivors: LEFT with the move.
