# The `Aspace` rename, as built

An appendix to [milestone 158 (build the eleven kernel object and identifier
renames)](../0158-kernel-object-rename-build.md). It holds the lane's account of `Aspace` and
`FreeVas` becoming `AddressSpace` and `FreeAddressSpace`: its real surface, the near-collision it
checked, its scope call, the bug it caught in itself and how it was verified.

## The surface

`Aspace`/`AddressSpace`, plus its decided companion `FreeVas`/`FreeAddressSpace`
(`kernel/src/thread.rs`). It was done to completion in its own lane, per the block's sequencing rule.

The block's own table estimated 9 files and 17 occurrences for the bare `Aspace` type token. A fresh
count against the same measure, before starting, found 18 occurrences across 9 files. That was
case-sensitive `Aspace` alone, and it confirmed the table's narrow count was accurate for what it
measured.

The real surface, once every case form and every compound identifier was counted, was far larger.
It followed the pattern `Endpoint` set: 391 case-insensitive occurrences of "aspace" across 37 `.rs`
files, not 17 across 9, because the type name was the smallest part of the surface. It touched:

- a whole `abi::aspace` -> `abi::address_space` ABI module. This is
  `MAP_INTO`/`MAP_RO`/`MAP_RW`/`MAP_CODE`/`LIST`, the method-number namespace every user program and
  `kernel/src/syscall.rs`'s dispatch use;
- `objtype::ASPACE` -> `objtype::ADDRESS_SPACE`;
- the `Object::Aspace` capability variant -> `Object::AddressSpace`;
- a family of lowercase kernel functions: `aspace_cap` -> `address_space_cap`, `user_aspace_create`
  -> `user_address_space_create`, `user_aspace_map` -> `user_address_space_map`,
  `user_aspace_root` -> `user_address_space_root`, `take_user_aspace` -> `take_user_address_space`,
  `reap_aspaces_in_region` -> `reap_address_spaces_in_region`, `readopt_user_aspace` ->
  `readopt_user_address_space`, and `aspace_list` -> `address_space_list`;
- a whole kernel test-infra module file, `kernel/src/user/aspace_service.rs` ->
  `kernel/src/user/address_space_service.rs` (no longer in the tree). Its
  `aspace_builder`/`ASPACE_BUILDER` sibling in `fixtures/src/hello.rs` became
  `address_space_builder`/`ADDRESS_SPACE_BUILDER` to match;
- a lock-rank constant, `kernel/src/sync.rs`'s `ASPACES` -> `ADDRESS_SPACES`;
- per-program constants: `components/src/pmap.rs`'s `ASPACE_SLOT` -> `ADDRESS_SPACE_SLOT`, and
  `fixtures/src/os_primitives_benchmarker.rs`'s `MAP_ASPACE` -> `MAP_ADDRESS_SPACE`.

The `FreeVas` companion is `kernel/src/thread.rs`'s stack-VA-range free list. It is an unrelated
subsystem reached by the same abbreviation, per DECISIONS §113 (eleven kernel object and identifier names move to the plain, standard term). It was 10 occurrences across 3
files (`thread.rs`, `stack.rs`, `sched.rs`). The lane renamed `struct FreeVas` -> `struct
FreeAddressSpace` and `static FREE_STACK_VAS` -> `FREE_STACK_ADDRESS_SPACE`. It left the struct's
private `vas: [u64; 128]` field alone. That field holds individual freed stack virtual addresses,
not address-space objects, and §113 named only the struct and the static, not this field.

## A near-collision, checked and resolved

This is a real near-collision, checked and resolved rather than treated as a blocker.
`kernel/src/user.rs` already defines `pub struct AddressSpace`, predating this rename. It is the
kernel-internal page-table object a process's memory actually is, with its own `Drop`,
`map_physical`, and so on.

Renaming `Object::Aspace` to `Object::AddressSpace` gives the capability variant the same name as
the object it names. That is not a collision. It is the same pattern this tree already uses for
`Object::Rendezvous(RendezvousId)` naming `struct Rendezvous` in `sched.rs`. A capability variant
and the kernel struct it points at sharing a name, in different modules, is how this tree already
reads "a capability that names one of these." No import collision results, since the variant holds
a `u64`, not the struct. The pairing is arguably more consistent post-rename than
`Aspace`/`AddressSpace` were pre-rename.

## The scope call

It is scoped the same way as `Endpoint`. The rename covers the identifier surface throughout: types,
functions, constants, module paths, struct fields, module file names, and backtick-quoted
doc-comment citations of those identifiers.

Local variable and parameter names that merely *spell* the concept were left alone: `aspace`,
`aspace_name`, `aspace_slot`, `b_aspace`, `as_region`. This matches the precedent's own treatment of
`ep`, never renamed to `rdv` when `Endpoint` became `Rendezvous`. A local binding is not in the
milestone's own listed scope, however it happens to be spelled.

Prose (comments, panic and `.expect` message strings) was converted from "aspace" to "address
space" throughout. That is a broader sweep than `Endpoint`'s, which left much of
`kernel/src/user.rs`'s and the test files' prose alone. "aspace" is not standard English the way
"endpoint" is. Leaving it anywhere the rename touched would have left the exact contraction
DECISIONS §113 is retiring sitting in prose next to its own renamed identifier.

`notes/abi.md`, `notes/frames.md`, `notes/object-revocation.md`, `notes/process-view.md` and
`notes/thread-spawn-fork.md` were updated for the same reason `notes/ipc-naming.md` was. Two
genuinely historical citations were deliberately left alone:

- `crates/abi/src/lib.rs`'s crate-naming-ratification note ("...the actual test (does the architect
  have to ask what this means, which sank `Tcb`/`Aspace`/ `Untyped`)...", 2026-08-23). It describes
  the state of the tree at the moment that test was applied.
- `design/decisions/0114-aspace-enumerate.md`, a DECIDED record whose own text already
  parenthetically notes the pending rename. It was left as the point-in-time record it is. That
  matches how the `Endpoint` rename left
  `0026-fault-endpoint.md`/`0041-endpoint-as-broker.md`/`0091-endpoints-before-the-refusal.md`
  untouched.

## A self-inflicted bug, caught by the build

There was one self-inflicted bug during the build. The crate's own build caught it, and it was fixed
before any commit existed to hide it.

A first attempt at the prose sweep used a single blanket word-boundary substitution. It did not
distinguish comments and string literals from live code. It briefly turned every local `aspace`
binding (`let aspace = ...`, `aspace_slot` parameters, `aspace.rights` field access) into the
syntactically invalid `let address space = ...`. `cargo check` on every touched crate caught all of
it before anything was committed.

It is recorded because it is exactly the kind of mistake the ladder in this tree's own conventions
expects a gate to catch rather than a person to have avoided, and it did.

## Verified

The same way `Endpoint` was:

- `script/lint` clean;
- `cargo test --workspace` (excluding the bare-metal `kernel`/`user`/`fs_server` targets) green,
  142 test-suite runs including every doctest;
- the kernel builds clean for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`. The last is new since `Endpoint`'s rename: milestone 161 (the x86_64 kernel port) item 4 gave x86_64
  a real kernel test leg;
- `cargo xtask swish-check` green on both aarch64 and riscv64;
- the two Kani-proof-bearing crates that touch the renamed `abi::address_space` module both verify
  successfully post-rename: `crates/capability` (12 harnesses) and `crates/component_plan` (5
  harnesses);
- the full `script/test` suite green on aarch64, riscv64 and x86_64.
