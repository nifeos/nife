# Build, CI, scripts and tooling (part 3 of 3)

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  roadmap 0887 | a gate added to nife is not in the template table; nothing asks "and the template?" | Template drift check for new nife gates | S  |
|  roadmap 0889 | a gate split can drop a check; no one told a record waits; opt-in per clone; four stale `script/lint` mentions | Fleet-split gaps | M  |
|  roadmap 0914 | outsider pass "blind to history" is honor only; nothing enforces it | Enforce no-history for outsider passes | M  |
|  roadmap 0917 | reproducible only at `/tmp/basalt-build`; macOS link breaks match | Reproducible build independent of path | M  |
|  script/apropos | host `script/apropos` and the guest `apropos` builtin share one name by decision and nothing checks the pairing | Gate the apropos script and builtin names together | S  |
|  script/apropos | printed number is the raw count but results are sorted by density, so the list reads as unsorted | Print the sort key in `apropos` results | S  |
|  script/audits | nothing runs the audit-cadence script (not in lint or CI), so it broke for three weeks; wiring is left to an architect | Run `script/audits` somewhere that notices when it breaks | S  |
|  script/boot-check | only x86_64 boots a release kernel; release aarch64 and riscv64 never boot, so optimizer-exposed defects reach a bench first | Boot release aarch64 and riscv64 kernels in CI | M  |
|  script/build-is-reproducible | covers the aarch64 kernel only; not the user archive or the `nifefs` image that ships, not riscv64/x86_64 | Widen the reproducibility gate to the shipped artifacts | S  |
|  script/card-check | says nothing about the boot script or `extlinux.conf`, the other two ways a card can be wrong | Check the boot script and extlinux.conf on a board card | S  |
|  script/decisions | nothing checks `raised` precedes `decided` or that dates are plausible ("cheap to add") | Check decision frontmatter dates for order | S  |
|  script/falsifications | separate workspaces (`redoxfs_server`, `tools/redoxfs_host`, `vendor/redoxfs`, `fuzz`) are not swept or checked for rot | Sweep falsification records in the separate workspaces | M  |
|  script/fastpath-footprint (and :128 duplicate) | `arches` is a hardcoded string; entry says 186 owns the mechanism but 186 is BUILT. Built milestone, gap open | Derive `script/fastpath-footprint`'s architecture list | S  |
|  script/lint-fleet | nothing finds a fleet check that was never added to the selftest list | Lint-fleet finds its own orphaned selftests | S  |
|  script/mutation-census | `--add` and `--add-run` do not refuse a census built from a partial set of shards | Refuse partial censuses in mutation-census --add | S  |
|  script/preflight-queue (--act) | `--act` works from a stale queue snapshot and can comment on or dequeue a PR already handled | preflight-queue --act re-checks each entry before acting | S  |
|  script/stack-frame-check (exceptions) | hand-kept exception table; a function that moves from boot to thread stack keeps its exemption unnoticed | Check the stack-frame exception table against where each function runs | S  |
|  script/stack-frame-check (x86_64) | x86_64 left out of the default arch list because `iommu::init` is 12504 bytes; no decision on fix or exception (text may be stale next to stack-depth-check) | Decide the x86_64 iommu::init frame and put x86_64 in the default list | S  |
|  script/swish-check | a killed user thread is not a failure; the ratchet that fails on it is the "obvious next" one | swish-check fails on an unexpected killed user thread | S  |
|  system_tests/src/main.rs | rule that tests use only the test-access facade is prose; the facade re-exports whole modules | Gate system_tests to the test-access surface | M  |
|  xtask/src/boot_check.rs | emulator once found orphaned after many x86_64 boots, cause unknown | Find why boot-check leaves an orphaned QEMU | S  |
|  xtask/src/install.rs | gate uses one 1 GiB disk; layout floor/rounding and under-600 MiB refusal never exercised | Vary the install gate's disk size | S  |
|  xtask/src/swish_check/verdict.rs | per-echo 30 s budget flakes under load; real echo time under load never measured | Measure echo time under load and size the budget | S  |
