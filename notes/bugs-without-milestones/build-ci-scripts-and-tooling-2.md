# Build, CI, scripts and tooling (part 2 of 3)

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  notes/cryptography-provider.md | `script/crate-probes` may use the wrong toolchain (in-repo build, RUSTUP_TOOLCHAIN only); the fifty probes and the 43/7 split of milestone 64 (Enough `std` to run somebody else's crate) were never re-run | Re-run the fifty crate probes under the right toolchain | S  |
|  notes/delegated-review.md | Cost of the 55 delegated reviews was not measured, so the adopt case has no price | Record what a delegated review costs | S  |
|  notes/denominators.md | "// true" that swallows an outcome passes lint; selectors inside run: blocks are ungated (milestone 401 (A gate that selects the set it judges can pass by checking nothing), BUILT, called an unclosed convention) | Gate "// true" and run:-block selectors | M  |
|  notes/doc-coverage.md | A crate can drop `#![warn(missing_docs)]` by deleting a line; new crates not required to adopt it. Cost is a second clippy pass | Lint requires `missing_docs` on clean and new crates | M  |
|  notes/empty-selectors.md | `--exclude` and `.cargo/mutants.toml` exclusion globs that match nothing are silently accepted | Fail on stale exclusion globs | M  |
|  notes/falsification.md | A harness's model of its caller is never re-read; stays green after the kernel drops the guard | Expire or re-check harness caller models | M  |
|  notes/falsification.md | `falsifications.yml` concurrency group has no ref, so a hand dispatch cancels another branch's sweep | Per-ref concurrency for falsification dispatch | S  |
|  notes/falsification.md | No lint requiring `kani::cover!`; 23 sites in 4 of 24 harness crates ("wants a lane") | Vacuity guard for Kani harnesses | M  |
|  notes/falsification.md | Kernel `#[test_case]` falsifications are recorded but unswept; milestone 210 (No kernel test can be run by name, so one falsification costs a whole suite) landed, "a lane rather than a limitation now" | Weekly sweep replays kernel test_case falsifications | M  |
|  notes/framebuffer-contract.md | Referee's red report says "the compositor test passed" for every unmatched picture even when the guest panicked first | Scanout referee stops asserting the compositor test passed | S  |
|  notes/host-recovery/devices-and-partitions.md | Backup GPT is not consulted when the primary header is damaged | Fall back to the backup GPT | S  |
|  notes/host-recovery/devices-and-partitions.md | Recovery tool never run against a real raw device (`/dev/rdiskN`, zero metadata length) | Run the recovery tool against a real raw device | S  |
|  notes/instruction-clock.md | `run_bench` has the same blocking-read exposure as the icount path (emulator leaked twice) and was not given the watchdog | Watchdog for `run_bench` | S  |
|  notes/interface-stability.md | items reached only through a glob re-export are unseen (domain_name_resolution_contract, user_mode_runtime are unmeasured) | Expand glob re-exports in the interface fingerprint | S  |
|  notes/interface-stability.md | nothing checks that a ledger entry exists when an encoder's wire layout changes with no public item changing | Fail when a wire-layout change has no ledger entry | M  |
|  notes/job-mix/null-syscall-under-load.md | `script/fastpath-footprint` leaves `exception_body` out of aarch64's `syscall_entry` set (related to 0368, which is about a flat set, not this omission) | Add exception_body to aarch64's footprint entry list | S  |
|  notes/kernel-proofs.md | `script/verify` kernel row seconds is a dev-Mac number; replace from the first CI log | Replace the kernel row timing with CI's | S  |
|  notes/kernel-proofs/riscv64-with-a-patched-kani.md | `script/falsifications --sweep` uses stock Kani so riscv64-only harnesses can only be `attested` | Teach falsifications --sweep the patched riscv64 Kani | M  |
|  notes/load-sensitive-assertions/inbound-eintr.md | source of the signal that raised EINTR is unknown; other blocking calls in xtask could see it | Identify the EINTR source in the inbound prober | S  |
|  notes/main-is-red.md | a PR opened or marked ready during a hold is not held; nothing re-runs `hold` on a timer | Hold sweeps late arrivals | S  |
|  notes/memory-ordering.md | `PAIR:` gate passes a marker naming a nonexistent or wrong function | Check that a PAIR marker names a real function | S  |
|  notes/merge-queue.md | drain's surface pass judges a new PR against its own checkout's `main`, giving a false `needs-architect` | Drain surface pass uses the PR's real base | S  |
|  notes/merge-queue.md | `Blocked-by:` holds the drain not the queue, reads only the first line, matches inside code spans | Make Blocked-by hold the queue and parse properly | S  |
|  notes/merge-queue.md | queued-push refusal misses cloud lanes (needs a server-side workflow) | Server-side queued-push check for cloud lanes | M  |
|  notes/merge-queue/prose-only.md | a script run only by a skipped job that walks a prose directory could slip through; nothing checks it later; also `fetch-depth: 0` follow-up unmeasured | Verify prose-only skip against walked directories and trim fetch-depth | S  |
|  notes/model-comparison/2026-09-27-qwen-27b-122b-bakeoff.md | gate lock cannot name "this rename touches gate machinery on purpose" short of listing every path | Gate-lock target by glob for rename tasks | S  |
|  notes/model-comparison/2026-09-27-qwen3-coder-bakeoff.md | open-lane loop exited 1 after round 1 without its "not green" message, cause undiagnosed | Diagnose the open-lane silent exit | S  |
|  notes/model-comparison/2026-09-27-qwen3-coder-bakeoff.md | `helpers/open-lane.sh` maps only the main model, so side requests for `claude-sonnet-5` get a 400 per run | Map the small-model slot in open-lane.sh | S  |
|  notes/mutation-census.md | nothing checks a `--add` row was ingested from the run it names | Verify census row against its run id | S  |
|  notes/mutation-testing/census-design/roadmap/2026-09-21-triage.md | coverage and mutation file-level exemption lists agree only by "change both together" (drifted once); the "carried" 152 survivors are inferred not re-derived | Derive the file-level exemptions; re-derive carried survivors | M  |
|  notes/observed-failing.md | `--verify-runs` is never scheduled so a mistyped run id passes lint; nothing forces a bootstrap record to be replaced | Schedule --verify-runs and flag bootstrap records | S  |
|  notes/open-model-lanes.md | `OPEN_LANE_EFFORT` default unmeasured through the gateway (does the flag survive `drop_params`) | Measure effort flag through the gateway | S  |
|  notes/overflow-checks.md | `cryptography_exerciser` built only by hand, so CI never runs it with checks on | Build cryptography_exerciser in CI with checks | S  |
|  notes/packages/jig.md | nothing gates jig's dependencies to the public contracts | Gate jig dependencies to contracts | S  |
|  notes/project-metrics/limitations.md (CSV merge bullet) | Nothing flags blank cells in past rows after a metrics CSV merge resolved with `--update` | `script/metrics --check` refuses empty cells in historical rows | S  |
|  notes/push-while-queued.md (both paragraphs) | Queued-branch push check is hook-only (cloud sessions, fresh clones, `--no-verify`, draft conversion all bypass it); no server-side check exists | A server-side workflow that comments when a queued PR's head moves | M  |
|  notes/qemu.md (first bullet) | Pinned QEMU cannot be built on macOS by `script/ci-qemu` (configure needs `--disable-cocoa --disable-pvg` and Homebrew python); a shared `$HOME/.cache/nife-qemu` changes the emulator under every checkout | Build the pinned QEMU on macOS through the script, and scope the cache per checkout | M  |
|  notes/register-of-measures.md (third bullet) | Nothing checks that gated/dated register rows still name a real script and command | A check that every register row's instrument still exists | S  |
|  notes/register-of-measures/code-proofs-and-coverage.md (first bullet) | Coverage floor backfill has no command; it is a hand recipe | `script/metrics --coverage-backfill` as a real command | S  |
|  notes/ripgrep-on-nife.md (first bullet) | Upstream issue for the `ignore` parallel-walker bug is "link pending" | File the upstream `ignore` issue for non-unix/windows targets | S  |
|  notes/scripts/outsider-passes.md (machine check bullet) | Machine check reads the first `podman machine inspect` result, not the default connection | Outsider-pass launcher checks the default podman connection | S  |
|  notes/soak.md (last bullet) | Nothing compiles the soak build in `script/test`, so it can rot unnoticed | `cargo check` the soak build in lint or test | S  |
|  notes/stack-high-water.md (first bullet) | Per-IPC depth instrument copies two fixtures' role numbers instead of reading a crate | Per-IPC depth reads role numbers from the fixture crate | S  |
|  notes/stack/progenitor-stack.md (release + interrupt_ignorer bullets) | No CI row runs `swish-check --release`; its one local run failed `interrupt_ignorer` on aarch64, uninvestigated | Add a `swish-check --release` row and find the `interrupt_ignorer` failure | M  |
|  notes/system-tests-and-the-kernel-crate.md (check 13, dead-code bullets) | Check 13 counts a cfg(test)-only use as production and ignores names in strings; x86_64 test build not dead-code gated | Tighten lint check 13; gate dead code on the x86_64 test build | S  |
|  notes/system-tests-and-the-kernel-crate.md (first bullet) | Five service crates remain kernel dependencies pending #1389's division | Divide the five service crates out of kernel dependencies | M  |
|  notes/trusted-base.md (six-category audit bullet) | "No allocator in kernel" and "no pointer crosses the boundary" are read, not gated; a new syscall could reintroduce either | Gate the trusted-base categories (allocator, pointers at the syscall edge) | M  |
|  notes/tsc-under-tcg.md (qemu-bounded bullet) | `helpers/qemu-bounded.sh` leaves `qemu-system-x86_64` orphaned when the output consumer exits early (milestone 226 (`qemu-bounded.sh` leaves an emulator behind, and the next run blames the wro) is BUILT) | Reproduce and fix the early-consumer orphan in `qemu-bounded.sh` | S  |
|  notes/uefi-boot-two-core-rate.md (+ notes/x86-uefi-boot.md "suite runs one core") | `uefi-test` leg still boots one core under OVMF; two-core suite is unmeasured | Flip the uefi-test leg to two cores, measure, keep what the number earns | S  |
|  notes/undefined-behavior.md | `cfg(miri)` sample domains have no completeness pin except `glob`, so they can shrink silently | Pin the size of every Miri sample domain | S  |
|  notes/undefined-behavior.md | Miri is weekly only; running it on a PR that touches `unsafe` is a habit, not a gate | Run Miri per pull request on crates whose unsafe changed | M  |
|  notes/unsafe-obligations.md | `script/lint` compiles `user`/`user_mode_runtime` for aarch64 only, so 9 of 14 sites are linted on one ISA | Lint the user crates on riscv64 and x86_64 | M  |
|  notes/user-proofs.md | `user`'s 3 s in `script/verify`'s table is a dev-Mac number to be replaced from a CI log | Replace the dev-Mac timing for `user` with a CI figure | S  |
|  notes/working-from-a-cloud-session.md | no cloud session has run `script/test` on the pinned QEMU; whether it can push outside `claude/` is unrecorded (two entries merged) | Record a cloud session's `script/test` run and push limits | S  |
|  notes/working-from-a-cloud-session.md | nothing reports a cloud container that died with unpushed work (no launchd-watcher equivalent) | Report a dead cloud session's unpushed work | M  |
|  notes/x86-uefi-boot.md | a hand-made stick can carry a stale `.efi` and nothing says so | Check a boot stick's `.efi` against the build | S  |
|  roadmap 0583 | gloss key ignores the letter, so milestone 19d (Run a real workload) rides on 19's gloss | The citation gloss key is letter-aware | S  |
|  roadmap 0587 | weak-host argument for the QEMU legs is recalled, never measured (needs a deliberately broken ordering) | A weak-host QEMU leg is proven by a seeded ordering bug | M  |
|  roadmap 0597 | nothing writes the note.o a foreign build must link to carry a note | A tool emits the manifest-note object | M  |
|  roadmap 0611 | dev-dependencies unchecked and per-program check reads source text (note's BUGS) | Package-boundary check covers dev-dependencies | S  |
|  roadmap 0623 | `script/metrics --selftest` not wired into `script/lint` | Metrics selftest runs in lint | S  |
|  roadmap 0630 | `--ready-branch` matches by branch name only | Ready-branch check verifies the block's PR | S  |
|  roadmap 0641 | a denominator label can be wrong; "// true" is labeled by family, not gated; exceptions stay quiet (177) | Denominator labels and "// true" are checked | M  |
|  roadmap 0721 | `verify.yml` jobs have no wall-time cap (no median measured) | Wall-time caps for verify.yml | S  |
|  roadmap 0722 | cost rows are means from two runs; need re-tightening against first medians | Tighten the swish-check cost rows to medians | S  |
|  roadmap 0723 | watcher check reads the last run, not the last success | The watcher check reads the last success | S  |
|  roadmap 0724 | helper reads newest 100 events; 3 PRs over 100 undercounted; trunk-health re-announces a bad day each run | Queue-share fetch pages all events; announce once | S  |
|  roadmap 0740 | two inflow keys are mixed (equivalent and killed mutants) and have no row | Triage the timetable mixed-mutant keys | S  |
|  roadmap 0749 | a test name matching any base test anywhere is not "new"; second same-named fn escapes | Falsification ratchet keys on path plus name | S  |
|  roadmap 0753 | the progenitor trace patch is not checked to still apply; riscv64 and x86_64 not traced | Gate the login-block trace patch; trace the other ISAs | M  |
|  roadmap 0755 (also 0810, 0916:101) | basalt gate is one job for three ISAs, a red arch hides the rest; a red before images keeps no artifact | basalt gate splits per architecture | M  |
|  roadmap 0798 | qtest materials live only on one machine outside any repository | Commit the PLIC qtest reproducer | S  |
|  roadmap 0807 | `--hvf` and `--test` runs not recorded (`time_record.rs`) | Record --hvf and --test runs | S  |
|  roadmap 0808 | `nanny.py` reads PRs only so budget issues reach a session late; `script/lint` times no check; two `gate` jobs share a key; fuzz job no step summary | Gate-time accounting gaps | M  |
|  roadmap 0855 | `package_index_tests` falsification attested, not replayable (owed) | Replayable falsification for package_index_tests | S  |
|  roadmap 0861 | workflow check proves YAML, not GitHub's schema; composite actions dir not walked | Workflow schema check | S  |
|  roadmap 0870 | `main-caches` runner pool vs restoring job's pool not compared | cache_save_scope compares runners | S  |
