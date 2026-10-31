---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Ratify the three names that block the NVMe crate's move

Raised 2026-10-31 (UTC) by the lane for milestone 814 (`non_volatile_memory_express`, released on its own). The lane re-derived the state and found one blocker ahead of every later stage: the crate cannot leave for its own repository while three of its names are provisional.

## Background

A crate that moves to its own repository must carry only ratified names, because a name is permanent on first publication. `script/crate-standalone non_volatile_memory_express` checks this first, and it fails today on exactly three names: the type `Namespace`, and its two methods `holds_block` and `transfer_command`. All three were minted on 2026-10-26 (UTC) when `namespace_holds_block` and the free function `transfer_command` became methods (ratified in their free-function form on #2749). A lane may not ratify a name, so the move waits for calef.

The crate had five provisional names that morning. This lane removed two without renaming anything: `THE_CONTROLLER_CONFIGURATION_FIELDS_SHARE_NO_BIT` and `THE_QUEUE_DWORD_FIELDS_SHARE_NO_BIT` are compile-time assertions that two bit fields do not overlap. Nothing outside the crate uses them, and a stranger's API listing gains nothing from them, so they are now private. A private item is not part of the published names.

## Recommendation

Ratify all three as they stand:

| name | kind | what it says |
|---|---|---|
| `Namespace` | struct | a namespace's capacity in bytes and its logical blocks per transfer: what the bounds check and the transfer builder need |
| `Namespace::holds_block` | method | is this whole block inside the namespace |
| `Namespace::transfer_command` | method | the read or write command for one whole block, or `None` |

The type is the NVMe specification's own word. Nothing in it is abbreviated. The methods keep the verbs calef ratified for the free functions on #2749, with the namespace now the receiver.

One loose end for the same sitting: the struct's field is `size_bytes`, while `IdentifyNamespace` calls the same quantity `capacity_bytes` (ratified 2026-10-25, #2720). Recommend renaming the field to `capacity_bytes`, so one quantity has one word. The cost is a rename in the crate and its callers, before anything is published; after publication it would be a breaking change.

## Prior art

- Linux calls the structure `nvme_ns` and keeps `nsze` and `ncap` for the sizes, the specification's abbreviations. Its checks sit in free functions over a request (`nvme_setup_rw`).
- SPDK has `spdk_nvme_ns` with `spdk_nvme_ns_get_size` and `spdk_nvme_ns_cmd_read`: the namespace is the receiver of the question and the command builder is a free function taking it.
- Rust's own convention for a membership question is `contains` (`Range::contains`, `str::contains`). `holds_block` is the nearer word to the specification's "namespace holds blocks", and `contains_block` is the standard-library spelling; either reads clearly.

Neither spells the type differently. `Namespace` matches both.

## Where nife differs

Those systems expose the controller's raw fields. This crate gives the caller a value that already knows its bounds, so a driver cannot build a command for a block the namespace lacks. That property is a Kani harness (`a_transfer_command_is_only_built_for_a_block_the_namespace_has`), and it is the reason the bounds check is a method on the value rather than a loose function.

## Example

```rust
use non_volatile_memory_express::Namespace;

let namespace = Namespace { size_bytes: 8 << 20, blocks_per_transfer: 8 };
assert!(namespace.holds_block(2047, 4096));
assert!(!namespace.holds_block(2048, 4096));
let command = namespace.transfer_command(7, 1, 2047, 4096, 0x4001_5000, 4096, false);
assert!(command.is_some());
```

## What a yes unlocks

The ratifying pull request carries the table `python3 helpers/name_release.py --draft non_volatile_memory_express` prints and sets the three blocks to `ratified <date> (#PR)`. Then `script/crate-standalone non_volatile_memory_express` exits 0, which is the milestone's precondition for the move. Creating `nifeos/non_volatile_memory_express` is a separate step that also needs calef, and so is every publication.

## Alternatives

- Ratify with the field renamed to `capacity_bytes` (recommended above).
- Rename `holds_block` to `contains_block` before ratifying. Cheap now, a breaking change later.
- Refuse `Namespace` and keep the free functions. This loses the receiver that makes the Kani property read naturally.
