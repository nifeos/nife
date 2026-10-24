# `pmap`, and `ENUMERATE` extended to the address-space object

*Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `pmap-listing`.*

An appendix to [notes/process-view.md](../process-view.md), which summarizes it. The pricing of
`pmap` from the prompt is a separate page, [pmap.md](pmap.md).

## The method

`pmap` works on both ISAs, over `abi::address_space::LIST`: `Rendezvous::SURVEY`'s split one object type
over. `abi::aspace::MAP_INTO` needs `WRITE`, the authority to shape a space; `LIST` needs
`ENUMERATE`, the authority to look. A capability holding `ENUMERATE` alone lists every mapping and
cannot make one. `kernel::user::pmap_tests` proves that in both directions, with the discipline
`survey_tests` established. A viewer is refused `MAP_INTO`, and a builder is refused `LIST`. An
empty space answers rather than refusing, and an empty slot answers `NoSuchSlot`.

The listing reads the space's own revocation log rather than walking page tables.
`kernel/src/revoke.rs` already keeps one entry per mapped page, per space, for reclamation (§13 (capability revocation and untyped reclamation)).
So `LIST` costs nothing that log did not already pay for, and the answer cannot drift out of
agreement with what a real revoke would find. `revoke::list_mapping` hands back a `va`. The syscall
handler turns it into a permission with `arch::mmu::translate_at(root, va)`, present on both
architectures already and previously reachable only from revocation's own tests. `kind` reuses
`abi::aspace::MAP_RO`/`MAP_RW`/`MAP_CODE`, the same three words `MAP_INTO`'s mode argument takes.
A second vocabulary would have been invented for what is, read back, the same fact about the same
page.

## The delegation audit that DECISIONS §114 required

§114 (`pmap` gets its listing) required this audit. Every site that mints an
`Object::AddressSpace` capability was found and checked:
`components/src/builder.rs`, `crates/supervision_protocol::build_child_space`,
`fixtures/src/hello.rs`'s `aspace_builder`, `fixtures/src/os_primitives_benchmarker.rs`'s
`spawn_one`, and `kernel/src/bench.rs`'s `map_el0` harness. Every one retypes and maps. Every one
except `hello.rs`'s deliberately-unconfigured demo consumes the capability at `ThreadControlBlock::CONFIGURE`, all
inside the one thread that started it. None delegates an `Object::AddressSpace` capability to a
different program.

So the caveat's feared case has no instance in the shipped tree today. That case was a holder
nobody assessed for `ENUMERATE` gaining a real power the day the method starts consulting the bit.
This is the audit's actual finding rather than an assumption: there was nothing to narrow.

## What building it found, which the fork's wording did not anticipate

`ThreadControlBlock::CONFIGURE` does not only consume the caller's capability. It removes the space's entry from
the registry that `user_address_space_root` (and so `LIST`) resolves through (`take_user_address_space`). So the
instant a space is bound to a thread, every capability that ever named it reads as an empty listing
rather than a live one. That includes a capability already sitting in some other program's
capability table. `kernel::user::pmap_tests::a_capability_outliving_its_space_reads_as_empty`
proves this directly. It calls `take_user_address_space` the way `thread_control_block_configure` does, and shows `LIST`
answers `DONE`.

Combine that with the delegation-audit finding above: nothing hands an `Object::AddressSpace`
capability to a second program at all. The practical consequence is that no address space anywhere
in this system can today be handed, as a live view, to a program other than its own builder.

`ps` reaches the interactive shell because `Manifest::domain` tells `system_initializer` which live
supervision endpoint to place in a child's capability table. That endpoint is `deaths`, which
persists for a thread's whole life. Nothing plays that role for an address space. Nothing survives
long enough, held by anyone but its builder, to be worth wiring a manifest field to. `pmap`'s kernel
mechanism and its program are real, and proven end to end against a genuine `Object::AddressSpace`
(`kernel::user::pmap_tests`). That is the same way `ps`'s `survey_tests` prove `SURVEY` without
going through the shell. But unlike `ps`, that is the only place `pmap` runs today.

This is not the lane's decision to make, and it was not decided here. A fix would need a builder to
hand a narrowed, still-registered view of a space it is constructing to a third party, *before*
`CONFIGURE` consumes its own copy. That changes how spawning works rather than how `pmap` works. It
is named as an open finding in the `BUGS` of `design/roadmap/0126-who-else-is-running.md`, and in
`crates/pmap`'s own, for whoever picks it up.
