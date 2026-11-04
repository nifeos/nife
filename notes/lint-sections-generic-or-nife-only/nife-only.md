# The nife-only sections of script/lint

*An appendix to [`../lint-sections-generic-or-nife-only.md`](../lint-sections-generic-or-nife-only.md). Name: provisional, minted 2026-11-03 (UTC) by lane/lint-section-tagging.*

Eighty-three sections read the kernel, the roadmap, the decisions, nife's own scripts or a table of nife files, so a crate repository would not inherit them. The number is the section's place in `script/lint`, counted by `helpers/lint-no-cargo.awk`. Seconds come from merge-group run 41694693300.

| # | section | what it checks | tag | what it reads | seconds (wall / CPU) |
|---|---|---|---|---|---|
| 4 | the heavy-job slots (a full machine holds a job back; a killed holder frees its slot) | the machine's heavy-job slots, in a scratch tree | nife-only | `helpers/heavy_slot.py` | 1 / 0.4 |
| 5 | the CI fuzz job keeps a finding private | the CI fuzz job and daily fuzz report keep a finding private | nife-only | `.github/workflows/ci.yml`, `xtask` | 3 / 4.1 |
| 6 | script/verify --parallel forks the shards, joins their verdicts, and keeps to one slot | `script/verify --parallel` shards | nife-only | `helpers/verify_parallel.py` | 0 / 0.0 |
| 7 | script/push believes origin's ref, not git's exit status | `script/push` reads origin back | nife-only | `script/push` | 0 / 0.1 |
| 8 | the rustup retry (a dropped CDN connection retries; a real error does not) | the rustup retry classifier | nife-only | a stub rustup | 0 / 0.1 |
| 9 | the continuous fuzz supervisor installs the pinned toolchain once, before any slot | the fuzz supervisor's one toolchain install | nife-only | `helpers/continuous-fuzz-selftest.sh` | 0 / 0.0 |
| 10 | the fuzz host follows only a green, fast-forward main and installs no unit file | the fuzz host follows only a green main | nife-only | `helpers/fuzz-follow-main.py` | 4 / 1.0 |
| 12 | clippy: redoxfs_host (its own workspace) | clippy on the RedoxFS host tool | nife-only | `tools/redoxfs_host` | 6 / 14.4 |
| 13 | clippy: glyph_atlas_generator (its own workspace) | clippy on the atlas generator | nife-only | `tools/glyph_atlas_generator` | 11 / 13.5 |
| 14 | clippy: redoxfs_server (its own workspace) | clippy on `redoxfs_server` | nife-only | `redoxfs_server/Cargo.toml` | 6 / 14.2 |
| 15 | clippy: fuzz targets (its own workspace) | clippy on the fuzz workspace | nife-only | `fuzz/Cargo.toml` | 12 / 37.9 |
| 17 | clippy: kernel + user + user_mode_runtime (aarch64) | clippy, aarch64 kernel and user | nife-only | `kernel`, `components`, `fixtures` | 6 / 18.9 |
| 18 | clippy: system_tests (aarch64) | clippy, aarch64 system tests | nife-only | `system_tests` | 5 / 4.6 |
| 19 | clippy: components --features confinement_attackers (aarch64) | clippy, confinement attackers | nife-only | `components` | 3 / 10.2 |
| 20 | clippy: kernel (x86_64, the binary only; see the note above) | clippy, x86_64 kernel binary | nife-only | `kernel` | 3 / 7.0 |
| 21 | clippy: kernel (riscv64) | clippy, riscv64 kernel | nife-only | `kernel` | 2 / 6.4 |
| 22 | clippy: kernel --features bench (x86_64, the binary only; see the note above) | clippy, x86_64 `bench` | nife-only | `kernel` | 4 / 7.0 |
| 23 | clippy: kernel --features cycle_counter_grant (x86_64, the binary only; see the note above) | clippy, x86_64 `cycle_counter_grant` | nife-only | `kernel` | 10 / 10.9 |
| 24 | clippy: kernel --features network_bench (x86_64, the binary only; see the note above) | clippy, x86_64 `network_bench` | nife-only | `kernel` | 2 / 1.7 |
| 25 | clippy: user + user_mode_runtime (x86_64) | clippy, x86_64 user | nife-only | `components`, `user_mode_runtime` | 33 / 49.9 |
| 26 | clippy: kernel --features watchdog_probe (x86_64) | clippy, x86_64 `watchdog_probe` | nife-only | `kernel` | 3 / 8.0 |
| 27 | uefi_loader: build the three kernels the boot files embed (see the note above) | builds the three kernels the boot files embed | nife-only | `kernel`, `uefi_loader` | 14 / 36.5 |
| 28 | clippy: uefi_loader --features uefi (x86_64-unknown-uefi) | clippy, `uefi_loader` x86_64 | nife-only | `uefi_loader` | 1 / 2.8 |
| 29 | clippy: uefi_loader --features uefi (aarch64-unknown-uefi) | clippy, `uefi_loader` aarch64 | nife-only | `uefi_loader` | 1 / 1.6 |
| 30 | clippy: uefi_loader --features uefi (riscv64imac-unknown-none-elf, the stick's PE payload) | clippy, `uefi_loader` riscv64 | nife-only | `uefi_loader` | 0 / 0.4 |
| 31 | clippy: uefi_loader --features uefi,screen_hold (x86_64-unknown-uefi, uefi-boot's build) | clippy, `uefi_loader` with `screen_hold` | nife-only | `uefi_loader` | 0 / 0.4 |
| 33 | rule 1 (arch code stays under kernel/src/arch/) | no `asm!` outside `kernel/src/arch/` | nife-only | `kernel/src`, `system_tests/src` | 0 / 0.0 |
| 34 | programs enter through program_entry! | programs enter through `program_entry!` | nife-only | `components/src`, `fixtures/src` | 0 / 0.0 |
| 35 | rule 5 (every arch:: path portable code names exists in every port) | every `arch::` path exists in every port | nife-only | `kernel/` | 5 / 5.2 |
| 36 | hand-written asm carries CFI | hand-written assembly carries CFI | nife-only | `kernel/src/arch/` | 0 / 0.0 |
| 37 | the custom-section link_section names are gated to the bare-metal target | custom `link_section` names gated to bare metal | nife-only | `kernel/`, `crates/` | 0 / 0.2 |
| 39 | the machine description is not gated on a boot mode | the machine description is not boot-mode gated | nife-only | `kernel/src/machine_description.rs` | 0 / 0.0 |
| 42 | the shell allocates only where it can fail | the shell allocates only in `heap.rs` | nife-only | `components/src/swish`, `crates/swish` | 0 / 0.0 |
| 46 | roadmap status | roadmap status | nife-only | `design/roadmap/` | 0 / 1.1 |
| 48 | decisions | decisions | nife-only | `design/decisions/` | 0 / 0.3 |
| 49 | journeys | journeys | nife-only | `script/journeys` | 0 / 0.1 |
| 50 | numbered record files carry four digits | numbered record files carry four digits | nife-only | `design/decisions/`, `design/roadmap/` | 0 / 0.0 |
| 53 | the publish allowlist | the publish allowlist | nife-only | `crates/publish-allowlist.txt` | 0 / 0.0 |
| 54 | citations | citations resolve | nife-only | `script/citations`, `design/` | 5 / 5.5 |
| 55 | new citations say what they cite | new citations carry a gloss | nife-only | `script/citations` | 1 / 0.8 |
| 56 | notes citing a milestone this branch moved | notes citing a moved milestone | nife-only | `script/citations` | 0 / 0.7 |
| 57 | fatal risks | fatal risks | nife-only | `design/fatal-risks/` | 2 / 2.6 |
| 58 | the host pass excludes exactly the bare-metal crates | the host pass excludes exactly the bare-metal crates | nife-only | `cargo metadata` | 0 / 0.7 |
| 59 | every crates/ member has a row in the trust-boundary table | every `crates/` member has a trust-boundary row | nife-only | `helpers/trust_boundary.py` | 0 / 0.0 |
| 60 | the trust-boundary table matches cargo metadata | the trust-boundary table matches cargo metadata | nife-only | `helpers/rust_source.py` | 1 / 0.6 |
| 61 | the unsafe listing of the crates the kernel links matches Cargo.lock | the kernel's dependency unsafe listing | nife-only | `notes/kernel-dependency-unsafe/` | 0 / 0.5 |
| 62 | mutation excludes every target the host build never selects | mutation excludes unselected targets | nife-only | `.cargo/mutants.toml` | 1 / 0.6 |
| 63 | mutation compiles every feature of the crates it scores | mutation compiles every feature | nife-only | `script/mutation` | 0 / 0.2 |
| 65 | the project's QEMU is on PATH | the project's QEMU is on PATH | nife-only | `helpers/qemu-path.sh` | 0 / 0.0 |
| 66 | the icount baselines name the pinned nightly | the icount baselines name the pinned nightly | nife-only | `helpers/lint_icount_baselines.py` | 0 / 0.0 |
| 67 | counted claims | counted claims | nife-only | `notes/counted-claims.md`, `kernel/`, `crates/` | 5 / 6.3 |
| 68 | naming conventions | naming conventions | nife-only | `components/`, `design/naming.md` | 0 / 0.1 |
| 70 | proof-only modules: crate::arch is the host's | proof-only modules: `crate::arch` is the host's | nife-only | `kernel/src/arch/` | 2 / 2.1 |
| 71 | names meet the current naming rules | names meet the current naming rules | nife-only | `helpers/name_checks.py` | 6 / 6.0 |
| 73 | the audit index | the audit index | nife-only | `design/audit-reports/` | 0 / 0.0 |
| 74 | the stranger test's run record | the stranger test's run record | nife-only | `script/stranger-test` | 0 / 0.1 |
| 80 | the fuzz heartbeat (selftest, and the workflow keeps to its limits) | the fuzz heartbeat keeps to its limits | nife-only | `.github/workflows/fuzz-heartbeat.yml` | 0 / 0.0 |
| 82 | the kernel's dependencies: nothing only its tests use | the kernel's dependencies | nife-only | `kernel/Cargo.toml` | 3 / 3.0 |
| 83 | test oracles stay dev-dependencies of the crate they judge | test oracles stay dev-dependencies | nife-only | `ORACLES` names `lamfat` | 1 / 0.1 |
| 85 | every userspace trap declares the registers the kernel writes | userspace traps declare clobbered registers | nife-only | `crates/`, `kernel/` | 0 / 0.5 |
| 88 | the removed-files index (design/removed.tsv: each path absent, each sha real, no retired number returns) | the removed-files index | nife-only | `design/removed.tsv` | 0 / 0.1 |
| 90 | the module-block ratchet (every module, document and pub item carries a Name: block; the unblocked list only shrinks) | every module and document carries a `Name:` block | nife-only | `design/naming.md`, a baseline | 2 / 1.6 |
| 97 | capability is not abbreviated cap (calef, 2026-10-06) | capability is not abbreviated `cap` | nife-only | `KEPT` names nife files | 3 / 3.0 |
| 100 | the outsider-pass floor checker and launcher selftests | the outsider-pass checker and launcher | nife-only | `notes/confinement-claims.md` | 3 / 3.1 |
| 102 | the prose-only classifier | the prose-only classifier | nife-only | `.github/workflows/ci.yml` | 0 / 0.0 |
| 104 | CI steps leave evidence when they hang | CI steps leave evidence when they hang | nife-only | `.github/workflows/ci.yml` | 0 / 0.1 |
| 105 | merge groups save no rust-cache entry; main-caches saves what jobs restore | merge groups save no rust-cache entry | nife-only | `.github/workflows/` | 0 / 0.1 |
| 107 | every mutation triage row names a package, through the rename table | mutation triage rows name a package | nife-only | `notes/project-metrics/` | 0 / 0.2 |
| 108 | the survivor classification is current | the survivor classification is current | nife-only | `notes/project-metrics/` | 1 / 0.2 |
| 109 | the shape-sites file is at most one census behind | the shape-sites file is current | nife-only | `notes/project-metrics/` | 0 / 0.1 |
| 110 | the mutation triage rows are classified and housed (a baseline that only falls) | mutation triage rows are classified | nife-only | `notes/project-metrics/` | 0 / 0.1 |
| 111 | the mutation baseline is the recorded census it names | the mutation baseline is its census | nife-only | `script/mutation-census` | 0 / 0.2 |
| 112 | every mutation exclusion names code that exists | every mutation exclusion names real code | nife-only | `.cargo/mutants.toml` | 0 / 0.4 |
| 114 | format values move with their version constants | format values move with version constants | nife-only | `crates/` | 1 / 0.1 |
| 115 | the first cut's programs declare what they link | the first cut's programs declare what they link | nife-only | `package.toml` files | 0 / 0.4 |
| 117 | externalized crates name no nife program | externalized crates name no nife program | nife-only | `components/`, `crates/` | 0 / 0.2 |
| 119 | dependency direction | no crate depends on a binary | nife-only | `crates/*/Cargo.toml` | 0 / 0.1 |
| 120 | package boundaries | package boundaries; three timing selftests | nife-only | `package.toml` files | 1 / 0.5 |
| 121 | the region claim protocol has one decision path | the region claim protocol's one decision path | nife-only | `crates/memory_regions/`, `kernel/` | 0 / 0.0 |
| 124 | every crate with proof harnesses is in the verify table | every crate with harnesses is in the verify table | nife-only | `script/verify` | 0 / 0.1 |
| 125 | every RETYPE outside the run wrapper passes 0 | every raw `RETYPE` passes 0 | nife-only | `*.rs` outside `kernel` | 0 / 0.0 |
| 129 | script/verify passes extra arguments through to cargo kani | `script/verify` passes arguments through | nife-only | `script/verify` | 0 / 0.1 |
| 131 | script/kani-install: an installed pin is not reinstalled | `script/kani-install` skips an installed pin | nife-only | `script/kani-install` | 0 / 0.0 |
