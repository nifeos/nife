# Security audit, 2026-10-26: the compiler

**Kind:** security. **Lens:** the compiler, read as code that runs with authority nobody reviewed:
the nightly pin and the path by which a new nightly reaches `main`, the workflow that bumps it, and
`patches/std-nife`, the `std` every std program on nife links, read as a confined program facing a
hostile counterparty. **Findings:** fixed 4, minted 1, accepted 3.

The finding to carry off is where the new nightly first runs. The bump workflow's `propose` job
mints the automation App's write token, checks the tree out with it, and then builds the std
overlay with a nightly installed minutes earlier and the build scripts of the registry packages in
that graph. Every later step can read the token, and the job arms its own branch to merge. Minted, because the fix is a job split that only a run on `main` can prove. In the overlay,
a path of two long names overran the page `std::fs` shares with the file server; fixed, with a check
in the std transcript.

## Why this lens

`script/audits` on `main` at `9ab1a3acf` reported both kinds DUE, each on one trigger: security on
milestones built 469 to 488 (+19, fires at 15), documentation on 466 to 488 (+22, fires at 10).
Issue #2750 is the scheduled workflow's repeat row. Its six reds from 2026-09-21 to 2026-10-26 each
printed `DUE: documentation, security` with counts that match the index rows on record at each run;
none was a broken run. This report closes the security kind. The documentation kind is still due
and wants a sweep of its own.

Two audits in a row (2026-10-19 and 2026-10-20) and the index's lens list named the compiler as the
lens no audit had taken: the nightly pin, `patches/std-nife` under `-Zbuild-std`, and
`toolchain-bump.yml`, which moves the pin unattended. The trigger that fired is a count, and it
points nowhere in particular, so the record's pointer decides.

### The two uncountable triggers, answered

Has a new component taken device or network authority? **No.** The component count is unchanged
at 236 since 2026-10-20, and the window's device and network merges (#2683, #2699, #2749) changed
records, not grants.

Has this booted on a new machine class? **No.** Milestone 890 (riscv64 and x86_64 join the shared
boot path) moved xenon onto the shared boot path, and xenon already booted.

## What was read, and what came back clean

**How the toolchain arrives.** `rust-toolchain.toml` pins `nightly-2026-10-26`. Every CI install
goes through `helpers/rustup-retry.sh`, which retries a failed download, including a checksum
failure, and so re-fetches rather than accepts bad bytes. The 2026-09-24 audit cleared `script/bootstrap`'s rustup installer. Every Actions job in this repository runs on a GitHub-hosted
runner, so a new nightly runs on no machine the project owns until a person pulls `main`.

**`toolchain-drift.yml`** installs the floating nightly with `contents: read`, no environment and no
secret. **The `restamp` job** builds under both nightlies with `contents: read`; what it hands
`propose` is an artifact that `propose` copies only into `bench/baseline-*.txt`, data that CI
checks. The `automation` environment deploys to `main` only, and `main`'s ruleset blocks deletion
and force-pushes.

**The two other App-token jobs** that check out with the token, `falsifications.yml`'s
`record-times` and `mutation.yml`'s `record`, run only this tree's scripts over artifacts and build
nothing, so finding 1 does not reach them.

**The overlay, at every counterparty value.** The argument reader walks every record against the block's capacity before it hands one out. The terminal's reply count stops at 8 and a sink chunk at 16. `READ` and `READDIR` clamp to the page, the entry reader slices with `get`, and `seek` checks its sum.
Socket receive lengths clamp to `DATA_MAX`, which with the payload offset is exactly the page; socket ids are local, and the PAL takes at most 8 resolver answers of 253 bytes. The futex passes only the
program's own atomics. `Layout` and `HEAP_MAX` bound the allocator's size classes and its growth. `Command::spawn` is upstream's `unsupported`, so std hands no capability to a child.

**Where the overlay could silently not apply.** Every overlay file is new to std, each hangs off a `mod nife;` the farm's patch places first in its dispatcher, and a missing anchor fails the build
("anchor not found"). No overlay file can drop out while the build passes.

## Findings

### 1. MINTED: the new nightly runs where the App's write token is readable

`propose` mints the App's installation token (contents and pull requests, write) and passes it to
`actions/checkout`, which persists it for later git commands in the job by default. The next
substantive step is `cargo xtask std-exerciser`: a nightly rustup installed in this job, plus the
build scripts in `xtask`'s and `std_exerciser`'s registry graphs (`rayon-core` and
`crossbeam-utils` among them). A compromised step can read the token, push to any branch and open a
pull request that triggers CI. The ruleset requires no approving review, and this job arms
auto-merge on `toolchain/nightly-bump`, so a push there merges once CI is green. A compiler already
owns what it compiles. What the token adds is commits on `main` beyond the build's output that no
person reads.

The 2026-09-24 audit recorded that the App token "is never written to a file"; for a checkout given
the token, that is not so. Proposed as [a nightly builds where no write token can be read](../roadmap/proposals/a-nightly-builds-where-no-write-token-can-be-read.md):
build in a job with no token, and push from a job that builds nothing. This lane does not fix it because the job runs only on `main`, so no pull request can prove a split, and a broken bump job is
the silence the 2026-08-01 fix in that file exists to prevent.

### 2. FIXED: the bump workflow took an unchecked dispatch input, and staged the overlay

A `workflow_dispatch` `target` went into `GITHUB_OUTPUT` unchecked, and later steps interpolate that
output into their scripts with `${{ }}`. A crafted input was shell in `propose` beside the App
token, and a newline in it wrote further step outputs. Dispatch needs write access, so this widened
a writer's reach to the App's identity rather than a stranger's reach to anything. Both plan steps
now refuse any target that is not `nightly-YYYY-MM-DD` before they write an output, checked by hand
against a good name, a short one, a quoted payload and an embedded newline.

The same job ran `git add -A patches/std-nife` before committing. No step edits the overlay (a
repair is a lane's commit, as the job's own body says), so the line could only carry what a build
step left there onto a branch armed to merge. It stages only the pin now.

### 3. FIXED: a path of several long names overran the shared file page

`count_names` bounds each name to the 4,096-byte page and not their sum. With two or more names
and a server that takes paths, `open_handle` and `dir_at` joined the whole path and `Page::put`
copied all of it into the one page at `FS_PAGE`; its safety comment said callers clamp, and these
two did not. Anyone whose string reaches `File::open`, `fs::read` or `read_dir` (a peer naming a
file) could send two 2,100-byte names. Today the next page up is unmapped, so the copy faults and
the kernel ends the process; a later mapping there would turn it into silent corruption.

Fixed: `joined` answers `None` past the page, and those paths take the hop-by-hop walk, which puts
one name at a time. `Page::put` now asserts its bound, so a future caller that forgets panics
instead of writing past the page. `std_exerciser`'s fs transcript gains `long path refused`, which
the kernel suite compares byte for byte. Every other `put` caller stays inside the page: single names, `File::write`, and both `rename`s.

### 4. FIXED: a huge thread stack size could round to zero

`Thread::new` rounded `Builder::stack_size` with `next_multiple_of`, which std passes through
unchanged. Without overflow checks a size near `usize::MAX` wrapped to 0, the allocator returned a
small block, and the new thread's first push wrote below it: undefined behavior from safe code.
Only the program chooses the size (the environment carries no `RUST_MIN_STACK`), and this tree's
release profile enables overflow checks, so here it panicked instead. A size that rounded to more
than `isize::MAX` without wrapping panicked in `Layout::from_size_align`'s `expect`. `Thread::new`
now builds the layout once, fallibly (`checked_next_multiple_of`, then `from_size_align`), refuses
any size with no layout as `InvalidInput`, and frees the stack with that same layout.

### 5. FIXED: three records said less, or other, than the code

- `SECURITY.md` counted eleven security audits and listed ten: the 2026-10-20 audit raised the
  count and added no entry. Both entries are there now, and the count is twelve.
- `std_runtime_contract::PROCESS_SLOT` said a program holds the slot "only if the program may have threads". The system initializer places it, with `BIND`, for every std program it starts. It
  widens nothing outside the process, since a joined thread spends the program's own table and
  budget, and the doc now says so.
- `notes/std/threads.md` recorded the spawned thread's missing guard page as a crash risk. It now
  says what it means for a parser of hostile input (finding 8).

### 6. ACCEPTED: the pin names a date, and a bump merges unattended

`rust-toolchain.toml` names a nightly, not a digest. rustup checks each component against its
date's manifest, fetched over TLS from `static.rust-lang.org`, and nothing here records what that
manifest said. The bump defaults to merge (ruled 2026-09-24 UTC), so a new nightly reaches `main`,
CI and every developer machine within about a day, and this tree's CI is its only reader. Accepted:
that is the trust any Rust nightly asks for, and a recorded hash would detect a swapped manifest
only after the build had run it. Recorded in
[notes/std/caveats.md](../../notes/std/caveats.md#what-the-pin-and-the-farm-trust) and
`notes/std.md`'s `BUGS`.

### 7. ACCEPTED: the farm's stamp does not hash what it patches

The stamp keys on `rustc -vV`, the overlay's files and `STD_SRC_PATCH_VERSION`, which a person
bumps by hand when the patch logic changes. A logic change without the bump leaves a warm farm on
the old std while the build passes, and the stamp skips a missing input. Accepted: CI builds every
farm cold, and the cure is `rm -rf target/nife-farm`. rust-src's integrity is rustup's check at
install. Recorded beside finding 6.

### 8. ACCEPTED: a spawned thread's stack overflows into the heap

Already in `notes/std/threads.md`'s `BUGS` as a missing guard page. Read through this lens, a
program that parses hostile input by recursion on a spawned thread turns deep input into heap
corruption rather than a fault. Accepted: the fix (a stack mapped from the region with an unmapped
page below) is the entry's own, and the entry now tells such a parser to bound its depth.

## Is any confinement claim false as stated?

No. Finding 3 corrupted, at worst, the memory of the process that made the request, and finding 4
needed the program's own choice. Neither crosses a capability.

## Fatal-risk implications

None. Finding 1 is about who can change `main`, not what a process on nife can reach, and none of
the nine risks names the project's own automation.

## What was deliberately not examined

- rustc itself, LLVM and upstream `std` outside the overlay; this audit trusts the nightly's code
  and reads only the path it takes into the tree.
- Kani, `.kani-version` and `propose-kani`'s `script/kani-bump`, beyond seeing that it compiles
  nothing; a verifier's pin wants its own reading.
- The C toolchain (`clang` from Ubuntu's archive) and relibc's build.
- basalt, which pins nife and builds the image. It is another repository.
- The documentation kind, which is still due.

## Method, so the negatives can be judged

Two readers, 2026-10-26 (UTC), on `main` at `9ab1a3acf`: one for the workflows, the pin and the ruleset (from the API), one for the overlay and the farm, each finding checked against the
code before it was dispositioned. The overlay change built under `cargo xtask std-exerciser`
(`std-aborts`: 25 process-ending bodies, all accounted for) and passed `script/test`.

## What wants a lane of its own

The proposal above, and the documentation sweep the cadence still reports due.
