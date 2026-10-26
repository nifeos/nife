# The unsafe code in the crates the kernel links

*Name: provisional, minted 2026-10-25 (UTC) by maintainer/kernel-dependency-unsafe-listing.*

§46 (thin primitives or whole subsystems; we write everything in between) gives the kernel a bar
that a third-party crate must clear before the kernel links it. Its condition 4 asks for the crate's
`unsafe`: where each hand-written block sits, what checks it upstream or here, and whether a claim in
nife rests on it. The one crate the kernel links today, `uuid` on x86_64, predates the bar. This
note is its listing. It also lists the `unsafe` of the kernel's in-tree lock, `crates/spin_lock`,
because the largest claim the kernel makes about ordering rests on it.

No register crate is in the kernel. aarch64's system registers are one `asm!` accessor each in
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

`cargo tree -e normal -p kernel --target <t>`, keeping the packages with no local path:

| Target | Registry crates in the kernel |
|---|---|
| `aarch64-unknown-none-softfloat` | none |
| `riscv64imac-unknown-none-elf` | none |
| `x86_64-unknown-none` | `uuid` 1.28.0 |

`--all-features` gives the same set. `uuid` arrives on x86_64 through `boot_slot` and
`globally_unique_identifier_partition_table`, and pulls no procedural macro or build script into
the kernel. §46's table also names `spin`, which the kernel does not link; the
kernel's lock is `crates/spin_lock`, listed below.

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
| `uuid` 1.28.0 | 13: 9 blocks, 2 in macro bodies, 2 `unsafe fn` | 13 | 0 | 6 (0) | 2 |

`python3 helpers/kernel_dependency_unsafe.py --summary` prints the first, fifth and sixth columns
from the table. The block counts agree with the census in
[dependency-standards-census.md](dependency-standards-census.md), which measured the same source
with a different tool (it read `uuid` at 1.24.0, the version `redoxfs_server` pins).

## spin_lock, the in-tree lock

`IrqSafeMutex` in `kernel/src/sync.rs` wraps `spin_lock::SpinLock`, and a handful of statics in
`kernel/src/user/fs_service/two_dirs.rs` and `system_tests/` use it directly. It is not a registry
crate, so the table and the helper do not list it; its sites are here because the claim below rests
on them. The kernel calls `new`, `lock`, `try_lock` and `force_unlock`, and uses the guard's
`Deref`, `DerefMut` and `Drop`.

The `unsafe` in `crates/spin_lock/src/lib.rs`, all compiled into the kernel and all reached:

| Site | What it relies on | What checks it |
|---|---|---|
| `unsafe impl<T: Send> Sync for SpinLock<T>` | one guard at a time reaches the data, so sharing the lock moves a `T` between threads and never shares it | the loom models |
| `SpinLockGuard`'s `Deref::deref` block, `&*read(&self.lock.data)` | the guard exists only after the Acquire compare-exchange won, and the flag stays set until its drop's Release store | the loom models |
| `SpinLockGuard`'s `DerefMut::deref_mut` block | the same, and `&mut self` makes the borrow unique | the loom models |
| `unsafe fn force_unlock` | its caller's promise, on the panic path, that no guard is live or that a torn critical section is acceptable | nothing; outside the models by construction |

The guard needs no `unsafe impl` of `Send` or `Sync`: a `PhantomData<&mut T>` field makes it
`Sync` only when `T` is, and `Send` only when `T` is.

What checks it: `script/interleaving-check`, which CI runs as the required `interleavings` job.
Under `--cfg loom` the flag and the data cell are loom's, so the model fails any two accesses to the
data that no happens-before edge orders. Five models:

- `two_takers_are_exclusive_and_ordered`: two threads each take the lock and increment the data.
  Every interleaving, no preemption bound.
- `three_takers_are_exclusive_and_ordered`: three threads, with every execution that preempts a
  running thread at most once. The bound and why it sits there are in
  [interleaving/what-was-modeled.md](interleaving/what-was-modeled.md).
- `try_lock_refuses_while_another_guard_holds`: a held lock refuses `try_lock`, and a refusal leaves
  the holder holding.
- `a_release_publishes_what_the_holder_wrote_before_it`: a write outside the lock, made before the
  release, is visible to the next holder. This is the edge the kernel's IPC rendezvous uses.
- `a_relaxed_release_publishes_nothing`: the same model with the crate's own release store passed
  `Relaxed`. It passes only when loom reports the causality violation, so a run that stops seeing
  the bug fails.

Weakening the guard's release store to `Relaxed`, or the lock's compare-exchange to `Relaxed` on
success, fails the first four models.

The claim that rests on it: [memory-ordering.md](memory-ordering.md), "Where this kernel's
ordering actually lives": most of the kernel's happens-before edges come from this lock's Acquire
compare-exchange and Release store, and §9 (locking) and §11 (per-CPU run queues) lean on them.
The guard's `deref` and `deref_mut` blocks turn that ordering into a `&mut T` with no data race.
Loom proves it in the C11 model; it is not a proof about aarch64 or riscv64 silicon
([interleaving.md](interleaving.md)).

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
| A kernel lock gives exclusion and a happens-before edge | `spin_lock`: the guard's `deref` and `deref_mut`, and the lock's `Sync` impl | loom, two and three threads, with a falsification | nothing to track |
| A partition GUID survives the on-disk layout | none | Kani, over safe code | nothing to track |

## Keeping it current

The listing changes with a bump, and `script/lint` makes the bump say so. A pull request that
moves `uuid`, or adds a crate to the kernel's graph:

1. fails `script/lint` with "sites.tsv is stale";
2. runs `python3 helpers/kernel_dependency_unsafe.py --write` and reads the table's diff, which is
   the `unsafe` delta of the bump;
3. rereads this note for each changed site: reached or not, checked by what, and whether a claim
   moved, and updates the note in the same pull request.

A pinned fact (`PINS` in the helper, empty today) fails even after step 2, with the claim named in
the message.

## EXAMPLES

```
$ python3 helpers/kernel_dependency_unsafe.py --summary
uuid 1.28.0: 21 rows; 13 hand-written outside tests (block 9, block in a macro body 2, fn 2); 6 macro expansions; 2 in test modules
```

## BUGS

- A person reads the "compiled" and "reached" columns by hand. A kernel change that starts calling
  a `Uuid` formatter fails nothing; the next bump's reread is the only thing that catches it.
- This note read the upstream checks at each release commit on 2026-10-25 (UTC). A project can drop its
  Miri job without a release, and nothing here would notice.
- Expansions count where a crate invokes its own macro. The kernel invokes none of `uuid`'s macros that
  hold `unsafe`; a future caller of one would add sites the table does not list.
