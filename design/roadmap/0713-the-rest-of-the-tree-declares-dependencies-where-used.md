---
status: NOT-STARTED
raised: 2026-10-03
promoted_from: the-rest-of-the-tree-declares-dependencies-where-used
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 713. The rest of the tree declares its dependencies where they are used

*Title and slug are drafts.*
Raised by milestone 631 (the kernel declares each dependency where it is used), which covered the
kernel only.
The same `cargo::unused_dependencies` lint (new in `nightly-2026-10-02`, warning-only) fires on two
other manifests during `script/lint`'s clippy passes:

| Manifest | Dependency | Pass that warns |
|---|---|---|
| `crates/user_mode_runtime/Cargo.toml` | `counter_frequency_contract` | `clippy: kernel + user + user_mode_runtime (aarch64)`. Its own comment says only `cntfrq`'s x86_64 arm reads it and that it is unconditional on purpose. |
| `redoxfs_server/Cargo.toml` | `capability_manifest_note`, `address_space_map`, `globally_unique_identifier_partition_table`, `entropy_contract` | `clippy: redoxfs_server (its own workspace)`, which is a host build. Whether these are unused everywhere or only on the host was not checked. |

Nothing fails: the lint is warning-only and `-D warnings` does not promote it. The work is to
decide each one the way milestone 631 did. Use a target table where the split is by architecture.
Remove the dependency where it is dead. Add a `BUGS` entry beside it where the warning is the
cheaper truth. `user_mode_runtime`'s comment already argues for keeping it unconditional, so that one may
end as a recorded exception rather than a move.

## Index row

`cargo::unused_dependencies` warns on `user_mode_runtime` and `redoxfs_server`, outside the kernel that milestone 631 covered. Proposed: declare those dependencies where they are used.
