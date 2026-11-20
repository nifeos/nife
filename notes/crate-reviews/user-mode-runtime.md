# Review package: `user_mode_runtime`

*Name: provisional, minted by lane 1112-accessors-package. The `user_mode_runtime` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to ratify the crate's names and to clear it for its move to its own
repository. That move is step 3 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md),
after the contract crates the crate links. It describes the crate as `main` plus this branch leaves it. The
outside review that produced the work is
[milestone 1112 (`user_mode_runtime`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1112-user-mode-runtime-outside-review.md).

## In one paragraph

What every nife program links: one syscall wrapper (`invoke`) and the calls built on it, the entry
macro, the panic handler macro, and typed, bounds-checked, alignment-checked windows over memory the
kernel handed a process (`MappedWindow`, `socket_frame::Frame`). It covers `aarch64`, `riscv64` and
`x86_64`. The crate is `no_std`, has about 2,700 lines of source in 14 files and 100 `unsafe` sites,
each of which a safe wrapper discharges once. A wrong argument to a safe function panics or meets a
refusal from the kernel; it never reads or writes memory the type did not promise. Window reads and
writes have one spelling, `read::<T>` and `write::<T>`, over the sealed `WindowInteger`. No model
checker covers the crate: it has no Kani harness, and the package says so rather than claiming a
proof. The names are not ratified. 24 of 122 carry a ratified block, 48 are provisional and 49 have
no block.

## Identity

| item | value |
|---|---|
| crate | `user_mode_runtime`, ratified (replaces `user_rt`: `rt` is an abbreviation, and `user` alone means a person in this tree) |
| identity values | `INTERFACE_ID` 20 (the entry registers and the link layout), `VERSION` 1. Both carry provisional `Name:` blocks |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88, both literals |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest. The crate adapts no source |
| `publish` | `false`, with the one-sentence reason in the manifest: milestone 1017 keeps moved crates unpublished under §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit) until their release milestone |
| dependencies | `abi`, `address_space_layout`, `current_cpu_contract` and `socket_contract` by path, `counter_frequency_contract` by path outside `aarch64`, and `clock_contract` and `entropy_contract` already by git `rev` |
| manifest metadata | description, keywords (`no-std`, `runtime`, `syscall`), categories (`no-std`, `embedded`, `os`), `readme`, and `[package.metadata.docs.rs]` for the three targets. The `repository` key waits for the move |
| size | 14 files under `src`, 2,689 lines |
| lints | literal `[lints.rust]` and `[lints.clippy]` tables, the same set `current_cpu_contract` carries |
| `link.ld` | delivered by the crate's `build.rs` through `OUT_DIR` and `cargo::rustc-link-search`; no program names a path into the crate |

## Who in nife depends on it

Every program. Each is a path dependency with `version = "0.1.0"`:

- `components`: 62 binaries, among them every program of `disk-tools`, `process-tools` and
  `core-tools`.
- `fixtures`: 72 binaries.
- The `*-unknown-nife` workspaces: `jig`, `mdr`, `std_exerciser`, `cryptography_exerciser` and
  `pinned_tls_exerciser`, `redoxfs_server`, `c_library`.
- The crates `memory_region_heap`, `process_builder`, `hot_swap_server` and `system_initializer`.
- The `kernel` names it only in a comment (`user_mode_runtime::counter_hertz`); it has no dependency.

In the order of milestone 1017 the crate leaves at step 3, after `abi`, `address_space_layout`,
`current_cpu_contract` and the contract crates it links, and before `disk-tools` (step 4) leaves,
because every program of that package depends on it. After the move each of those manifests pins the
new repository by a full commit.

Outside the dependency lines: `notes/user-mode-runtime.md` and its `user-mode-runtime/` directory
describe the crate and move into its `docs/` (the move lane's work, below). `helpers/` and
`packages/runtime.package.toml` list it, and `crates/externalization-list.txt` holds its row.

## What was run

Run in this branch's worktree on the toolchain pinned in `rust-toolchain.toml`, except the minimum
version rows.

| check | result |
|---|---|
| `cargo test -p user_mode_runtime` | 13 unit tests pass (the window's bounds, alignment and wrap panics, the round trip at every width, the clock arithmetic, the frame, the stub and the linker script). 9 doctests pass (eight `no_run` examples compile and one `should_panic` runs); the two that stay `ignore` (`process::program_entry` and `process::panic_handler`) say why in their text |
| `cargo clippy -p user_mode_runtime --all-targets -- -D warnings` | no warning |
| `cargo clippy -p user_mode_runtime --lib -- -D warnings` on `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none` | no warning. The bare targets take `--lib`: `x86_64-unknown-none` has no `test` crate |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p user_mode_runtime`, host and the same three targets | no warning. One unresolved link (`trap` in `counter_hertz`'s doc) predated this branch and is fixed |
| `cargo build -p components -p fixtures --target <each of the three>` | pass on all three |
| `redoxfs_server`: `cargo test` and `cargo clippy --all-targets -- -D warnings` | pass (86, 5, 2, 1 and 1 tests across its targets) |
| the call sites of the removed accessors | none remain: `grep` finds no `read_u8` through `write_u64` call on a window anywhere in `components`, `fixtures`, `crates`, `patches` or the notes. The three `DirectMemoryAccessRegion` impls in the `designware_ethernet` and `e1000e` transports keep their trait methods `read_u64` and `write_u64`, and call `read::<u64>` inside |
| MSRV: a copy of the crate and its path dependencies (five before `clock_contract` left) outside the tree, `cargo +1.88 build` | passes for the host; `--lib` passes for the three bare-metal targets |
| `python3 helpers/template_report_gates.py --check` | clean. The rows for `prose_acronyms` (23) and `readme_acronyms` (1) left `helpers/template_report_baseline.txt`; `abbreviations` (19) and `register_names` (2) stay |
| `python3 helpers/name_release.py --draft user_mode_runtime` | 122 rows: 24 ratified, 48 provisional, 49 with no block, 1 recorded (`_start`) |
| `script/crate-review-packet user_mode_runtime` | facts match this note: the public API, the 100 `unsafe` sites and the dependents above |
| `script/lint --quick` | passes all sections |
| `script/test` (host tests, the patched std build, kernel and system tests under QEMU 11.1.2 on `aarch64`, `riscv64` and `x86_64` including OVMF and AMD-Vi, the screen boot tours, the redoxfs image check) | passes, exit 0 |
| `script/crate-standalone user_mode_runtime` | 12 failures, all for the move lane (P1-8): five path dependencies (`abi`, `address_space_layout`, `clock_contract`, `current_cpu_contract`, `socket_contract`; `clock_contract` has since left for git), the 108 names not yet ratified, 97 citations of the nife tree, milestones and sections, one architect's name, and the build and proofs that follow from the path dependencies |
| `script/crate-move --dry-run --into <scratch> user_mode_runtime` | the copy, mutation cron, scaffold, name-block removal and patch checks pass, and the README prose caps and passive-voice checks pass after this pull request's README edit. It fails the build (the path dependencies), standalone, the citations and five mentions of nife, the same list as above, and prints the nife-side pull request: thirteen dependents repoint to the repository at the tag |

Not run, and so not claimed:

- Miri and Kani. The crate has no harness, and its `unsafe` is a trap instruction, a volatile
  access and a pointer made from an address, none of which Miri executes on a host. See Proofs.
- The stranger test, mutation, `cargo-semver-checks` (a first release has no baseline) and the
  template's workflows, which run in the repository.
- A build of `jig`, `mdr`, `std_exerciser`, `cryptography_exerciser`, `pinned_tls_exerciser` and
  `c_library` for the `*-unknown-nife` targets. They need `-Zbuild-std` against the patched
  standard library source, which `cargo xtask std-src` builds, and no call into the crate changed
  there: none of them called a sized accessor.

## Public API

| module | items |
|---|---|
| crate root | `VERSION`, `INTERFACE_ID`, `CycleMeaning` (re-exported from `abi`), the macros `program_entry!` and `panic_handler!` |
| `syscall` | `invoke`, `raw_syscall`; both `unsafe` |
| `process` | `exit`, `trap`, `yield_now`, `require`, and the entry symbol `_start` |
| `clock` | `now`, `counter_hertz`, `counter_hertz_checked`, `cycle_reading` with `CycleReading { count, meaning }`, `current_cpu`, `tick_hertz`, `monotonic_nanoseconds` |
| `inter_process_communication` | `send`, `receive`, `receive_capability`, `receive_message`, `receive_capability_message`, `receive_request`, `call`, `call_receiving`, `reply`, `reply_capability`, `send_capability`, `badge`, `reap`, `survey`, `survey_record`, `virtual_address_space_list`, the notification calls `notification_signal`, `notification_wait`, `notification_poll` and `notification_bind`, `timer_arm`, `timer_cancel`, `retype_sleeper`, `sleep_until`, and the types `Message`, `Received`, `Request`, `Reply` (with `slot`) and `Delivered` (with `decode` and `into_reply`) |
| `objects` | `map_page_frame`, `is_granted`, `retype_into_page_frame`, `retype_page_frame_run`, `slice_page_frame`, `retype_into_kernel_object`, `split_memory_region`, `destroy_memory_region`, `memory_region_usage`, `map_memory_region_page`, `revoke_page_frame`, `address_space_layout_into`, `thread_control_block_capability_insert`, `thread_control_block_configure`, `thread_control_block_start`, `thread_control_block_suspend`, `thread_control_block_resume`, `capability_delete` |
| `devices` | `interrupt_wait`, `interrupt_acknowledge`, `reboot`, and on `x86_64` `outb` and `inb` |
| `mapped_window` | `MappedWindow` with `new` (`unsafe`), `window`, `read::<T>`, `write::<T>`, `copy_within`, `copy_to_slice`, `copy_from_slice`, `as_slice` and `as_mut_slice` (both `unsafe`); the sealed trait `WindowInteger`; `PAGE_BYTES`; `doorbell_barrier` |
| `socket_frame` | `Frame` with `new` (`unsafe`), `set_destination`, `destination_network_address`, `destination_port`, `len`, `set_len`, `payload_write`, `payload_read` |
| `virtio` | `read_register`, `write_register`, `setup_queue`, `notify`, `doorbell_barrier`, `virtio_ring_barrier` |
| `initial_ramdisk` | `bytes` (`unsafe`), `INITIAL_RAMDISK_VIRTUAL_ADDRESS` (re-exported from `address_space_layout`) |
| `entropy` | `fill` |
| `child_stub` | `SEND_THEN_EXIT`, the machine code a child runs |

The one type from another crate in a public signature is `CycleMeaning`, from `abi`, which is a leaf
that leaves first. No nife capability type, spawn handoff or nife error enum appears. Return codes
are `i64` words: a negative value is an `abi::Error`, and the doc of each call says which.

What a stranger may find surprising:

- `read::<T>` and `write::<T>` are the only accessors. They take the sealed `WindowInteger` (`u8`,
  `u16`, `u32`, `u64` and arrays of them), check bounds and alignment, and panic on a misaligned or
  wrapping offset. The sized forms `read_u8` through `write_u64` were removed in this branch (a
  maintainer ruling, provisional until the first tag); adding them back later is compatible, and
  removing them after the tag is not.
- Nothing runs on the host except the window and frame types over a local buffer. Every other
  function traps to a kernel, so the examples are `no_run`.
- `trap` is the one way a program stops on a panic; the macro `panic_handler!` expands to a handler
  in the program because a library cannot own the one per binary.
- `now` returns ticks of a counter whose rate `counter_hertz` reports, not nanoseconds;
  `monotonic_nanoseconds` converts. On `x86_64` there is no architected rate, so the rate comes from
  a page the kernel fills, and a zeroed page is a panic in `counter_hertz` and a `None` in
  `counter_hertz_checked`.

## Proofs

None. The crate has no Kani harness and no falsification patch, and the template's `proofs` check
will report that in the repository. The surface that takes hostile input is small and is covered by
tests instead: the window's bounds, alignment, wrap and overlap tests, and the frame's clamp of a
reply length to the buffer and to the frame. A harness for `MappedWindow`'s offset arithmetic is the
natural first one, and the move lane or a later lane can add it with a falsification patch.

## Names

None of the names is ratified by this package, and this branch renamed none. The table gives
each public name's status as `script/names` reads it, and a proposal. Proposal "stands" means the
spelling meets `design/naming.md` as written and the block only needs the ratified status.

| item | status | proposal |
|---|---|---|
| crate `user_mode_runtime` | ratified | stands |
| modules `initial_ramdisk`, and the items `bytes`, `fill`, `interrupt_acknowledge`, `interrupt_wait`, `send_capability`, `as_slice`, `as_mut_slice`, `address_space_layout_into`, `destroy_memory_region`, `is_granted`, `map_memory_region_page`, `retype_into_kernel_object`, `retype_into_page_frame`, `revoke_page_frame`, `split_memory_region`, `thread_control_block_capability_insert`, `thread_control_block_configure`, `thread_control_block_start`, `notify`, `read_register`, `write_register`, `setup_queue` | ratified (24 with the crate) | stand |
| modules `child_stub`, `clock`, `devices`, `inter_process_communication`, `objects`, `process`, `socket_frame`, `syscall` | provisional | stand. The ratified `initial_ramdisk` and the contract crates set the pattern: a module named for the seam it holds |
| modules `entropy`, `mapped_window`, `virtio` | no block | stand; `virtio` is an accepted word, like `pci` |
| `INTERFACE_ID`, `VERSION` | provisional | stand: the names every contract crate carries |
| `CycleReading`, `cycle_reading`, `counter_hertz`, `counter_hertz_checked`, `tick_hertz` | provisional | stand: a rate ends in `_hertz` |
| `Message`, `Received`, `Request`, `Reply`, `Delivered`, `receive_message`, `receive_capability_message`, `receive_request`, `receive_capability`, `into_message`, `into_reply`, `decode`, `slot`, `virtual_address_space_list`, `survey`, `survey_record` | provisional or no block | stand. `receive` and its variants are one family in two shapes (the raw words and the decoded value), the result of milestone 1112's P2-15 |
| `MappedWindow`, `WindowInteger`, `PAGE_BYTES`, `new`, `window`, `read`, `write`, `copy_within`, `copy_to_slice`, `copy_from_slice`, `doorbell_barrier`, `virtio_ring_barrier` | provisional or no block | stand. `copy_within` keeps the standard library's argument order; `window` replaced `sub`, which is the standard library's subtraction |
| `Frame`, `set_destination`, `destination_network_address`, `destination_port`, `len`, `set_len`, `payload_read`, `payload_write` | provisional or no block | stand. `len` returns a header field of `u16` bytes and is not a collection's length; its doc and an `allow` say so |
| `send`, `reap`, `badge`, `call`, `call_receiving`, `reply`, `reply_capability`, `notification_signal`, `notification_wait`, `notification_poll`, `notification_bind`, `timer_arm`, `timer_cancel`, `retype_sleeper`, `sleep_until` | no block | stand. Each is the method it invokes, spelled out |
| `map_page_frame`, `slice_page_frame`, `memory_region_usage`, `retype_page_frame_run`, `capability_delete`, `thread_control_block_suspend`, `thread_control_block_resume` | provisional or no block | stand |
| `exit`, `trap`, `yield_now`, `require`, `program_entry`, `_start` | no block, provisional, recorded | stand. `require` halts on a mismatch that cannot be recovered; the kernel's `trust::require` has the same meaning. `_start` is the entry symbol every toolchain uses |
| `now`, `monotonic_nanoseconds`, `current_cpu`, `inb`, `outb`, `reboot`, `SEND_THEN_EXIT` | no block or provisional | stand. `now` returns ticks, which its doc says, and `counter_hertz` is its pair. A reader may prefer `counter_ticks`; the architect can rule it, and the cost is every program that reads the counter |
| `invoke`, `raw_syscall` | no block or provisional | stand |
| private: `Sealed`, `invoke6`, `trap6`, `linker_script` | provisional | not public; the move lane rewrites the blocks with the rest |

Two judgments in the table are the lane's and not a ruling: that every provisional name stands, and
the optional `counter_ticks` for `now`. The names the draft lists as provisional carry blocks that
read "provisional, minted by lane/1112-outside-review-fixes" and similar. A ratification replaces
each block with a ratified one, and that is the move lane's step once the architect rules.

## License

`license = "MIT OR Apache-2.0"`, both texts at the crate root. The crate adapts no source, so no
`AND` term follows the pair. Register and instruction names (`CNTVCT_EL0`, `rdtime`, `rdtsc`,
`syscall`) are architecture facts, and the crate copies no text from a manual or a kernel.

## What still blocks the move

| item | owner |
|---|---|
| Ratify the names above and replace the provisional `Name:` blocks with ratified ones. | architect, then the move lane |
| Move `notes/user-mode-runtime.md` and `notes/user-mode-runtime/` into the crate's `docs/`, and drop the externalized-crates ceiling to zero (milestone 1112 P1-8). The text cites 34 nife paths, 31 milestones, 31 sections and the architect's name in the sources and the manifest, which the standalone `citations` check lists. | move lane |
| Convert the four path dependencies (`abi`, `address_space_layout`, `current_cpu_contract`, `socket_contract`) and `counter_frequency_contract` to `git` and `rev` with `version = "0.1.0"`, once each has left. | move lane, after the contract crates |
| `helpers/template_report_baseline.txt` holds two rows (`abbreviations` 19, `register_names` 2) and `packages/runtime.package.toml` and `crates/externalization-list.txt` list the crate. | move lane |
| The template's scaffold findings (the pull request template names the architect, `CONTRIBUTING.md` cites a nife path, the commit hook and `deny.toml` mention nife) are listed in the runbook's `BUGS`. | move lane |
| Every dependent's manifest (the ones listed above) repoints to the new repository by a full commit. | move lane, step 3 of milestone 1017 |
| The one test that reads a path outside the crate, `linker_script`, reads `link.ld` inside it, so it moves unchanged. | none |
| Run the stranger test, the template's workflows and the first weekly mutation run in the new repository. | move lane |
| Add a Kani harness for the window's offset arithmetic, if the architect wants one before the tag. | later lane |

## Open questions

1. Whether `now` is renamed to a name that says it returns ticks. Recommendation: keep it. It
   carries a doc that says so, and the rename touches every program that times itself. The
   maintainer can decide it, and it stays two-way until the first tag.
2. Whether the crate wants a harness before the tag. Recommendation: no; the checks above cover the
   hostile-input surface, and the first tag does not depend on one.

## Recommendation

The names stand as built and wait for the architect to ratify them, and the package waits for the
architect to clear it. The crate has a literal manifest, a README for three targets, license files,
one way to read and write a window and no acronym left unexpanded at its first use. What remains is
the move lane's text work (P1-8) and the contract crates that leave first.

Ready for review.
