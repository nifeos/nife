# The error paths no test reaches

*Name: provisional (milestone 745's lane, 2026-10-04 UTC). calef names things; expect this to change.*

Measured 2026-10-04 (UTC) by milestone 745 (count the error paths no test reaches), a provisional
number. calef asked for it as the measurement that comes before any fault-injection work. This note
names the question, answers it, and leaves the remedy to calef. The full per-crate table and the
list of cleanup paths are in the [appendix](untested-error-paths/tables.md).

## The question

Across the kernel, the services and the host crates, how many error paths does no test execute?
Which of them release memory or authority, so that a fault there would become a leak or a
confinement bug (fatal risk 7)?

An error path here is a `?` taken on its error side, an `Err` built or returned, an error arm of a
`match` or a let-else, a refusal status code sent or returned, or a cleanup after a partial failure.

## The answer

| | Result-family paths | not reached | Option-family paths | not reached |
|---|---:|---:|---:|---:|
| host crates, measured | 1,150 | 586 (51%) | 437 | 156 (36%) |
| kernel, services, boot crates, unmeasured | 1,159 | unknown | 602 | unknown |

- Half the error paths in the host crates have never run under a test. Fifteen more sit on lines
  llvm-cov does not map.
- Half of all error paths in the tree are in code no coverage run can see. That is the kernel, the
  services in `components/`, `system_initializer`, `redoxfs_server` and four small protocol crates.
- The `?` operator is the gap. Of 636 Result-family `?` in host crates, 490 (77%) never took their
  error side. Of 457 explicit `return Err`, `.ok_or` and refusal codes, 71 (16%) never ran.
  Tests check the refusals a function writes itself, and rarely the ones it passes up.
- Every cleanup path the classifier found, 75 of them, is in unmeasured code. The host crates hold
  none. Cleanup after a partial failure, the case risk 7 cares about most, has no measured
  coverage anywhere.
- Four crates carry 303 of the 586: `device_tree_blob`, `uefi_loader`, `stick_maker` and
  `portable_executable`. All four are parsers or host tools.

### By kind, in the host crates

| kind | paths | not reached |
|---|---:|---:|
| `?` in a Result function | 636 | 490 |
| `return Err(..)` or a built `Err` | 402 | 69 |
| `.ok_or(e)` with no `?` | 10 | 1 |
| `Err(..)` arm or `let Ok(..) else` | 52 | 23 |
| refusal code used as a value | 45 | 1 |
| arm on a refusal code | 5 | 2 |
| `?` in an Option function | 193 | 131 |
| `return None` | 155 | 25 |
| `let Some(..) else` | 89 | 0 |

The Option family is reported apart because an absent value is often not an error. In the hand
check below, two of sixteen Option paths were a lookup finding nothing.

### By crate, the fifteen with most unreached Result paths

| crate | Result paths | not reached |
|---|---:|---:|
| `device_tree_blob` | 114 | 102 |
| `uefi_loader` | 108 | 94 |
| `stick_maker` | 88 | 61 |
| `portable_executable` | 58 | 46 |
| `activation_set` | 50 | 33 |
| `machine_discovery` | 53 | 31 |
| `timetable` | 67 | 30 |
| `walk_pricing` | 27 | 23 |
| `elf` | 50 | 20 |
| `grant_plan` | 93 | 17 |
| `swish` | 35 | 17 |
| `board_console` | 31 | 16 |
| `calendar` | 39 | 15 |
| `globally_unique_identifier_partition_table` | 45 | 14 |
| `paging` | 24 | 12 |

Seventy of the 86 crates in the report have an error path, and 24 of those have none unreached.
`stick_maker` includes its host files, which `script/coverage` runs but exempts from the floor.

## The twenty that release memory or authority

Ranked by hand from the measured unreached paths, in three groups. The first seven release
something. The next five fire midway through a grant, after something was taken. The last eight
refuse a grant or an allocation. Line numbers are at base `1a145fcaa`.

| # | where | path | what a fault there would cost |
|---|---|---|---|
| 1 | `crates/subtree_scope/src/lib.rs:280` | `unbind` refuses a caller that is not the open root | a client revoking another's subtree grant; the check is untested |
| 2 | `crates/paging/src/lib.rs:866` | `unmap` finds no leaf | a revoke that believes it unmapped |
| 3 | `crates/paging/src/lib.rs:840` | `unmap` on the wrong half | same, by address |
| 4 | `crates/paging/src/lib.rs:843` | `unmap` misaligned | same, by alignment |
| 5 | `crates/capability/src/lib.rs:394` | `empty`, slot out of range (first `?`) | the one door a capability leaves a table through |
| 6 | `crates/generational_table/src/lib.rs:175` | `remove` of a stale name | the kernel's table: a stale name removing a live entry |
| 7 | `crates/generational_table/src/lib.rs:176` | `remove` of an empty slot | `live` miscounted |
| 8 | `crates/paging/src/lib.rs:808` | `map_span` fails after `done` bytes are mapped | a half-mapped span, with no unwind in the crate |
| 9 | `crates/paging/src/lib.rs:823` | `map_range` fails midway | the same, page by page |
| 10 | `crates/paging/src/lib.rs:760` | `map_block`'s allocator returns nothing | tables taken at upper levels stay |
| 11 | `crates/paging/src/domain.rs:148` | an IOMMU domain's `map` fails after earlier pages | `kernel/src/iommu.rs` `expect`s it, so a panic, not a leak |
| 12 | `crates/paging/src/domain.rs:138` | a later region's `grant_pages` fails | earlier regions already in the domain |
| 13 | `crates/paging/src/domain.rs:72` | DMA grant misaligned | a device granted a page it should not reach |
| 14 | `crates/paging/src/domain.rs:75` | DMA grant region malformed | the same |
| 15 | `crates/paging/src/domain.rs:143` | `grant_page` past the count | proved unreachable by Kani; the branch stays |
| 16 | `crates/capability/src/lib.rs:477` | `put` to a slot out of range | a grant into nowhere |
| 17 | `crates/capability/src/lib.rs:372` | `fill` out of range | the same door, one level down |
| 18 | `crates/memory_regions/src/table.rs:381` | `split` past the watermark | refused before any mutation |
| 19 | `crates/page_frames/src/lib.rs:263` | `alloc_contiguous` of zero or too many | a DMA ring allocation refused |
| 20 | `crates/paging/src/lib.rs:743` | `map_block` on the wrong half | a user mapping into the kernel half |

Twelve of the twenty are in `paging`. The 2026-08-03 mutation baseline found the same soft spot from
the other side: every DMA-domain test granted exactly one page, so a page count could not be told
from the constant 1 ([baseline survivors](mutation-testing/baseline-survivors-grant-plan-to-swish.md)).

Not reached does not mean reachable. Item 15 is proved dead, and `device_tree_blob`'s 102 are mostly
bounds re-checked after a header check already passed. Telling the two apart is what a fault
injector does and a coverage report cannot.

## The blind spot

`script/coverage` measures the host crates only. The kernel, the services and the crates that wrap
syscalls run under QEMU, where host instrumentation does not reach. The run excludes them, and
`notes/scripts.md` says so. Counted, unmeasured:

| code | Result paths | Option paths |
|---|---:|---:|
| `components/` (services and programs) | 380 | 170 |
| `kernel/` (portable) | 321 | 305 |
| `redoxfs_server` | 237 | 16 |
| `kernel/src/arch/` | 86 | 68 |
| `system_initializer` | 84 | 37 |
| `supervision_protocol`, `cryptography_provider`, `hot_swap_contract`, `user_mode_runtime` | 51 | 6 |
| total | 1,159 | 602 |

The 75 cleanup paths are all here: 27 in `system_initializer`, 17 in `login`, 10 in `swish`, 13 in
the kernel, 8 in other services. They are the rollbacks risk 7 depends on. One is
`MEMORY_REGION_SPLIT`'s destroy-on-full-table (`kernel/src/syscall.rs:844`). Another is
`PAGE_FRAME_MAP`'s all-or-nothing `unmap_run_prefix` (`kernel/src/syscall.rs:1076`). The rest
include `login`'s `connect` and `mint`, whose own comments admit slot leaks. A grep for a test naming
`unmap_run_prefix` finds none, which is evidence and not a measurement.

Two ways to measure it, neither built:

- A QEMU TCG plugin recording executed blocks, mapped to lines through DWARF. It needs no kernel
  change and works on all three ISAs. Homebrew's QEMU 11.1.1 takes `-plugin` but ships no plugin
  libraries, so one would be built from QEMU's `contrib/plugins`. Two costs: blocks give line
  coverage, not regions, so a `?` needs its error edge found by address; and user programs share
  virtual addresses, so the address space must be told apart.
- `-C instrument-coverage` on the kernel with a `no_std` profiler runtime, dumping counters through
  semihosting at the test build's exit. That gives the same regions as the host report. It is a
  dependency, under §46 (thin primitives or whole subsystems), and a change to the test kernel. Services would each need their own dump.

The first is cheaper to try and the second is the better instrument. Either is a milestone of its
own (proposed, below).

## How good the classifier is

`helpers/error_paths.py` reads source lines with strings and comments blanked. It is not a parser.
Four hand checks, 44 paths, drawn at random with fixed seeds:

| sample | paths | wrong | what was wrong |
|---|---:|---:|---|
| measured, any kind | 12 | 1 | an `EPERM =>` arm in a function naming errors |
| unmeasured, any kind | 10 | 0 | |
| stratified toward new kinds | 10 | 4 | a keymap `return None` meaning no binding; a `fmt` arm; `EBADF` passed to a `?` counted twice; `if let Ok` read as an error arm |
| final classifier, fresh draw | 12 | 1 | an Option `return None` that means absent, not failed |

Four of the six mistakes were fixed by a rule, and the counts above are after the fix. The two that
stay are both in the Option family, two of its sixteen sampled paths, each a lookup that found
nothing. The Result
family had no wrong path in the final draw. Every reached verdict in the first sample was checked
against llvm-cov's markup by hand, and all twelve agreed.

Recall was checked on four files by listing every line with an error-looking token the classifier
skipped. It first missed `let Some(..) else`, `return None`, a bare `.ok_or` and a CamelCase
`Refused`. All four are now counted. What it still misses: a failure written as a plain `if` that
returns a sentinel, and a status code it has no name for.

The cleanup detector was checked on ten paths: ten were real rollbacks. One false hit,
`set_ipc_aborted` matching "abort", was found before that and the word removed.

## What it cost

One coverage artifact from merge-queue run 37167533332 at the base (3.8 MB, 2026-10-04 at 01:17 UTC,
92 seconds of CI). That group also carried #1579, which changes no Rust, so its sources are the
base's. The classifier reads the report in 2.4 seconds, and the unmeasured tree in 4. No local
coverage run, no QEMU.

## Where a fault-injection pilot pays first

Recommended, proposed as [a page-table allocator that fails on its Nth
call](../design/roadmap/0774-a-page-table-allocator-that-fails-on-its-nth-call.md). Twelve of
the twenty above are in `paging`. `Mapper` already takes its frame allocator as a closure, so no new
seam is needed. Sweep N over `map_span`, `map_range` and `build_identity_domain`, and after each
failure check that every frame handed out is reachable from the root, and what is left mapped. It is
host-only and runs in milliseconds.

The larger prize is in the blind spot, where all 75 cleanup paths are. A test kernel that fails a
process's Nth `RETYPE` or `SPLIT` would sweep `login`'s `connect` and `system_initializer`'s session
build against the region and capability counters the kernel already keeps. It touched the test
kernel, so it was an architect's call and came second. calef approved it on 2026-10-04 (UTC), and
it is built as milestone 757 (a test kernel fails a process on its Nth retype), provisional
([its block](../design/roadmap/0757-a-test-kernel-fails-a-process-on-its-nth-retype.md)). Its first
sweep of `login` found one leak.

## The retype-fault sweep of `login`

`system_tests/src/user/nth_retype_tests.rs` sweeps one whole login under `crate::retype_fault`,
which makes the kernel refuse one thread's Nth retype as though the region were empty. N runs from
1 until a run makes fewer than N retypes, which is a login that ran to the end. That covers the
`CONNECT` on the front door, the credential relay, `mint`'s caretaker build and the client's budget.
After every run the test compares `login`'s capability table, and the usage of every region it
holds, with their values before the sweep. A cleanup path that forgets a `cap_delete` or leaves a
split child behind crashes nothing, so the kept slot or pages are the only evidence. This note's
count found 17 such paths in `login` that no test reached.

It is a kernel test because no syscall shows another process's capability table or which retype the
kernel is on (`notes/scripts.md`, a new service test is a userspace program unless it cannot be).

It shares `login_tests`' one `login` rather than building its own. A `login` cannot be torn down: it
parks on a front door its own budget did not pay for (`holding.rs`'s BUGS). A second instance would
be a permanent charge on the suite's frame ledger. Every run is refused or logs back out, so it
leaves the instance as it found it, which is the property under test.

What it does not sweep needs a fresh `login` per N, which is the teardown `holding.rs` cannot do:
`login`'s own start-up (the three splits in `_start`) and a login that opens a schedule
(`Durable::open`). `system_initializer`, `swish` and the rest of the 75 cleanup paths are milestone
786 (sweep the other long-lived services under the retype fault).

## EXAMPLES

```sh
# from a CI run's coverage-report artifact, or after script/coverage
gh run download <run-id> -n coverage-report -D /tmp/coverage-report
python3 helpers/error_paths.py /tmp/coverage-report/html
# per-path JSON records land in target/error-paths/
```

## BUGS

- Not reached is measured; reachable is not. A defensive check that cannot fire counts the same as
  a missing test.
- The kernel, the services and `system_initializer` are counted, not measured.
- The Option family overstates: in the sample, one in eight was absence, not failure.
- A `.ok_or(e)` with no `?` is unmeasurable on a reached line, because `e` is built either way.
- The ranking of the twenty is by hand. The classifier's `releases` flag finds cleanup bodies; it
  does not rank.
