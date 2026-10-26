# The unsafe code in the crates the kernel links

*Name: provisional, minted 2026-10-25 (UTC) by maintainer/kernel-dependency-unsafe-listing.*

§46 (thin primitives or whole subsystems; we write everything in between) gives the kernel a bar
that a third-party crate must clear before the kernel links it. Its condition 4 asks for the crate's
`unsafe`: where each hand-written block sits, what checks it upstream or here, and whether a claim in
nife rests on it. The two crates the kernel links today predate the bar. This note is their listing.

No register crate is among them. aarch64's system registers are one `asm!` accessor each in
`kernel/src/arch/aarch64/instructions.rs`, and its device registers are volatile accesses at named
offsets in `kernel/src/drivers/`, as on riscv64 and x86_64. Those sites are the kernel's own: each
sits under a `SAFETY` comment that `clippy::undocumented_unsafe_blocks` requires, and the unsafe
census in [unsafe-obligations.md](unsafe-obligations.md) counts them.

The locations, file and line, live in
[`kernel-dependency-unsafe/sites.tsv`](kernel-dependency-unsafe/sites.tsv), one row per site, which `helpers/kernel_dependency_unsafe.py` writes from the exact source
`Cargo.lock` pins. `script/lint` regenerates it and fails when the two differ, so a bump that moves a
site, or a new crate in the kernel's graph, fails until the table is rewritten and this note is
reread. The prose below says what the table cannot: which sites the kernel reaches, what checks
them, and which claims rest on them.

## Which crates

`cargo tree -e normal -p kernel --target <t>`, keeping the packages with no local path, at the
branch point of this note's pull request:

| Target | Registry crates in the kernel |
|---|---|
| `aarch64-unknown-none-softfloat` | `spin` 0.12.3 |
| `riscv64imac-unknown-none-elf` | `spin` 0.12.3 |
| `x86_64-unknown-none` | `spin` 0.12.3, `uuid` 1.28.0 |

`--all-features` gives the same set. It matches the two §46 names: `uuid` arrives on x86_64
through `boot_slot` and `globally_unique_identifier_partition_table`. Neither pulls a procedural
macro or a build script into the kernel.

## How a site is counted

The helper's docstring holds the rules. In short: every `unsafe` keyword in shipped source (not
`tests/`, `benches/`, `examples/` or `build.rs`) is one site, classified as a block, an `unsafe fn`,
an `unsafe impl` of `Send` or `Sync`, or an `unsafe impl` of another trait. Every `asm!` is a site
of its own. A site inside a `macro_rules!` body carries the macro's name, and each place the crate
invokes such a macro is an `expansion` row, because the bar counts what a crate's macros generate
as the crate's own. The table marks a site inside a `#[cfg(test)]` module, since no kernel
build compiles it.

"Compiled" below means the features and target the kernel builds with leave the site in.
"Reached" means a call the kernel makes leads to it, read by hand from the kernel's call sites.

## The counts

| Crate | Hand-written sites outside tests | Compiled into the kernel | Reached by the kernel | Expansions (reached) | In test modules |
|---|---|---|---|---|---|
| `spin` 0.12.3 | 92: 31 blocks, 26 `unsafe fn`, 27 `Send`/`Sync` impls, 8 other trait impls | 13 | 12 | 0 | 18 |
| `uuid` 1.28.0 | 13: 9 blocks, 2 in macro bodies, 2 `unsafe fn` | 13 | 0 | 6 (0) | 2 |

`python3 helpers/kernel_dependency_unsafe.py --summary` prints the first, fifth and sixth columns
from the table. The block counts agree with the census in
[dependency-standards-census.md](dependency-standards-census.md), which measured the same source
with a different tool (it read `uuid` at 1.24.0, the version `redoxfs_server` pins).

## spin 0.12.3

The kernel takes it with default features off and `spin_mutex` on, which compiles `lib.rs`,
`relax.rs`, `mutex.rs` and `mutex/spin.rs`. `IrqSafeMutex` in `kernel/src/sync.rs` wraps
`spin::Mutex`, and one static in `kernel/src/user/fs_service/two_dirs.rs` uses it directly. The
kernel calls `new`, `lock`, `try_lock` and `force_unlock`, and uses the guard's `Deref`,
`DerefMut` and `Drop`.

Reached, all in source the kernel compiles:

- In `src/mutex.rs`, `unsafe impl` `Sync` and `Send` for `Mutex<T>` where `T: Send`, and the same
  pair for `MutexGuard`, where `&mut T` is.
- In `src/mutex/spin.rs`, the same four impls for `SpinMutex` and `SpinMutexGuard`.
- `SpinMutexGuard`'s `Deref::deref` and `DerefMut::deref_mut` in `src/mutex/spin.rs`:
  `&*self.inner.data.get()` and its mutable twin. Each is sound only while the guard holds the lock alone, which rests on the
  compare-exchange in `try_lock` and `try_lock_weak` and on the guard's release store. Those are
  safe atomics, and they are the part a claim leans on.
- `Mutex::force_unlock` and `SpinMutex::force_unlock`, each an `unsafe fn` around a release store.
  `IrqSafeMutex::force_unlock` calls it on the panic path, through `console::force_unlock`.

Compiled and not reached: `SpinMutexGuard::leak`. Not compiled: the four `lock_api` sites (the
`RawMutex` impls for `Mutex` and `SpinMutex` and their `unlock`) and every site in `once.rs`,
`rwlock.rs`, `lazylock.rs`, `barrier.rs` and the ticket and fair mutexes.

What checks it upstream: The release commit `7c335e36` carries `.github/workflows/rust.yml`,
which runs `cargo miri test --all-features` with strict provenance and the symbolic alignment check
(its step "Soundness (all features)"), and `cargo test`. `SECURITY.md` asks for private disclosure
and supports only the latest release. No Kani, fuzzing or loom. The project now lives on Codeberg
(`codeberg.org/zesterer/spin`), and its GitHub copy is read-only.

What checks it here: The census ran the crate's 80 tests under Miri with no undefined behavior
reported (`dependency-standards-census/miri-sample.tsv`). Miri checks the interleavings the tests
happen to produce on the host. It does not prove mutual exclusion, and it says nothing about
aarch64 or riscv64.

The claim that rests on it: [memory-ordering.md](memory-ordering.md), "Where this kernel's
ordering actually lives": most of the kernel's happens-before edges come from `SpinMutex`'s Acquire
compare-exchange and Release store, and §9 (locking) and §11 (per-CPU run queues) lean on them. The
guard's `deref` and `deref_mut` blocks are what turn that ordering into a `&mut T` with no data race.
Nothing in nife proves it. No Kani harness takes a kernel lock (the aarch64 harnesses in
`kernel/src/arch/aarch64/iommu.rs` prove table logic without locking), Kani has no threads, and
loom cannot run `spin`, whose atomics are `core`'s.

What does hold now: the helper pins the two orderings. It counts the Acquire compare-exchange and
the Release store in `src/mutex/spin.rs` on every `--check`, so a bump that weakens either fails
`script/lint` even after a fresh `--write`. That is a text check on the source, not a proof.
The proof is a proposed milestone,
[the kernel's lock proves what it takes from spin](../design/roadmap/proposals/the-kernels-lock-proves-what-it-takes-from-spin.md).

## uuid 1.28.0

x86_64 only, with default features off (`crates/globally_unique_identifier_partition_table/Cargo.toml`).
The kernel calls `boot_slot::cmdline::parse` and `boot_slot::medium::is_nvme`, and `boot_slot`
calls the partition table crate's `crc` module. No call from the kernel reaches a `Uuid` method,
so the kernel reaches none of the 13 sites.

The partition table crate's shipped code calls only the `const` constructors and accessors
(`from_bytes`, `from_fields`, `nil`, `into_bytes`), which hold no `unsafe`. Its derived `Debug` on
`Entry`, `Header` and the table would format a `Uuid` through `as_hyphenated` (a transmute
expansion) and `encode_hyphenated_uninit`, with `write_bytes` and `assume_init_ascii_mut`. The
kernel never formats one.

The sites fall in three groups. The text encoders in `src/fmt.rs`: `encode_simple_uninit`,
`encode_hyphenated_uninit`, `encode_braced_uninit` and `encode_urn_uninit` each call the
`unsafe fn assume_init_ascii_mut`, whose body holds two blocks; `write_bytes` is a
`copy_nonoverlapping`, and `slice_as_uninit_mut` casts a byte slice to `MaybeUninit`. Two
transmute macros in `src/macros.rs`, `unsafe_transmute_ref!` and `unsafe_transmute!`, which the
crate invokes in `Builder::from_bytes_ref`, in `Uuid`'s `as_hyphenated`, `as_simple`, `as_urn` and
`as_braced`, and in `encode_braced_uninit`. And `NonNilUuid::new_unchecked` in `src/non_nil.rs`. The
text parser holds no `unsafe`.

What checks it upstream: The release commit `276c2362` carries `.github/workflows/ci.yml`. Its
`miri` job runs `cargo miri test --lib --all-features` on x86_64 and on big-endian s390x, with no
`uuid_unstable` flag, so Miri exercises the plain-transmute macros the kernel build compiles. The OS
test matrix sets `--cfg uuid_unstable`, under which `zerocopy` checks the same transmutes' layouts at
compile time. The `fuzz` job runs an AFL canary over the parser (`fuzz/parse`), which has no
`unsafe`. `SECURITY.md` and GitHub's private reporting are both on.

The claim that rests on it: None. The partition table crate's Kani harness
`a_guid_survives_the_on_disk_layout` proves `guid::from_disk` and `guid::to_disk` inverse, through
`Uuid::from_bytes` and `Uuid::into_bytes`, which are safe code.

## The claims, and where each unproven one is tracked

| Claim | The `unsafe` it rests on | What proves it | Where the gap is tracked |
|---|---|---|---|
| A kernel lock gives exclusion and a happens-before edge | `spin`: the guard's `deref` and `deref_mut`, and the eight `Send` and `Sync` impls | nothing; Miri upstream, orderings pinned by text | proposal: the kernel's lock proves what it takes from spin |
| A partition GUID survives the on-disk layout | none | Kani, over safe code | nothing to track |

## Keeping it current

The listing changes with a bump, and `script/lint` makes the bump say so. A pull request that
moves `spin` or `uuid`, or adds a crate to the kernel's graph:

1. fails `script/lint` with "sites.tsv is stale";
2. runs `python3 helpers/kernel_dependency_unsafe.py --write` and reads the table's diff, which is
   the `unsafe` delta of the bump;
3. rereads this note for each changed site: reached or not, checked by what, and whether a claim
   moved, and updates the note in the same pull request.

A bump that changes a pinned fact fails even after step 2, with the claim named in the message.

## EXAMPLES

```
$ python3 helpers/kernel_dependency_unsafe.py --summary
spin 0.12.3: 110 rows; 92 hand-written outside tests (block 31, fn 26, impl-send-sync 27, impl-trait 8); 0 macro expansions; 18 in test modules
uuid 1.28.0: 21 rows; 13 hand-written outside tests (block 9, block in a macro body 2, fn 2); 6 macro expansions; 2 in test modules
```

## BUGS

- A person reads the "compiled" and "reached" columns by hand. A kernel change that starts calling
  `Aliased` or `leak` fails nothing; the next bump's reread is the only thing that catches it.
- This note read the upstream checks at each release commit on 2026-10-25 (UTC). A project can drop its
  Miri job without a release, and nothing here would notice.
- The `spin` pin is a text match on two orderings. It catches a weakened ordering and misses a
  restructured lock that keeps the same expressions and breaks exclusion some other way.
- Expansions count where a crate invokes its own macro. The kernel invokes neither crate's
  macros that hold `unsafe`; a future caller of one would add sites the table does not list.
