# P2: later

An appendix to [the milestone](../1086-domain-name-resolution-contract-outside-review.md).

## P2: after the first tag

12. `src/wire.rs` carries the spawner's half into the standard library. Finding: the module gets copied whole into the std platform layer, and the std client uses its client half only
    (`request`, the two client operations, the offsets, the two limits, `Outcome`, `status`,
    `network_step`). `control_word`, `control_fields`, `OPERATION_GRANT`, `OPERATION_FORGET`,
    `GRANTS_MAXIMUM` and `is_canonical_zone` ride along unused. Change: split the file into the
    client's page and words (the copy) and the spawner's control words. Or leave it and say in the
    module's head which half the copy is for. Door: two-way (the module paths are re-exported at the
    root). Effort: S.

13. The root zone has three spellings, and two tiers of zone spelling have no map between them.
    Finding: on the wire the root is the empty text (`src/messages.rs` line 9). `installed_programs`
    spells it `"."` (`crates/installed_programs/src/grants.rs` line 110) and converts.
    `is_canonical_zone` refuses `"."` and accepts `""` (`src/wire.rs` lines 40 to 64).
    `Name::from_dotted` accepts both. Separately, a canonical zone is lowercase, without `_` and
    without a trailing dot, while the grant table accepts uppercase, `_` and a trailing dot, so a
    zone the table grants may be one the manifest refuses. Both facts are by design, and two different crates state them. Change: the contract owns `ROOT_ZONE: &str = ""`, and the README states
    the two tiers in one paragraph: what a configuration may write, and what the wire accepts. Door:
    two-way (additive). Effort: S.

14. Small shapes. Finding: `Grants` and `Entry` derive no `Debug`, so a test failure prints
    nothing of the table. `Control::Granted(usize)` and `Forgotten(usize)` carry a bare window
    index. `grant_messages`'s `zone.get(start..).unwrap_or(&[])` (`src/messages.rs` line 20) cannot
    miss, since `start` is always below the length. `port_and_transport` accepts any `transport`
    value, and the resolver reads anything that is not `TRANSPORT_TCP` as UDP
    (`components/src/domain_name_resolver.rs` line 133). The size of `Grants` is about 4,160 bytes
    by hand (eight entries of a 256-byte `Name`, a 254-byte text buffer and the small fields),
    computed, not measured. Change: derive `Debug` (printing the dotted zone, as `Name` does). A
    `Window` newtype or a doc sentence that the index is the window. Drop the dead branch. Have
    `port_and_transport_fields` refuse a third transport or the resolver refuse it at start. A size
    test that states the figure. Door: two-way. Effort: S.
