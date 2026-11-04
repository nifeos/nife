# Evidence for the 2026-09-24 audit

The code paths behind [the report](../2026-09-24-new-trust-boundaries.md), at the tree it read
(`origin/main` at `69be99617`, against the last audit's `8e4587056`). Line numbers are from that
commit; the fixes in the audit's own branch move some of them by a few lines.

## The automation

| Question | Answer | Where |
|---|---|---|
| What arms auto-merge | every open, non-draft PR against `main`, no author test | `helpers/merge-drain.sh` `queue()`; `helpers/queue-hold.sh` `holdable()` |
| Reviews required on `main` | `required_approving_review_count: 0` | `gh api repos/nifeos/nife/rules/branches/main` |
| Forking, auto-merge | `allow_forking: true`, `allow_auto_merge: true`, public | `gh api repos/nifeos/nife` |
| Fork workflow approval | `first_time_contributors` | `gh api repos/nifeos/nife/actions/permissions/fork-pr-contributor-approval` |
| Secrets in reach of a merge-group run | `TOOLCHAIN_BUMP_PAT` (repo), `AUTOMATION_APP_ID`, `AUTOMATION_APP_KEY` (organization) | `gh api .../actions/secrets`, `.../actions/organization-secrets` |
| Fork PRs ever | none; 991 by calef, 9 by dependabot | `gh pr list --state all --limit 1000 --json isCrossRepository,author` |
| `pull_request_target` anywhere | no | `grep -n pull_request_target .github/workflows/*.yml` |
| Untrusted text into `run:` | none; only `github.event.*.number`, `.sha`, `.before`, `.after`, `merge_group.head_ref` | grep of every `${{ }}` in `.github/workflows/` |
| PR text in scripts | `title`, `body`, `headRefName` via `jq -r`; `body` only to `sed -n` for `Blocked-by: #N`; `head` only as a quoted `--branch` argument | `merge-drain.sh` (`blocked_by`, now `nife_blocked_by` in `helpers/blocked-by.sh`, and `stuck_checks`) |
| `curl \| sh` | rustup's installer, `--proto '=https' --tlsv1.2 -sSf` | `script/bootstrap` (its "Rust toolchain" section) |
| Committed bytecode | `scripts/__pycache__/name_provenance.cpython-314.pyc`, added 2026-09-18 in `bc4ba1d47` | `git ls-files -ci --exclude-standard` |

The fork approval policy makes a first pull request wait for a click; the drain then merges it
if green; the second pull request from that account runs its checks without a click. GitHub's
documentation says a fork's `pull_request` run gets a read-only token and no secrets, and says
nothing about restricting a `merge_group` run, whose ref is a branch in this repository. The
merge-group step was read, not exercised.

## The parsers

| Crate | Caller and trust boundary | Total over hostile bytes? | Tests |
|---|---|---|---|
| `package_archive` | none on target yet; a fuzz target and host tests | yes: `parse` bounds count (`MAX_MEMBERS`), table, and every `offset + len` in `usize` with `checked_add` (`Package::parse` in `lib.rs`) | doc tests, `fuzz/fuzz_targets/package_archive_roundtrip.rs` |
| `device_tree_blob` | kernel boot on aarch64/riscv64 (`device_tree` in `kernel/src/main.rs`, now `kernel/src/boot.rs`, and `init` in `kernel/src/memory.rs`), `uefi_loader` riscv64, `machine_discovery`; firmware supplies the blob | over a slice, yes: `be32`/`be64` are `checked_add` + `get`, depth-bounded walkers, `RegionOverflow`; **`from_ptr` believed `totalsize` (`DeviceTreeBlob::from_ptr` in `lib.rs`)**, fixed | 30 host tests in `tests/hostile.rs`, 4 Kani harnesses with falsifications, `fuzz_targets/device_tree_blob_walk.rs` |
| `firmware_configuration` | `drivers/ramfb.rs` (its `use firmware_configuration`), kernel, before the MMU; QEMU's `fw_cfg` DMA | yes: 64-byte `try_into`, prefix guard, `size` unused; walk bounded by `MAX_ENTRIES`, poll by `MAX_POLLS` | 8 host tests |
| `portable_executable` | `xtask/src/stick.rs` (`payload_riscv64`), host, over the ELF xtask just linked | no: `phoff + 4`, `vaddr + memsz`, `PT_DYNAMIC` unchecked, `vec![0; image_end]`, `DT_RELAENT = 0` loop (all in `from_elf` in `lib.rs`, now `xtask/src/portable_executable.rs`) | 4 host tests; none for truncation or overflow |
| `file_allocation_table` | `components/src/installer.rs` (`install`); inputs are constants, the disk size and the kernel's own boot file length | it is a writer; `Volume::new` refuses `> u32::MAX`, `checked_sub` on reserved and FAT sectors, `sector()` bounds index and length | 11 host tests, one ignored integration test |

## The kernel state

| Claim | Evidence |
|---|---|
| `SURVEY` needs `ENUMERATE`, refuses unknown records before the walk, takes any cursor, passes no pointer, zeroes outputs on `DONE` | the `SURVEY` arm in `syscall.rs` (`rendezvous_survey` in `kernel/src/syscall/rendezvous.rs` now); `survey_supervised` in `sched.rs` (the record check before the lock, the lock, the zeroing on `DONE`); `iter_from` in `generational_table/src/lib.rs`; `is_known` in `abi/src/lib.rs` |
| Domain membership is exact and proved | `survey_includes` in `capability/src/lib.rs` and its falsification patch |
| `CPU_TICKS` is zeroed on slot fill | `insert_at_in_place`, `insert_at` and `clear_cpu_ticks` in `sched.rs` |
| Nothing pins a thread from userspace | writers of `Thread::placement` are `spawn_on` (in `sched.rs`) and `start_thread_control_block` (in `sched.rs` too), both from `pick_spawn_target()` |
| `PLACEMENT` leaks a run-queue comparison | `pick_spawn_target` in `sched.rs`; the denial in the `SURVEY` arm of `syscall.rs` |
| Port revoke is synchronous on `x86_64` | `delete_port_range_caps_impl` in `sched.rs` under `IPC_TABLES`; `revoke_port_grant_everywhere` in `segments.rs`; `broadcast` in `mmu.rs`, an NMI broadcast with ack spin; handler `serve_shootdown_nmi` |
| The take-back reset the invoker's own core | `delete_port_range_caps_impl` in `sched.rs` spares the keeper's table; `revoke_port_grant_everywhere` in `segments.rs` reset locally regardless; `revoke_installed_port_grant_on` resets when the installed grant matches |
| Page revoke flushes synchronously on all three | aarch64 `flush_tlb` in `mmu.rs` (`tlbi vaae1is; dsb ish`); riscv64 `flush_tlb` in `mmu.rs` + `sbi_remote_sfence_vma` in `mod.rs`, now `remote_fence.rs` (SBI remote fence, honest `BUGS` in its documentation); x86_64 `flush_tlb` in `mmu.rs` |
| The in-flight `MAP` race §13 (capability revocation and untyped reclamation) deferred is open and recorded | `page_frame_map` in `syscall.rs` (PTE before log); `revoke_page_frame` in `revoke.rs` |
| FP is eager, scrubbed, never enable-driven | `move_the_register_file` in `fp.rs` (four cases) and the module documentation's "has this thread ever wanted FP" paragraph; `fp_state_of` in `thread.rs`; per-core disable in `init` and `adopt_secondary_idle` in `sched.rs`; `INITIAL` on both inserts, `insert_at_in_place` and `insert_at` in `sched.rs`; first use `enable_for_current` in `fp.rs` |
| The trap arm cannot be preempted between enable and mark-live | x86 interrupt gates (`interrupt_gate` and `init` in `exceptions.rs`); riscv64 patches the frame's `FS` (`riscv_trap_body` in `exceptions.rs`) |
| The kernel emits no FP | targets `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`, `x86_64-unknown-none` with `-sse,+soft-float` (`TARGET`, `RISCV_TARGET` and `X86_TARGET` in `xtask/src/main.rs`) |
| SVE/SME and V were not disabled | `arch/aarch64/fp.rs` (module documentation's SVE and SME paragraph, `FpState`); `arch/riscv64/fp.rs` (`FpState`); fixed in `init()` on both |
| `x86_64` cannot hold `ymm`/`zmm` state | `CR4.OSXSAVE` never set (no `xsetbv` in the tree), so VEX/EVEX `#UD` |
| The two window-added revocation tests are single-core page-table walks | `a_capability_revoked_while_it_is_in_flight_does_not_reach_the_receiver` in `revocation_in_flight_tests.rs`; `a_page_the_kernel_wired_is_unmapped_when_its_frame_is_revoked` in `spawn_mapping_revocation_tests.rs` |
