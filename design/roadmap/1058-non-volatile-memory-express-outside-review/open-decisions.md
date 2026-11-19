# Open decisions

An appendix to [the milestone](../1058-non-volatile-memory-express-outside-review.md).

## The provisional public names minted after the review

The fixes for this review added public names to the crate after its review package ratified 27.
Each pull request minted them as the maintainers' provisional call, and the release review was to
ratify them together. On 2026-11-19 that review took seven of them one question at a time, the
ones a check against the naming rules had flagged, and its rulings landed in
nifeos/non_volatile_memory_express#35 and #36. It did not ask about the rest,
because nothing in them broke a rule. They are still provisional:

- `admin_queue_attributes` (P1-10), the builder of the `AQA` value;
- `physical_region_page_list` (P2-1);
- `Completion::is_success`, `status_code` and `status_code_type`, and the constants
  `STATUS_CODE_TYPE_GENERIC`, `GENERIC_STATUS_SUCCESSFUL_COMPLETION` and
  `GENERIC_STATUS_INVALID_FIELD_IN_COMMAND` (P2-2);
- `CONTROLLER_STATUS_SHUTDOWN_OCCURRING` and `CONTROLLER_STATUS_SHUTDOWN_COMPLETE` (P2-3);
- `CONTROLLER_OR_NAMESPACE_STRUCTURE_ACTIVE_NAMESPACE_LIST` and
  `IdentifyController::maximum_data_transfer_bytes` (P2-4);
- `NotInFlightError` and `CompletionQueue::expects_phase_tag_set` (P2-5). The lane that renamed
  `InFlight` kept `NotInFlightError`, since an identifier the set does not hold is still a command
  that is not in flight.

Every one spells its words out, passes the name checks of the template, and carries its
specification mnemonic as a `doc(alias)`.

## The question

Should the architect ratify these fourteen names as built? My recommendation is A.

- A: ratify all fourteen as built.
- B: ratify all but `NotInFlightError`, and rename it to match its set, for example
  `NotAcquiredError`. Its meaning stays; the name would follow the type rather than the old one.
- C: leave them provisional until the first publish, and decide them then.

## The door

Two-way today: nothing has published, and a pre-release tag promises no stable names.
One-way at the first publish. Under the naming rules ratification is the architect's call.

## Prior art

Linux names the same values `NVME_SCT_GENERIC`, `NVME_SC_SUCCESS`,
`NVME_SC_INVALID_FIELD`, `NVME_CSTS_SHST_OCCUR`, `NVME_CSTS_SHST_CMPLT` and
`NVME_ID_CNS_NS_ACTIVE_LIST` (read in `include/linux/nvme.h`). This crate spells those words out
and keeps the mnemonics as aliases, as the naming conventions ask. From memory, not read here: the
Linux driver tracks command identifiers with the block layer's tag set, so it has no error type to
compare against `NotInFlightError`.

## The analogous case in the tree

The outside reviews of `xhci` (1060) and
`file_allocation_table` (1061) ended the same way: one ruling each, on 2026-11-16, ratified their
provisional names as built.

## What waits on it

Only the first publish. The pre-release tags and nife's pin do
not wait on it.
