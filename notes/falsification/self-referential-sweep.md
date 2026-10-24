# The sweep for self-referential harnesses

*Name: provisional.*

An appendix to [`notes/falsification.md`](../falsification.md), which states the result.

Milestone 211, 2026-09-01. This note's own line about `derive_never_widens_rights` (a harness
that states its property through the predicate it is testing cannot see that predicate break)
was written as a description of one harness. Milestone 202 then found the same shape in
`component_plan`, independently, two days later. Two instances in two crates is a class, so
every harness in the tree was asked the question.

146 harnesses swept. That is the 141 `script/falsifications` walks in `crates/`, plus the
two in `kernel/src/syscall.rs`, the two in `components/src/printenv.rs` and the one in
`vendor/redoxfs/src/node.rs`, which the walk does not reach (milestone 212 is fixing that).

11 were blind, and blind is a measurement here rather than a reading. For each one there is a patch
in the tree that applies a defect to the function the harness stated its property through; the
pre-211 phrasing was run against that patch and stayed green, and the rewritten harness goes red.
Both directions were checked for every one, because the whole point of
§134 (a harness carries a machine-replayable falsification record) is that a claim about a
proof is not evidence about a proof.

| harness | stated through | the defect it could not see |
|---|---|---|
| `capability::derive_never_widens_rights` | `Rights::is_subset_of`, which `derive` guards on | the predicate's two operands swapped |
| `component_plan::a_plan_never_grants_a_right_the_declaration_did_not_ask_for` | `Direction::rights`, which `plan` fills the word from | `Use` routed `READ`, so a client can receive on its server's rendezvous |
| `component_plan::the_device_split_partitions_the_mappings` | `PageKind::mode`, which `plan` fills the word from | device registers mapped `MAP_RW` |
| `component_plan::dependents_finds_exactly_the_non_target_instances_that_declared_it` | `str_eq`, which `dependents` decides membership with | `str_eq` true for strings of different lengths |
| `credential_contract::no_request_word_makes_the_parse_read_outside_the_page` | `id_len`, which `read` slices with | the identity length read from the wrong four bits of the request word |
| `direct_memory_access_validator::an_accepted_descriptor_is_confined` | `Desc::is_indirect`, which `check_descriptor` guards on | the indirect flag tested against the wrong bit, so an indirect table reaches the device |
| `jh7110_entropy::ready_requires_rand_rdy_and_carries_the_words_untouched` | `assemble`, which `interpret` calls | entropy laid out big-endian |
| `network_time_protocol::accepting_is_total_and_a_sample_is_coherent` | `Interval::is_negative`, which `accept` guards on | the predicate never true, so a negative delay is accepted |
| `paging::sv39::the_leaf_keeps_address_and_permissions_apart` | `entry_pa`, the decoder for the encoder under test | `PPN_SHIFT` moved, so both agree on the wrong bits |
| `paging::aarch64::the_leaf_keeps_address_and_permissions_apart` | the same round trip | the descriptor-type field dropped, so every leaf faults |
| `paging::x86_64::the_leaf_keeps_address_and_permissions_apart` | the same round trip | the present bit dropped |

All eleven now carry a replayable record of exactly that defect, which is the point: the evidence
that a rewrite closed a hole is a patch a machine replays, not a paragraph. `script/falsifications`
counted 25 of 141 replayable before this sweep and 33 after, measured on this lane's own base. The
tree's number is 35 of 145, because milestone 212 (`script/falsifications` walks `crates/` only, so
the ratio it prints is not the tree's) landed between the two and widened the denominator; take the
merged figure rather than either branch's.

**Three more were rewritten and are recorded as *not* findings**, which is the half worth being
strict about. `filesystem_contract`'s two attenuation harnesses state their property through
`allows`, and `capability::a_deleted_capability_stays_deleted` states it through `get`. Each
looks like the class and is not: the sweep tried to exhibit a defect their original phrasing
misses and could not. `attenuate` does not call `allows`, and a widening `attenuate` shows up on
some input under every wrong `allows` tried; `delete` does not call `get`. They were made more
direct anyway, and their comments say plainly that no blind spot was demonstrated, because
recording a fix for a hole nobody found is the same one-step-from-evidence failure §134 exists
to end.

### What the mechanical narrowing did and did not do

The block asked for mechanical narrowing before reading 146 harnesses by hand, and it is worth
recording what that bought, because the answer is "a third of the way".

A script parsed every harness body, resolved each call against the functions defined in that
harness's crate, and flagged a harness where one function it calls appears inside another
function it calls. 23 candidates out of 146, and all 11 findings are among them, which
sounds like a win and is worth being suspicious of.

Three of the eleven, the `paging` leaf trio, were flagged by an artifact. The extractor
takes a function body by counting braces over a bounded window, so it sometimes swallows a
neighboring function, and that is why `entry_pa` appeared to call `leaf_entry`. It does not.
An encoder and its own decoder never call each other, so a *correct* call-graph test would have
missed all three, and the round trip between an encoder and its decoder is the same failure in
different clothes. The narrowing is therefore weaker than its hit rate suggests.

It is also noisy in the other direction: several candidates it flagged are legitimate agreement
claims, and `globally_unique_identifier_partition_table::crc32_matches_its_bitwise_definition`,
which is the model of a *good* harness, is one of them.

So the narrowing is a worklist, not a verdict, exactly as the block predicted. Every harness was
read.

### The patterns that make a harness fine, since most of them are

Grouped, because the reasons repeat and a reader deciding whether their own new harness is at
risk wants the pattern rather than 135 rows:

- **The claim is stated in arithmetic the implementation does not own.** `kernel`'s two run
  harnesses do it in `u128`, `device_tree_blob::be32_reads_big_endian_when_in_bounds` writes the
  four shifts out, `paging`'s `the_indices_and_offset_tile_the_address` reconstructs the address by
  hand. This is the shape every rewrite above converged on.
- Two independent implementations are compared on purpose.
  `globally_unique_identifier_partition_table::crc32_matches_its_bitwise_definition` is the
  clearest: a table-driven CRC against the bitwise definition, where a self-consistent wrong table
  is the exact defect it exists to catch. `capability::subset_matches_allows` and
  `calendar::the_calendar_algorithms_are_mutual_inverses` are the same move.
- The harness carries its own model. `intrusive_fifo` records push order in a local array and
  compares the queue against it; `generational_table` counts against a counter the walk does not
  use.
- The property is totality or termination, where there is no predicate to be blind to:
  `glob::matching_is_total`, `pci::the_capability_walk_terminates_on_any_device`, `elf`'s four.
- The assertion is a constant.
  `non_volatile_memory_express::prp_pair_is_total_and_page_disciplined`,
  `paging::x86_64::no_vtd_entry_ever_sets_a_reserved_bit`.

### A second shape, found along the way and not this milestone's

`nifefs::the_validation_implies_reads_slice_is_in_bounds` calls neither `parse` nor `read`. It
re-implements both functions' arithmetic inside the harness and proves a property of the copy. That
is not self-reference, it is a proof about code that is not the code, and it goes green however
`parse` is rewritten. Its doc comment is honest that it states the arithmetic ("Exactly parse's
acceptance condition", "Exactly read's slice arithmetic"), so this is a recorded design rather than
a mistake. Closing it means restructuring `nifefs` so the condition is a function both the parser
and the harness can call. Raised as proposed milestone 213 (a harness that re-implements the code
instead of calling it), a provisional number then: a harness that duplicates the implementation
instead of calling it.