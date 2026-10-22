# `serial_console`

This appendix holds the 2026-09-20 triage of `serial_console`'s 43 survivors, module by module,
taken while the crate was named `board_console`, the name the account below keeps. The summary and
the current figures are in [notes/mutation-testing.md](../mutation-testing.md).

## 2026-09-20: `board_console`, milestone 326 (turn a mutation score upward) part 3

`board_console` had one of the new-crate backlog's worst rates: 238 viable, 43 survivors, 79.4%
caught in the 2026-09-19 census. Nobody had taken it until now. The discipline is the
[new-crate backlog](new-crate-backlog.md)'s. Every number below is re-derived with
`script/mutation -p board_console` on this lane's own worktree rather than trusted from the census.
Every kill was verified by re-running the sweep and watching the mutant die. Both equivalence
claims are mutants the second run still reports.

Before: 242 caught, 43 missed, 6 timeouts, 31 unviable (83.2% of viable, 85.2% counting a timeout
as a kill). After: 283 caught, 4 missed, 6 timeouts, 31 unviable (96.6%, 98.6% counting timeouts).
The census's own count differs from this run's (238 viable against 291). Its own `BUGS` section
gives the reason: the tool version and the tree both move between a census and a lane taking it.
This run's `before` column is what this branch measured before anything changed, so it is the
comparison point for the `after` beside it.

| module | survivors | killed by a test | equivalent | recorded gap |
|---|---|---|---|---|
| `board.rs` | 3 | 3 | 0 | 0 |
| `lottery.rs` | 8 | 8 | 0 | 0 |
| `port.rs` | 12 | 9 | 0 | 3 |
| `progress.rs` | 9 | 8 | 1 | 0 |
| `screen.rs` | 2 | 2 | 0 | 0 |
| `stop.rs` | 2 | 2 | 0 | 0 |
| `watch.rs` | 7 | 7 | 0 | 0 |
| **total** | **43** | **39** | **1** | **3** |

The 6 timeouts are not in the 43. All six are in `screen::parse_pixmap`'s header-token loop: `+=`
becoming `*=` or `-=`, or a comparison flipped so the whitespace-and-comment skip never reaches a
byte that ends it. The standing reading in
[how to read the numbers](../mutation-testing.md#how-to-read-the-numbers) applies unchanged: they
are the tests noticing by hanging rather than by failing. `cargo-mutants`' own 20-second per-mutant
bound is the deadline. Nothing here converts them, and nothing needed to. No test in this crate
calls the parser through a doctest, or any other path a deadline-wrapped test could not also reach.

### `board.rs`: 3 killed

`Sign::seen_in`'s (now `is_seen_in`) `None if complete => rest` arm had no test with a *complete*
line short enough to hit it. The arm is a complete line with no trailing whitespace after a
`WordAfter` prefix, such as `U-Boot 2021.10` with nothing following it. The existing test only fed
that shape as a partial line.

`Rung`'s `PartialEq` was only ever exercised through `<`, which goes through `partial_cmp`/`Ord`
rather than `eq`. So `assert_ne!` on two different depths was the one assertion missing.

`Profile::keys` was only ever checked for emptiness (`XENON.keys().count() == 0`), which an
always-empty iterator also satisfies. Checking `RADON.keys()`'s actual contents closes it.

### `lottery.rs`: 8 killed

Two were `parse_core_line` itself. No existing test called it directly, only through `tally`, whose
fixtures happen not to probe the exact byte offset or a malformed token.

The `+`-to-`-` mutant shifts the slice nine bytes early. Every fixture in the suite has enough real
content shifted into view that the guard filters it back out. So the miss needed a token planted at
exactly the wrong distance: `core=0 G9 threads=1 R2`, built so the shifted read lands inside `G9`.

The `||`-to-`&&` mutant makes the "skip a malformed token" guard unsatisfiable. An empty remainder
is vacuously all-digits, so "not all digits AND empty" can never hold. A token like `Grinder` then
starts reading as a grinder. The test that catches it is the token `||` was guarding against.

The other six are all `Series::report`. None was exercised, because every existing test asserts
that specific substrings are present and never that an absent one stays absent. The six are:

- the "never reached the workload" line printed when `attempts == draws.len()` (`>` to `>=`);
- the distribution header printed only when there is nothing to show it for (`!judged.is_empty()`
  deleted);
- a bucket's `==` filter flipped to `!=`, invisible in the old test because the wrong bucket
  produced the *same* substring the right one would have, in a different row;
- a single rate rendered as a degenerate range (`lo == hi` guard disabled);
- the exclusion footer printed when nothing was excluded (`<` to `<=`).

Closed with `Series`/`Draw` built directly rather than through `tally`. Both are `pub` structs with
`pub` fields, kept exactly for this: pinning the report to values chosen for the assertion, not to
whatever a fixture happens to contain.

### `port.rs`: 9 killed, 3 recorded gaps

`candidates()` read `/dev` directly. So no host test could pin down its result without depending on
what happens to be plugged into the machine running it. This crate's own `BUGS` puts it as "this
lane's own Mac has the CH343 attached ... CI has nothing". The walk now follows the move
`choose`/`pick` already made. It is `scan(dir: &Path)`, with `candidates()` as a one-line
`scan(Path::new("/dev"))`. `scan` is tested against a temporary directory built for the purpose
rather than against `/dev`: a real prefix, a look-alike with the wrong prefix, and a sort check.

The `stty()` boundary had nine mutants collapsed to one line; this module's own header calls it
"the whole IO residue... on purpose". No test had called it directly. Every existing test reached
it only through `open`/`configure` against a regular file. Those tests check the complaint text only
for presence, which every mutant's `Some(...)` also produces. A direct call with a flag no `stty`
accepts is a real, portable failure that needs no board. Checking that the exit status is neither
the empty string nor the mutants' `"xyzzy"` placeholder kills all nine at once. None of them is
killed by asserting exact OS-specific wording.

Two of the three `port.rs` mutants left are `candidates`'s trivial delegation, recorded as gaps.
`scan`'s tests prove the logic. But `candidates() = scan(Path::new("/dev"))` still depends on what
is plugged into the machine running `cargo test` to be observably non-trivial. The module's own
`BUGS` already names the same limitation for `configure`/`confirm_speed`.

The third is `confirm_speed`'s whole body replaced with `None`. Every path this test can build (a
regular file, a missing path, invalid UTF-8) makes the underlying `stty -f <path>` fail
(`ok=false`). That was checked directly against `/dev/null`: `stty: /dev/null isn't a terminal`. A
failure already returns `None` by the function's own `if !ok { return None; }`. So the mutant is
indistinguishable from correct code on every input a host test can construct. Reaching the
`Some(...)` arm needs a file `stty` treats as a real tty. That is the "No test opens a real serial
device" limitation this crate's `BUGS` already carries. Closing it needs a pseudo-terminal pair
(`posix_openpt`). That is a dependency decision under DECISIONS §46 (thin primitives or whole
subsystems), which a mutation-triage lane has no authority to take.

### `progress.rs`: 8 killed, 1 equivalent

`machine_line`'s capture guard was `&&`-to-`||`. `banner_line` already had a test for the same
"kept from first arrival" property; `machine_line` did not.

`Stage::label` and its `Display` impl (a whole-body replacement to `""`, or a no-op write) had never
been asserted on directly. Every report reads them by composing a string around them, which does
not pin the label's own text down.

`Failure::describe`'s one match guard (`reason.is_empty()`, forced to `true` or to `false`) had a
test for each shape of `Failure::FirmwareRefused`, but neither test called `.describe()`.

`LineFeeder::tail`'s whole body was never read back directly, only through the `Feeding::tail` it
also produces.

The one equivalent is `BootProgress::reach`'s `>` becoming `>=`. The extra branch fires only when
`stage == self.reached`. For the one variant carrying data (`Stage::Firmware(&'static Rung)`), that
can only hold when the two are the *same* rung. Within one session every `Stage::Firmware` comes
from the one profile a `BootProgress` was built with. `every_profiles_depths_count_from_one_without_gaps`
(in `board`; depth is `Rung`'s whole identity) makes a depth point at exactly one rung in that
profile. Reassigning `self.reached` to a value identical to what it already held is not observable.
This is recorded at the function itself as well as here.

### `screen.rs`: 2 killed

`ReadError`'s `Display` (a no-op write, the same shape as `Stage`'s above) had only ever been
checked by `PartialEq` on the value. No test read what a person at a bench would see.

The header's three-way `max != 255 || width == 0 || height == 0` had one existing test, for the
`max` term alone (`b"P6\n2 2\n254\n"`, both dimensions valid). An `||` flipped to `&&` anywhere in
the chain survives that test, because the single true term already short-circuits the *correct*
code. Closed with `width == 0` and `height == 0` each alone, the other dimension valid. A flipped
`&&` fails on those regardless of which of the two `||`s it replaced.

### `stop.rs`: 2 killed

`Report::describe`'s whole body (`String::new()` or `"xyzzy".into()`) was never called by any test.
Every existing test checks `Escape::report()`'s value. `describe()`'s wording is read only by
`Session::summary`, which composes it rather than asserting on it. One test now covers all five
variants, checking a distinguishing substring for each.

### `watch.rs`: 7 killed

`Session::summary`'s whole body has the same shape as `Report::describe` one level down, and was
never called directly for the same reason. `Session`'s fields are all `pub`, so the test builds it
directly rather than watching a real session.

`bytes += chunk.len()` becoming `*=` was invisible. The one existing assertion on `.bytes` checks
the zero case, where `0 * n` and `0 + n` agree for `n = 0`. The fix asserts
`session.bytes == sink.len()` on a real transcript. There every byte read is also a byte written,
so the two counts have to agree by construction.

Deleting the tail flush's `!feeder.tail().is_empty()` makes the flush fire only on an empty tail, a
no-op. It had never been exercised: nothing in the suite fed a source that ends mid-line. This
module's own header names that as the reason the flush exists: `[PANIC] ...` with no trailing
newline, followed by a halted machine.

`reader`'s `Err(e) if e.kind() == io::ErrorKind::Interrupted` had three mutants: the guard forced
`true`, forced `false`, and its `==` flipped to `!=`. No test had reached it, because nothing in the
suite hands the reader thread a real `io::Error`. Closed by a source that returns `Interrupted` once
and a genuine error after it. The test checks that the interrupted read was retried (no `Failed`
sent, the loop kept going). It also checks that the real error was not retried, and propagated as
the `Err` that `watch` returns.

No defect in shipped behavior was found. Every survivor here was a missing test, or one of the two
documented, measured equivalences above.
