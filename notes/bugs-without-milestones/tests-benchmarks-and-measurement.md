# Tests, benchmarks and measurement

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  crates/paging/tests/mapping.rs | Frame-guard binding is opt-in in this file; the sibling fixture was converted so forgetting is a compile error, this one was not (21 call sites). | Hang mapping.rs table allocation off the guard like domain.rs | S  |
|  design/decisions/0055-shell-holds-the-redirect.md (last bullet) | Every byte crosses the shell's address space twice; nothing prices it. | Price the shell's double copy for redirects | S  |
|  design/decisions/0139-cycle-counter-authority.md (first bullet) | Cost of the added `msr` on the context-switch path is priced by shape, never measured (may be answered by 229 or 237; verify). | Measure the cycle-counter grant's switch-path cost | S  |
|  design/decisions/0145-compartmentalization-at-process-cost.md (66 us bullet) | The 66 microsecond figure is a spawn benchmark, not a disposable-domain measurement. | Measure a disposable domain, not a spawn | M  |
|  design/decisions/0244-the-installed-disk-has-four-partitions.md | The boot-only image size (about 1.7 MB) is an estimate nobody measures. | Measure a boot-only image | S  |
|  design/roadmap/0121-ripgrep-and-enumeration.md | Spawning `rg` takes up to 39 s under load and the walk 432 s on x86_64 TCG, unexplained; peak memory unmeasured | Explain rg spawn and walk time under load and TCG | M  |
|  design/roadmap/0121-ripgrep-and-enumeration.md | No threaded comparison with Linux; `rg_search` and `run_linux_rg.sh` both pass `--threads 1` | Threaded rg comparison against Linux | S  |
|  design/roadmap/0132-the-fastpath-footprint.md | U74 32 KB and TX1 48 KB cache figures are from general knowledge, not checked against the boards | Confirm target cache sizes against the silicon | S  |
|  design/roadmap/0164-x86-64-fs-server-aes.md | Soft-AES cost on x86_64 unmeasured | Measure soft AES cost | S  |
|  design/roadmap/0168-multitasking-benchmark.md | No HVF cross-check recorded of the curve's shape (runs since exist only at one to four tasks) | HVF cross-check of the multitasking curve | S  |
|  design/roadmap/0182-x86-64-interactive-boot.md | `uefi-test` fails with the RedoxFS disk attached (one run); not investigated | Explain the uefi-test failure with a disk attached | S  |
|  design/roadmap/0250-an-unviable-mutant-is-a-hole-that-reads-as-a-pass.md | functions cargo mutants generates no mutants for (a larger hole than unviable ones) are not counted | Count the functions with no generated mutants | S  |
|  design/roadmap/0254-a-caller-stranded-by-a-dead-server.md | MAX_THREADS scan and 128x16 capability sweep on teardown are unmeasured; no bench tears a server down mid-call | A teardown benchmark for server death | S  |
|  design/roadmap/0307-which-assertion-fires.md | row 19 attackers set escape bits only on success, so a fixture that stops probing reads as a clean refusal; needs a per-probe "attempted" bit (wire format) | `dir_attacker` reports which probes were attempted | M  |
|  design/roadmap/0415-sub-tripwire-drift-accumulates-across-baseline-saves.md | audit reads `xtask/src/restamp.rs`'s ledger by string shape; a changed marker reads a zero term silently, nothing compares the two | Audit and restamp ledger share one parser | S  |
|  design/roadmap/0420-the-rest-of-the-x86-64-fixture-set.md | mmio skip text "(NIFE_RNG not set?)" misleads on q35 where no value of `NIFE_RNG` can attach an mmio device | Fix the x86_64 mmio skip message | S  |
|  design/roadmap/0420-the-rest-of-the-x86-64-fixture-set.md | crash disk attached but no x86_64 crash-consistency arm; fixture only keeps ordinals aligned | x86_64 crash-consistency test arm | M  |
|  design/roadmap/0514-the-hvf-std-listener-hangs-in-some-full-runs.md | `std_exerciser`'s eight accept windows (120 s) exceed the 90 s test budget, so the last two never run on the HVF leg | Fit `std_exerciser` windows inside the test budget | S  |
|  design/roadmap/0514-the-hvf-std-listener-hangs-in-some-full-runs.md | other `drain_sink` callers still hang on an aborting writer because their starters do not hand back the writer thread | Starters hand back the writer thread to `drain_sink` | S  |
|  design/roadmap/0522-a-boundary-drawn-by-dependency-not-by-path.md | unsafe in the five `shared` crates is not attributed to a call site (kernel production, std program, or test-only) | Attribute `shared` crates' unsafe to its callers | M  |
|  design/roadmap/0541-a-timed-window-that-excludes-preemption.md | nothing executes `map_new`'s masked window under test, so removing the masking passes every test | A test that `map_new`'s window is masked | S  |
|  design/roadmap/0541-a-timed-window-that-excludes-preemption.md | `map_el0` times a mapping loop from EL0 with the same preemption fragility; needs a bracketing syscall or marginal-cost reporting (design fork) | An EL0 timed window for `map_el0` | M  |
|  fixtures/src/file_sink.rs (also file_source.rs) | `PAGE_VA` copied in every FS-frame program | One shared PAGE_VA | S  |
|  fixtures/src/job_mix_task.rs | grind may be optimized away, nothing checks; refused `CALL` shows only as a hang | Job-mix grind check; call status | S  |
|  helpers/kani_reach.py | cargo-mutants never mutates an `unsafe fn`, so proofs over Rendezvous send/recv etc. get no score; a hand-written mutant set was named as the way round and does not exist (milestone 741 (Does a standing proof notice a regression?) only recorded it; 780 covers zero-kill harnesses, not this) | A hand-written mutant set for the unsafe fns that proofs cover | M  |
|  kernel/src/arch/x86_64/pmu.rs | PMU never read on silicon; entry points at xenon but 309 is BUILT, so no milestone owns the xenon run | Read the x86_64 PMU on xenon and record the in-step ratio | S  |
|  kernel/src/job_mix.rs | a stray CALL on the report endpoint counts as a zero-job report and wedges the sweep; badge and shape are not checked | The job-mix report endpoint checks badge and shape | S  |
|  notes/bench-runbook.md (1st bullet) and notes/board-console.md (identity bullet) | a board's identity is recorded from its order and never checked; proposed `bench/<name>/identity/` provenance plus a lint, "wants a milestone number" | Bench machine identity carries evidence, checked by lint | M  |
|  notes/benchmarks.md (map_el0 bullet; also benchmarks/preemption-in-the-window.md) | `map_el0` has `map_new`'s preemption-in-window shape and was never measured for it | Measure `map_el0` for preemption in its window | S  |
|  notes/benchmarks.md (rfence_self bullet) | riscv64 `rfence_self` oscillates 5991 to 6476; cause (hart count or compiler) unsettled | Attribute the `rfence_self` oscillation | S  |
|  notes/benchmarks/drift-since-the-design/roadmap/2026-09-26-floors.md (bullets 2 and 3) | the +43 fixed term from 7a90fcc59 is unexplained; `spawn_reap`'s two causes do not add up (1,087 vs 612+881) | Attribute the residual drift in `spawn_reap` and the +43 term | S  |
|  notes/benchmarks/drift-since-the-design/roadmap/2026-10-10-floor.md (bullets 2 and 3) | `TPIDR_EL0` hand-over toll (~48 instr per switch) named not measured; riscv64 and x86_64 not bisected | Measure the remaining 2026-10-10 drift candidates | S  |
|  notes/cryptography-provider.md | TLS handshake/transfer cost measured only under QEMU TCG; nothing on radon or xenon, nothing separating aarch64 from riscv64 | TLS handshake cost on real boards | S  |
|  notes/effort-levels.md | Effort levels measured on Claude only, never the rented backend ("most consequential gap") | Repeat the effort-level benchmark on the rented backend | M  |
|  notes/futex.md | Wake placement on the waker's core (wrong for broadcast) is unmeasured | Measure futex wake placement for broadcast | S  |
|  notes/hung-component.md | The spinner hang shape (never `BLOCKED`) is untested; "easiest to detect" | Test the spinner hang shape | S  |
|  notes/hvf-leg.md | Nothing asserts the `test result: ok. N passed` string `hvf_kernel_leg` matches; a rewording makes the leg always fail | Assert the HVF verdict string exists | S  |
|  notes/hvf-leg.md | Failed HVF run leaves an exception storm; guest should park in `wfi` after a marker, host must stop on the marker (both halves together) | HVF guest parks after the semihosting exit | M  |
|  notes/interactive-stack-swap.md | A hung `line_editor` caused by the console can answer the replacement one flush early | Fence stale console acks across a swap | S  |
|  notes/mutation-testing/inflow-design/roadmap/2026-10-12-triage.md | `designware_ethernet` `prepare` keys survive because `sim::Memory` hides the read-back length | No-write-allocate mode for sim::Memory | S  |
|  notes/non-cooperative-fallback.md | steady-state cost of `broker` reading its warning page per request is unmeasured | Benchmark the real broker program | S  |
|  notes/overflow-checks.md | no release cycle measurement of overflow checks on riscv64 or x86_64 | Overflow-check release cycles on riscv64 and x86_64 | M  |
|  notes/ripgrep-on-nife.md (gitignore, thread count) | `ignore` matching never exercised (no ignore files in fixture); no gate counts the walk's threads at the prompt | Gitignore fixture and a thread-count assertion in `swish-check`'s `rg` walk | S  |
|  notes/ripgrep-on-nife.md (peak memory, memory grows with cores) | Walk peak heap unmeasured; 12 workers would want ~680 pages against a 384-page std region | Measure `rg` heap per worker and size `STD_REGION_PAGES` above 4 cores | S  |
|  notes/ripgrep-on-nife.md (spawn slow, x86_64 TCG walk, gap to Linux unattributed) | Where `rg`'s prompt time goes is unmeasured (shell staging vs progenitor hash; TCG; failed ignore-file opens) | Attribute `rg`'s prompt time: staging, hash, walk requests | M  |
|  notes/risk-6-bench-evening.md (Linux rate bullet) | Linux's 74 to 188 MB/s read rate and tail unexplained; nife's counter frequency unchecked against wall time | Run the baseline page's next checks: Linux rate and counter-vs-wall-clock | S  |
|  notes/scripts/one-kernel-test.md | `filtered_leg` starts no referee, so a filter selecting the host-process-connects test hangs to the 60 s watchdog | `--test` filter starts the referee its test needs | S  |
|  notes/tail-output-narrowing.md (second bullet) | Caretaker-hop race between a child's exit and its trailing delivery is argued, never measured | Measure how often exit races trailing sink delivery | S  |
|  notes/touch.md (host-check bullet) | Nothing outside the guest re-reads the image to check a node's stored `mtime` | Host check of the stored mtime after the touch probes | S  |
|  notes/walk-pricing.md | `rg` itself has not walked; the entry waits on milestone 205 (How a foreign program is told what to do), which is BUILT. Cited milestone built, gap still open | Run ripgrep's walk on nife and price it | M  |
|  roadmap 0598 | riscv64 ipc_rtt is bimodal ~0.6%, nightly restamp refused when mode flips; modes unattributed | Attribute riscv64 ipc_rtt's two modes | M  |
|  roadmap 0598 | icount differs between two worktrees of one commit (baked-in paths, not bisected) | Icount does not depend on checkout path | S  |
|  roadmap 0629 | x86_64 spawn_reap +1.74% under B not chased | Explain the spawn_reap layout shift | S  |
|  script/metrics (--check stale) | `--check` reports weeks.csv stale right after a commit because the row records HEAD's sha | Make metrics --check ignore the commit column it just wrote | S  |
|  script/metrics (churn cells) | model-line churn differs by a few lines between Actions and macOS; cause not found, pinned git flags untried | Find why model churn differs between runner and Mac | S  |
|  script/metrics (lcov) | `--coverage-for` takes the caller's word for which commit an lcov measured | Tie an lcov file to the commit it measured | S  |
|  script/stranger-test (container) | no container: cold-build half unmeasurable on a warm laptop, and a stranger can change machine-global state (brew, qemu cache) | Run the stranger test in a container | L  |
|  script/stranger-test (journal) | stranger's journal.md can land in the harness root so the printed summary path is empty | Find the stranger's journal wherever it was written | S  |
|  script/stranger-test (lint message) | `script/lint` in the clone prints a line that tells the stranger a key was withheld | Silence the lint line that announces the withholding | S  |
|  script/stranger-test (model) | `record.json` does not record which model ran; run 6 differed from 1 to 5 | Record the model in the stranger-test run record | S  |
|  script/stranger-test (tripwire) | `reached_withheld_note` fires on the bare filename; should compare note-only content or the `git show HEAD~1:` call | Make the withheld-note tripwire fire only on real reads | S  |
|  system_tests/src/user.rs | modules sorting after `thread_leak_police` (seven now) run after the probe and can leak unpoliced; nothing enforces order | Enforce that no test module sorts after the leak probe | S  |
|  system_tests/src/user/futex_tests.rs | wake is called from the kernel test because threads could not share a space; milestone 812 (BUILT) now allows two threads | Drive futex wake from a second thread in the same space | S  |
|  system_tests/src/user/login_user_schedule_tests.rs | one unexplained DENIED on the HVF leg (1 of 10 runs); not investigated | Run down the login DENIED seen once on HVF | S  |
|  system_tests/src/user/map_revocation_window_tests.rs | `DeviceFrame` arm of MAP_INTO reasoned, not driven | Drive the DeviceFrame arm of MAP_INTO under revocation | S  |
|  system_tests/src/user/revocation_in_flight_tests.rs | `DeviceFrame` and `PortRange` sweeps untested through a parked hand-off; `ipc_receive_cap` skips `port_range_grant` | Drive DeviceFrame and PortRange revocation through a parked hand-off | M  |
|  system_tests/src/user/revocation_window_tests.rs | `DeviceFrame` and `PortRange` take-backs reasoned, not driven | (same milestone as the two rows above and below) | S  |
|  system_tests/src/user/running_space_tests.rs (guard) | placement guard fails a correct kernel when both threads land on one core; should retry or skip | Make the cross-core TLB test retry placement | S  |
|  system_tests/src/user/spawn_mapping_revocation_tests.rs | `DeviceFrame::REVOKE` and `revoke_region` sweeps with a wired mapping not driven | (same milestone: one test for every revoking sweep) | S  |
|  system_tests/src/user/std_tests.rs | callers without the writer's thread still hang forever when a program aborts | Make every std test sink end on program abort | S  |
|  system_tests/src/user/syscall_fuzzer_tests.rs (corpus) | corpus seeds can rot green when the kernel's walk changes (seed 342); roadmap 0752 replay table still names 342 | Check that each corpus seed still reaches its defect | M  |
|  system_tests/src/user/syscall_fuzzer_tests.rs (model) | fuzzer does not generate REAP, SURVEY, MemoryRegion, AddressSpace, Timer, Irq, MAP/SLICE, BIND, death messages | Extend the syscall fuzzer model to the ungenerated calls | L  |
|  system_tests/src/user/thread_pointer_tests.rs | each thread has its own space for the same reason; 812 (BUILT) now allows one space | Test thread pointers with two threads of one space | S  |
