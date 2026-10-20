# The unsafe census, week by week and by trust boundary

*An appendix to [the register of measures](../register-of-measures.md). The name is provisional.*

## unsafe blocks outside `kernel/src/arch/`

Blocks per 10,000 code lines, outside `kernel/src/arch/`. This is `script/lint`'s census and the
one number in this tree that is actually gated on a direction. The dashed line is the ceiling.

227, 243, 138, 121, 111, 93, 77, 77. Over the same period the absolute count of unsafe blocks
outside `arch/` went from 171 to 704, while the density fell by two thirds. Both are true, and only
the density is about soundness; the count is a system being built. That is why the gate holds a
ratio.

The one rise is 2026W29 to 2026W30, the shape of an early kernel: the tree was 7,500 lines and one
driver moved the number.

### How much this series has been checked

This is the one place in the register where a number has been checked two different ways.

#### A consistency check

Comparing against `notes/unsafe-obligations.md` is a consistency check. That note carries a
seven-point table, 227.8 down to 78.8, taken on seven different dates. It is *not* an independent
record. It is the output of this same derivation, written down seven times by lanes. Reproducing it
proves the walk agrees with what was recorded, and nothing more.

It did agree. On 2026-08-18 the reconstruction hits 747 blocks over 80,359 lines exactly, at commit
`93607fa4`: both figures to the digit. `unsafe impl Send`/`Sync` and the inside-`arch/` count match
the recorded value at five of the seven dates, and are within three at the other two. The residual
is which commit inside the day the column was taken at, not a difference in definition.

The 2026-08-23 column is the worked example. The table prints 777 and this reconstruction prints 799.
The note's own prose already says the count immediately before that lane's reduction "was 799, not
777", over 85,476 lines. That is this reconstruction to the digit.

#### An independent check

A second counter was written from scratch for this. It is a character scanner that tracks Rust's
real lexical states, rather than blanking comments and literals with one regular expression. It
differs deliberately in two places where the regex is loose. Block comments nest in Rust and the
regex's do not. And a lifetime `'a` is not the start of a character literal.

Run over every in-scope file at `705e3919`, the two methods agree on 701 blocks outside `arch/` and
253 inside, with zero files disagreeing. (Those are that commit's own row; the number moved with the
tree afterwards.) So the shortcuts in `script/lint`'s regex do not bite in this tree.

## The same unsafe blocks, by trust boundary

The chart of `unsafe_outside_arch` mixes two populations that mean opposite things. This is the same
census with the two separated. `unsafe_outside_arch` (824 today) adds every `unsafe` block not in
`kernel/src/arch/`, kernel and userspace both, into one number.

In a capability microkernel that matters. An `unsafe` block inside `kernel/src` runs with nothing
confining it. An `unsafe` block in a userspace program (`components/`, `fixtures/`, or a crate that
ships only into one of them) is confined by the MMU-plus-capability-table mechanism that confines
every other program. A single density cannot answer "how much of the code that matters for
isolation is unsafe", because it never separates the two populations that question is about.

A research lane raised exactly this problem on 2026-09-20, in `notes/trusted-base.md`, written for
the RedLeaf comparison (see `notes/redleaf.md`). It hand-computed a first cut and flagged in its own
`BUGS` that nothing kept it computed. This split is that mechanism. The note has since landed.

### Today's split, and a smaller correction

Today, 2026-09-20: 694 kernel, 382 userspace, 19 shared, 37 boot chain, 0 unclassified. That sums to
1,132, not 1,138 (824 + 314). The 6-block difference is not the split disagreeing with the old
census. It is a separate, smaller correction this pass found while classifying `crates/`.

Three crates (`board_console`, `portable_executable`, `stick_maker`) are host tooling. They run on
the developer's Mac and never on nife, exactly like `bench/host/`, `xtask/` and the rest of
`unsafe_census`'s own `HOST_ONLY` exclusion. They are just not under one of `HOST_ONLY`'s path
prefixes, and each says so in its own header.

`unsafe_outside_arch` and `unsafe_density` are left untouched by this finding. They still mean
exactly what they meant, and `script/lint`'s ceiling still gates the same number it always has. This
split simply does not count those three crates' 6 blocks toward either side. The question it
answers, kernel privilege or userspace confinement, has no answer for code that runs on neither.
Whether `unsafe_census`'s own `HOST_ONLY` should widen to match is an architect's call, recorded
rather than made here.

### What each bucket is

The boundary decisions behind each bucket are in `helpers/rust_source.py`'s own comment on
`trust_boundary_census`.

- kernel (694 blocks, 48,724 code lines, density 142 per 10,000). This is `kernel/src/**` (arch and
  not) plus the sixteen `crates/` members reachable *only* from the `kernel` package, over a real
  `cargo metadata` dependency edge. `paging` and `dma_validator` are among them, both lifted out of
  `kernel/src` on purpose so Kani could reach them.
- A count of only `kernel/src/**` misses these sixteen crates entirely. The hand computation in
  `notes/trusted-base.md` did that when written, and its 577 undercounted the trusted base by the
  117 blocks those crates carry. That note's `BUGS` section already named this exact risk ("a tree
  can shrink [the kernel line count] by moving code out of `kernel/src` without reducing what anyone
  has to trust"). It had not checked whether it had happened to its own number. The note now
  records the 577 as short by 117, corrected the same day it was written.
- userspace (382 blocks, 31,639 code lines, density 120 per 10,000). This is `components/`,
  `fixtures/`, and the sixteen `crates/` members reachable only from them. It also takes the
  packages that are each their own cargo workspace and never appear in the main one, listed in
  `rust_source.USERSPACE_ONLY_DIRS`: `redoxfs_server`'s `el0` build, `std_exerciser`,
  `entropy_backend`, `cryptography_exerciser` and `cryptography_provider` among them. By its own
  header, every one of them runs on nife at EL0 and never as kernel code.
- shared (19 blocks, in 5 of 36 crates reachable from both sides). This code genuinely executes with
  kernel privilege in the kernel binary and, separately, under confinement in a userspace program.
  `environment_contract`'s `ConfigPage` and `clock_contract`'s `ClockPage` are built by the kernel's
  `unsafe fn new`/`from_raw_parts`. A userspace `std` program reads them back through the identical
  accessor. So the same unsafe source is real in both roles, and folding it into either side would
  overcount one and undercount the other.
- Some shared blocks may run from the kernel only in its own `#[cfg(test)]` oracles. Several of these
  crates' `Cargo.toml` comments say their kernel-side use is exactly that: a test predicting what a
  client sent, which would never ship. Telling that apart per call site is a source-level read this
  pass did not do, recorded as future work.
- boot chain (37 blocks): `uefi_loader` and any `crates/` member only it links (none today;
  `sealed_pair` is its build script's dependency, which runs on the developer's machine). It runs
  once, before the kernel starts, with the full privilege of the pre-OS environment, to decide which
  kernel image gets control. Its memory is gone by the time the kernel's isolation boundary exists.
  That is a chain-of-trust question, not a runtime-isolation one, so it is reported on its own. Which
  claim it backs is an architect's to decide.

### Which number the ceiling should be held against

`script/lint`'s `<!--count-at-most:unsafe-density-outside-arch-->` (`notes/unsafe-obligations.md`)
holds the mixed density: currently 88 (2026-09-24) against 77. This pass does not change that
marker's value, because a gate's threshold is an architect's. What the ceiling would mean under each
candidate:

- 142, held against the kernel-only density. This answers "how much of the code nothing confines is
  unsafe".
- 120, against the userspace density, the confined population. A ceiling matters less there, because
  a bug there is a bug in one program, not in the base.
- 77, the status quo, which answers neither question precisely because it is built from both.

Recommendation: the kernel density is the one worth a ceiling of its own, because the mixed number's
blind spot lives there. But a ceiling set on it starts cold, with no history of it moving
deliberately. That decision is an architect's to make with these numbers in hand.

Cinched 2026-10-08 (UTC) at the merge of milestone 139 (drive the unsafe count down) round 10: the
marker lowered 72 to 66 over the merged tree's 59.7 (1,029 blocks over 172,506 lines), keeping the
milestone's seven points of headroom. The open question above, which population the ceiling should
measure, is untouched; the cinch moved the number, not the population.

Cinched again 2026-10-12 (UTC), 66 to 61, with no reduction behind it: the tree had grown to 61.09
(1,103 blocks over 180,545 lines), and the ceiling now sits at its truncated value with no
headroom. [ceilings.md](../unsafe-obligations/ceilings.md) records why and what it costs.

### How each crate is placed, and every week restated

`helpers/trust_boundary_crates.tsv` gives each `crates/` member its side, generated from `cargo
metadata` by `helpers/trust_boundary.py` (the rule is in its docstring). `script/lint` regenerates it
and fails when the checked-in file differs, and the pre-push hook fails when a member has no row.
A crate that leaves the tree keeps a `retired` row with the side it last had, so the weeks that held
it still place its unsafe.

`script/metrics` cannot run cargo, so it applies today's table to every historical week. That is
the restatement trade `MILESTONE_STATUSES`/`NAME_STATUSES` make. A crate whose side changed over
time is counted on its current side in every week. `unsafe_trust_unclassified` reads 0 in every
week on record. It counts a `crates/` directory the table has no row for, which lint refuses on
today's tree.

The retired rows were seeded once from `cargo metadata` run at each charted week's own commit, all
fifteen of which still resolve offline. A crate that was linked and then fell out of use before
its deletion (`heap`, `slab`) keeps the side it had while linked.

### The two-day loss, restored 2026-09-23

The `unsafe_trust_*` columns landed on `main` at 2026-09-20 23:30 (`3764a78d3`), with the ten weeks
already backfilled to the numbers in this section. `weekly.csv` was then the single series file;
milestone 581 (one metrics file per measure) has since retired it. The next commit to touch it was
`6e8934474` ("Rebuild the weekly series after the merge", 2026-09-21). It resolved a conflict between
two branches that had each added columns to the same file. Its message says the plan was to restore
its own branch's cost columns and "let `script/metrics` add the trust-boundary columns from the
merged script."

What ran next was `--update`, not `--backfill`. `--update` only recomputes the current week and any
week missing outright. So it added the ten new column headers to every row, but left these empty,
not zero, for 2026W29 through 2026W38: `unsafe_trust_kernel`, `unsafe_trust_kernel_code_lines`,
`unsafe_trust_kernel_density`, the userspace triple, `unsafe_trust_shared` and
`unsafe_trust_boot_chain`. That is why `unsafe-trust.svg` drew one point instead of ten from
2026-09-21, until this was found and `script/metrics --backfill` was rerun on 2026-09-23.

The nine restored values match the 2026-09-20 backfill exactly. The `unclassified` counts for
2026W29 through 2026W37 are the same 57-to-188 figures, and 2026W38 is still exactly 0. So this is
the known rename-alias gap reappearing, not a new one.

The lesson is about the merge, not the census. When one branch adds a column while another is
independently adding columns to the same CSV, the resolved merge needs a `--backfill`. `--update`
treats every already-present row as correct, and will not notice blank cells a merge introduced.

The same shape was checked for elsewhere on 2026-09-23 and found nowhere else. Every other column
that could go this route was in fact backfilled at the commit that added it: `milestones_superseded`,
`milestones_refused`, and the fatal-risk columns, then `fatal_risks_tested` and
`fatal_risks_untested`. The flow columns (`milestones_built_this_week`, `merged_pull_requests`, the
four cost columns) are recomputed for every row on every run regardless of mode. They cannot hold a
stale blank this way.

## Adding a measure to the register, worked on the unsafe census

Moved verbatim from the register's EXAMPLES under §212 (a prose budget) on 2026-10-13 (UTC).

The unsafe census went from calef's question to a gated row in four steps.

1. Apply the test. Unsafe is where verification stops, so something depends on it. 42 non-merge
   commits changed it in fourteen days, so it moves.
2. Take the number more than once. A single measurement cannot tell a direction from a level:

   ```sh
   # blocks outside kernel/src/arch/, at four points in the tree's history
   2026-07-15   171 blocks in   7,508 lines   227.8 per 10,000
   2026-08-04   728 blocks in  58,351 lines   124.8 per 10,000
   2026-08-16   817 blocks in  73,129 lines   111.7 per 10,000
   2026-08-18   747 blocks in  80,359 lines    93.0 per 10,000
   ```

   The count more than quadrupled and the density more than halved. A ceiling on the count would
   have fired on nearly every lane.
3. Choose the relation from the shape of the quantity. Equality for a census somebody maintains,
   `count-at-least` where a deletion is the bad event, `count-at-most` where a drift up is. See
   notes/counted-claims.md.
4. Watch it fail. Add one `unsafe impl Send` anywhere and run `script/lint`:

   ```sh
   $ script/lint
   lint: a counted claim disagrees with the tree:
     notes/unsafe-obligations.md:461: claims at most 17, the tree has 18
   ```

   The density ceiling's first marker never fired. Written as `at most 91 blocks per 10,000
   lines`, it bound to the line's last number and compared 10,000 against 92. A marker now sits
   right after its own number.

## The ceiling's failure message, in full

The worked example above shows the first lines of what `script/lint` prints when one
`unsafe impl Send` is added past the ceiling. This is the whole message, as recorded when the
ceiling was 17.

```sh
# add one `unsafe impl Send` anywhere, then:
$ script/lint
lint: a counted claim disagrees with the tree:
  notes/unsafe-obligations.md:461: claims at most 17, the tree has 18
  (unsafe-thread-safety-claims: how many `unsafe impl Send`/`Sync` claims the tree makes, each one
  a hand-written assertion that the compiler is wrong about a type). A ceiling is only wrong when it
  stops being true, so this means the count went UP by 1 past the headroom. Take the addition back
  out, or raise the ceiling in this commit and say beside it why the addition was worth it
```

## BUGS

- **`patches/std-nife/overlay/` is outside the unsafe census, and it is our code.** Thirty-seven
  `unsafe {}` blocks in the `std` platform layer are counted by nothing here. There are two reasons,
  and only the first is a decision. A ceiling asserts a direction, and that code implements `std`'s
  internal interfaces. It cannot be restructured to hold fewer unsafe blocks without diverging
  further from the crate we track.
- The second reason is worse and is not a decision at all. That code is compiled into `std` by the
  farm and never by a clippy configuration here, so `undocumented_unsafe_blocks` and
  `unsafe_op_in_unsafe_fn` do not reach it either. Fifteen of its blocks have no `SAFETY:` comment
  in the form the lint wants, and nothing has ever said so. That is a coverage hole in the lint
  policy rather than a gap in the register, and it wants a lane.

- Unsafe density can be diluted by writing more safe code, and nothing stops that. The denominator is
  non-blank lines after comments and string literals are stripped, so prose cannot move it, but a
  verbose safe refactor can. The counter-argument is that the effect is small at 80,000 lines, and
  that the alternative, a raw count, was measured and is worse. Watch the numerator, which
  `script/lint` prints beside the ratio for exactly this reason.

- The unsafe derivation is a text scanner, not a parser. It blanks comments and literals with a regex
  before matching keywords, which keeps fourteen `unsafe {}` written inside doc examples out of the
  count. Block comments are matched non-greedily, and Rust's nest. The tree has no nested ones, and a
  nested one could only make the count too high, which fails loud. The same caveat applies to
  `script/lint`'s `# Safety`, dead-code and `#[path]` checks, which are built the same way.

- Nothing here measures the verification argument, and nothing can. Unsafe density says how much code
  is outside the compiler's guarantees. It says nothing about whether the invariants written in the
  `SAFETY:` comments are true. As §61 (a lint is adopted on evidence from this tree, not on its
  description) already records, a lint checks that a comment exists and never that it is right. A
  register of numbers is not a substitute for reading them.
