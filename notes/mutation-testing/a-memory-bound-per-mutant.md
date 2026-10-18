# A memory bound per mutant

This appendix of [notes/mutation-testing.md](../mutation-testing.md) records how a mutation run
bounds what one mutant may allocate, the measured ceiling, and a demonstration that it fires.

## 2026-09-12: bounding what one mutant may allocate

This is milestone 277 (bound what one mutant may allocate, so a runaway kills the mutant and not the
machine). Every scheduled run of the `mutation testing` workflow had died. Milestone 238 (two
scheduled checks have never once succeeded) fixed the off-by-one in the shard indices, and exactly
one cause was left. **One mutant went 1.4 GB to 15.8 GB in twenty seconds and took the runner agent
down with the machine.** The workflow's own BUGS header has the ten-second samples that caught it.
It also explains why a sixty-second sampler had reported innocence an hour earlier.

A clock cannot catch an allocation, and a clock was the only bound there was. cargo-mutants 27.1.0
offers `--timeout`, `--build-timeout`, a multiplier for each, and `--minimum-test-timeout`. That is
the complete list, checked against `cargo mutants --help` rather than assumed. The cheapest outcome
would have been that the tool already had the feature; it does not. On this tree the auto timeout
derives to 28-51 seconds, and 16 GB is gone well inside that, so the existing bound could never have
fired.

### The mechanism: a cargo runner

cargo runs a test binary through `target.<triple>.runner` when one is set. That is the only point in
the pipeline that sees exactly one test binary: not rustc, not cargo, not the other `-j 2` job's
binary. So it is the one place a per-mutant bound can live.

`helpers/memory-bounded-runner.sh` (name provisional; names are an architect's) sets `RLIMIT_AS`
with `ulimit -v` and execs the binary. `script/mutation` points `CARGO_TARGET_<HOST>_RUNNER` at it
for the length of a mutation run. Nothing else in the tree does, so an ordinary `cargo test` is
untouched.

Putting it in `.cargo/config.toml` was refused for that reason. That file would bound every host
`cargo test` in the tree, and a memory ceiling is a property of a mutation run, not of the tree.

### The number, from both directions

The ceiling is 4 GiB, measured twice rather than picked. First, each child's own `VmPeak` across all
143 host test binaries (`cargo test --workspace` minus the bare-metal crates, at
`--test-threads=4`):

| | peak address space |
|---|---|
| largest (`board_console`) | 1,028 MiB |
| next (`graphics_contract`) | 481 MiB |
| mean across 143 binaries | 169 MiB |

So 4 GiB is 4.0x the largest honest test binary in the tree. The other half of the choice is the
machine. Under `-j 2` two test binaries can be resident at once, so a runaway in each costs twice
the ceiling. 8 GiB is survivable on the 16 GiB boxes this runs on, where twice a larger ceiling
would not be. Four is the largest value that keeps both true.

Second, lowering it until it bites proves the bound is enforced. The honest suite is unaffected at
4 GiB and still unaffected at 1 GiB. At 256 MiB `board_console` fails, exactly where its measured
peak says it should.

### That it fires, demonstrated

A bound nobody has watched kill something is decoration. §134 (a harness carries a
machine-replayable falsification record, or it is not evidence) makes the same argument about
proofs. A synthetic crate whose `i += 1` cargo-mutants rewrites to `i *= 1` leaves the loop counter
at zero forever and pushes without limit. That is the exact shape the workflow measured. Run at four
ceilings, it dies at whatever ceiling it is given and nowhere else:

| ceiling | died after | allocation that failed |
|---|---|---|
| 512 MiB | 0.4 s | 536,870,912 bytes |
| 1 GiB | 1.7 s | 1,073,741,824 bytes |
| 2 GiB | 3.9 s | 2,147,483,648 bytes |
| 4 GiB | 8.6 s | 4,294,967,296 bytes |

Being bounded only by the ceiling is what "unbounded" means, and why no clock was ever going to help.
Under `cargo mutants` with the bound in place, that crate's 8 mutants came back 8 caught, the
runaway among them, and the run exited 0.

The accounting matters more than the kill. `script/mutation` treats 0 as all-caught, 2 as survivors
and 3 as timeouts. **Everything else is a broken run that exits fatal.** A bound that killed a
mutant but turned the run red would be the same outcome with a new cause. It does not. At 8.6 s the
allocation failure lands well inside the 20 s minimum auto timeout. Rust's allocation error handler
aborts the process, cargo sees a failed test, and cargo-mutants records an ordinary caught mutant.
The sweep continues.

The failure path is covered too. `MUTATION_MEMORY_LIMIT_KB=1024 script/mutation -p bitmap_font`
fails the unmutated baseline and exits 4, unchanged, with a new line naming the ceiling as the
suspect. An honest test binary that cannot fit under the ceiling fails in exactly the shape a broken
baseline does, and the next reader should not have to find the runner on their own.

### What is not bounded, and where this is not enforced

- The build is not bounded. This wraps the test binary, so a mutant whose damage lands in rustc is
  still unbounded. None has been observed (the measured runaway is test-side). A linker legitimately
  wants a great deal of address space, so it would need a different number.
- It is off by default on macOS, and that is a gap rather than a verdict. XNU does enforce
  `RLIMIT_AS`, contrary to the folklore and to what this lane first wrote down before reading the
  source. `bsd/kern/kern_resource.c` hands the limit to `vm_map_set_size_limit()`.
  `vm_map_enter()` fails with `KERN_NO_SPACE` once `map->size` passes it, and
  `vm_map_inherit_limits()` carries it across exec. All of that has been true since at least
  xnu-8792 (macOS 13). What is not known is what number is safe there. This lane had only Linux to
  measure on, and macOS reserves address space far more freely than glibc does. Defaulting it on
  with an unmeasured number would fail every Mac sweep at its baseline. Defaulting it off leaves the
  dev Mac exactly as exposed as before. Turning it on is one environment variable, and whoever
  measures it first should write the number into the runner's header.
- The ceiling is address space, not resident memory, so it over-counts reservations nothing
  touches. `RUST_MIN_STACK` is 16 MiB here, and `--test-threads=4` makes that 64 MiB of untouched
  stack before a byte of heap. That over-count is priced into the 4x headroom.
- The weekly workflow has still never succeeded. This was demonstrated against a synthetic runaway
  and measured against the honest suite, both on Linux. The first green scheduled run is the
  evidence that matters, and it has not happened yet. (Corrected 2026-09-24: the 2026-09-14 census
  was the first weekly run to finish; see [the first weekly censuses](first-weekly-runs.md).)
