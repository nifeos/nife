# Review package: `test_and_set_lock`

*Name: provisional, minted by lane 1109-test-and-set-lock. The `test_and_set_lock` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to clear the crate for its move to its own repository, as part of
[milestone 1109 (what an outside maintainer would fix before it leaves)](../../design/roadmap/1109-spin-lock-outside-review.md).
It describes the crate as `main` plus this branch leaves it. This package ratifies nothing and
clears nothing.

## In one paragraph

A test-and-set spinlock for `no_std` code: one flag, taken with an Acquire compare-exchange and
released with a Release store, and a guard that hands out the data while it holds the flag. The
source is written so that loom runs the real lock, not a copy of it, under every interleaving of two
and three threads. It is not fair, not reentrant, does not poison and does not mask interrupts. The
kernel's `InterruptSafeMutex` wraps it and adds the interrupt masking and the lock ranks. The crate
is `no_std`, has no run-time dependency, and holds four `unsafe` sites in the library: the `Sync`
impl, the guard's `Deref` and `DerefMut`, and `force_unlock`. It has no Kani harness: its proof is
five loom models. Nothing in its manifest points back into nife.

## Identity

| item | value |
|---|---|
| crate | `test_and_set_lock`, named by the architect's ruling on milestone 1109's "The name the crate leaves under" (option B, 2026-11-19 UTC), renamed from `spin_lock` before the move so the repository, the package and the registry agree. crates.io holds `spin-lock`, which it reads as the same name as `spin_lock`, and holds nothing under the new name |
| directory | `crates/test_and_set_lock` |
| identity values | none: no `INTERFACE_ID`, `VERSION` or `MAGIC`; the lock is an in-process type, not a wire layout |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest |
| `publish` | `false`; the standing hold on publishing applies |
| dependencies | none at run time; `loom = "0.7"` only under `cfg(loom)` |
| size | one `src/lib.rs` of 598 lines: the library, its host tests and the loom models |

## Who in nife depends on it

Each by a path dependency with `version = "0.1.0"`, except the litmus tool, which has a path only.

| package | manifest | table |
|---|---|---|
| `kernel` | `kernel/Cargo.toml` | dependencies. `kernel/src/sync.rs` wraps `test_and_set_lock::SpinLock` in `InterruptSafeMutex`, and `kernel/src/user/fs_service/two_dirs.rs` holds one static |
| `system_tests` | `system_tests/Cargo.toml` | dependencies. Six statics in `system_tests/src/user/`: `pipeline_tests.rs`, `pipeline_service.rs`, `confined_fuzzer_tests.rs`, `redirection_tests.rs` and two in `syscall_fuzzer_tests.rs` |
| `host_reordering_litmus` | `tools/host_reordering_litmus/Cargo.toml` | dependencies. It runs the real lock on a bare-metal guest beside its own copy, and `weaken-spin.sh` edits the crate's source in place |

The tooling that names the crate: `script/interleaving-check` and `.github/workflows/ci.yml` (the
loom models), `.cargo/mutants.toml`, `packages/kernel.package.toml` (its `interfaces` list),
`helpers/trust_boundary_crates.tsv`, `helpers/template_report_baseline.txt` (three rows), the
mutation triage and survivor rows, and the notes that describe the kernel's lock. The standard
library overlay does not name it. Four design records name the old spelling and were left alone:
`design/decisions/0011-per-cpu-run-queues.md` and `design/decisions/0046-dependency-rule.md` cite
`crates/spin_lock`, and `0009-irq-safe-locking.md` names the Linux call `spin_lock_irqsave`.
Section 11 and section 46 want a pointer to the new path; this lane did not edit
`design/decisions/`.

What references back into nife from the crate: the module comment and `Cargo.toml` cite
`notes/interleaving.md`, `notes/memory-ordering.md`, `script/interleaving-check` and
`kernel/src/sync.rs` (6 lines, first at `src/lib.rs` line 8), and the source carries 8 `Name:`
blocks. Both are move work. The README, the two license files and the manifest literals stand.

## What was run

Results come from this branch, on a Linux host.

| check | result |
|---|---|
| `cargo test -p test_and_set_lock` | 7 host tests pass, plus 1 doctest (the README's example is the `SpinLock` doctest) |
| `RUSTFLAGS="--cfg loom -D warnings" cargo test --release -p test_and_set_lock --lib` | 5 loom models pass, in 0.01 s after the build |
| `cargo miri test -p test_and_set_lock` | the doctest runs under Miri and passes. The host tests and loom models were not run under Miri |
| `cargo clippy -p test_and_set_lock --all-targets -- -D warnings` | no warning |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p test_and_set_lock --no-deps` | no warning |
| `cargo check -p test_and_set_lock -p kernel` for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`, `x86_64-unknown-none` | all build. The kernel's only warnings are dead-code warnings under `kernel/src/arch/x86_64/`, which this rename does not touch |
| `cargo check -p system_tests` for `aarch64-unknown-none-softfloat` | builds |
| `cargo check` in `tools/host_reordering_litmus` for the aarch64 and riscv64 bare-metal targets | both build; the tool refuses any other target by design |
| `script/crate-standalone test_and_set_lock` | three failures, all move work. `names`: `get_mut` and `into_inner` are provisional. `citations`: 6 lines cite paths in this tree. `proofs`: no Kani harness, because the proof is loom's. The build passes on 1.88 for the host and the three bare-metal targets, which is the MSRV build in a copy outside the tree |
| `script/crate-review-packet test_and_set_lock` | the packet this package follows. It lists 17 public items, no type from another crate in a public signature, and 7 `unsafe` sites of which 4 are library code and 3 are in tests or one test helper |
| `script/names test_and_set_lock` | the tree's name index has no row for the new crate name. The `Name:` block records the ruling |

| `script/crate-move test_and_set_lock --dry-run --into <scratch>` | exits 1 with three findings for the move lane: the standalone check and the citations check each report the 6 lines that cite paths in this tree, and the passive-voice check reports 6 passives in the README. It stages the repository in the scratch directory, creates and pushes nothing, and prints the three dependent manifests' `git` and `rev` lines |


Not run, and so not claimed: the stranger test, mutation, `cargo-semver-checks` (a first release has
no baseline), Kani (the crate has no harness), the QEMU gates (`script/test`, `script/boot-check`
and the like: the change is a package rename, the kernel and `system_tests` build for all three
targets, and the heavy-job slots on this host were held for most of the session), and the
template's workflows, which run in the repository. The queue runs the QEMU gates on this pull
request.

## Public API

| kind | items |
|---|---|
| the lock | `SpinLock<T: ?Sized>`, `const fn new(value: T)` (a plain `fn` under `cfg(loom)`), `lock -> SpinLockGuard`, `try_lock -> Option<SpinLockGuard>`, `unsafe fn force_unlock`, `into_inner`, `get_mut` |
| traits on the lock | `Sync` (for `T: Send`), `Default`, `From<T>`, `Debug` (prints `<locked>` through `try_lock` rather than spin) |
| the guard | `SpinLockGuard`, with `Deref`, `DerefMut`, `Drop`, `Debug` and `Display` |

No nife type, capability or handoff appears in a public signature.

What a stranger may find surprising:

- A second `lock` on the thread that holds it spins forever, where `std::sync::Mutex` may panic or
  deadlock. There is no fairness and no poisoning, and the guard is `Send` when `T` is.
- `force_unlock` is outside the models by construction: it breaks a lock whose holder will never
  release it. Its one caller in nife is the kernel's panic path.
- `new` is not `const` under `--cfg loom`, because loom's types are not.

## Proofs

Five loom models and no Kani harness. Two takers and three takers are exclusive and ordered, a
refused `try_lock` does not release the holder, a Release unlock publishes what the holder wrote,
and a Relaxed unlock publishes nothing, a model that passes only when loom reports the race. The
three-taker model runs at a preemption bound of one, which is loom's limit on spin-wait loops.
Loom judges against C11, not the Arm or RISC-V model, and the crate's `BUGS` says so.

## Names

The ruling renames the crate and nothing inside it. This package ratifies none of the names below.

| item | kind | status | proposal |
|---|---|---|---|
| `test_and_set_lock` | crate, package, repository | ratified by the architect's ruling on 2026-11-19 (UTC), recorded in the crate's `Name:` block and in milestone 1109 | none |
| `SpinLock`, `SpinLockGuard` | types | ratified 2026-10-26 (UTC), on #2760 (milestone 953 (the kernel's lock proves what it takes from spin)) | none; the type keeps its spelling, so the crate name and the type name differ on purpose |
| `SpinLock::new`, `lock`, `try_lock`, `force_unlock` | functions | ratified 2026-10-26 (UTC), on #2760 | none |
| `get_mut`, `into_inner` | functions | provisional, minted 2026-11-14 (UTC), in the standard library's spelling and meaning | keep; no rename is proposed |

No later rule contradicts a ratified name. `lock` and `try_lock` are the standard library's
spellings, and no name carries a unit or an abbreviation.

## What the move lane does

| item | owner |
|---|---|
| Rewrite the README's 6 passives in the active voice, strip the `Name:` blocks, rewrite the 6 citations of paths in this tree as plain prose, and drop `notes/...` and `script/...` mentions from the manifest comments. | move lane |
| Write `repository`, `description` and the manifest literals; the moved `[lints.rust]` keeps `cfg(loom)` in `check-cfg`. | move lane |
| Add one `.github/mutants-exempts` line for the `pause` survivor, with the triage's reason. | move lane |
| Ship `falsifications/interleavings.two_takers_are_exclusive_and_ordered.patch` and a loom form of the template's falsify gate (item 7 of the milestone). | move lane, with `nifeos/crate-workflows` |
| Convert the three dependents to `git` and `rev` with `version = "0.1.0"`; keep `weaken-spin.sh` and `script/interleaving-check` pointed at the pinned checkout. | move lane |
| Rewrite `crates/spin_lock` in `design/decisions/0011-per-cpu-run-queues.md` and `0046-dependency-rule.md` to the new path. | an architect, since the lane does not edit `design/decisions/` |
| Run the stranger test, the template's workflows and the first weekly mutation run in the new repository. | move lane |

## Open questions

None.

## Recommendation

Ready for review.
