# Every pin in the tree, and what watches it

Appendix to
[the proposal](../every-pin-has-a-watcher-and-a-bump-carries-evidence-for-where-it-sits.md).
Measured on 2026-10-25 (UTC) against `main` at `e865d8838`, with #2714 applied for the lockfile rows.
The kernel's rows, under "The kernel's registry crates" and "The verification path", were read again
on 2026-10-26 (UTC) at `01b316db9`.

## The pins

| Pin | Where | Watcher | Cadence | What a newer release produces |
|---|---|---|---|---|
| root `Cargo.lock` | `/` | Dependabot | weekly | a grouped minor/patch PR; a PR per major |
| 13 workspace lockfiles | `c_library`, `cryptography_*`, `fuzz`, `jig`, `mdr`, `pinned_tls_*`, `redoxfs_server`, `std_exerciser`, `tools/*` | Dependabot (#2714) | weekly | one grouped minor/patch PR; one PR per major across workspaces |
| `vendor/redoxfs/Cargo.lock`, `vendor/talc/Cargo.lock` | `vendor/` | none, deliberately | none | upstream's lockfiles; `vendor-verify` refuses any change |
| measurement harness | `notes/packages/container-format-measurements/fuzz` | none, deliberately | none | a `Cargo.toml.in`; exact pins are what the note measured |
| GitHub Actions | `.github/workflows` | Dependabot | weekly | one grouped PR |
| `vendor/redoxfs.pin` | `vendor/` | `script/vendor-watch` | weekly | a draft PR raising the pin; upstream commit subjects |
| `vendor/talc.pin` | `vendor/` | `script/vendor-watch` | weekly | as above, plus the state of talc#57 |
| relibc seed `893a3b9133ac` | `vendor/relibc` | none | none | proposed: a report of upstream commits touching taken files |
| DejaVu 2.37 | `vendor/dejavu` | none | none | proposed: a report-only row |
| `rust-toolchain.toml` | `/` | `toolchain-bump.yml` | daily | a PR on a fixed branch; floors restamped when proven unmoved |
| `.kani-version` | `/` | `toolchain-bump.yml`, `propose-kani` | daily | a PR on a fixed branch |
| riscv64 Kani 0.67.0, patched | `script/verify-riscv64` | none | none | proposed: a report-only row |
| `.qemu-version` | `/` | milestone 943 (a new QEMU release is proposed as a pin bump), not started | none | planned: a PR on a fixed branch |
| `.cargo-deny-version` | `/` | none | none | proposed: `script/pin-watch` |
| `.cargo-fuzz-version` | `/` | none | none | proposed: `script/pin-watch` |
| `.cargo-mutants-version` | `/` | none | none | proposed: `script/pin-watch` |
| cbindgen 0.29.0 | relibc headers (`vendor/README.md`) | none | none | proposed: `script/pin-watch` |
| `ubuntu:24.04` | `.github/runner-container/Containerfile` | none | none | proposed: Dependabot's `docker` ecosystem, report only |

## The kernel's registry crates, by target

`cargo tree -p kernel --target <t> -e normal,build --prefix none --offline --locked`, keeping lines
without a path:

| Target | Registry crates linked |
|---|---|
| aarch64-unknown-none-softfloat | none |
| riscv64imac-unknown-none-elf | none |
| x86_64-unknown-none | `uuid` 1.28.0 |

`cargo tree -i uuid` on x86_64 gives the path: `uuid`, then
`globally_unique_identifier_partition_table`, then `boot_slot`, then `kernel`. `cargo metadata`
without a target filter reports 31 registry packages under the kernel, because it counts the
host-only `loom` and `tracing` graph behind `cfg(loom)`. The classes need the target filter.

## The verification path

Of the 37 crates in `script/verify`'s table, three reach a registry package by normal or build
edges on the host: `globally_unique_identifier_partition_table` (`uuid`), `kernel` (`uuid`),
and `components` (27 packages, among them `smoltcp`, `argon2`, `blake2`, `digest`, `heapless` and
`httparse`). The other 34 have no registry dependency.

## Counts

- Registry packages in the root `Cargo.lock`: 86 (`cargo metadata`, packages with a source).
- Distinct registry packages across every committed lockfile but `vendor/`'s: 204.

## What already runs on every pull request

From `ci.yml` and `verify.yml`, so on a Dependabot pull request too: Kani (`verify (Kani proofs)`),
the icount tripwire (`bench`), the boot file budget, the fastpath footprint, cargo-deny and
`vendor-verify` (`supply chain`), and every fuzz target for 60 seconds. The mutation gate runs
`cargo mutants --in-diff`, which finds no first-party lines in a lockfile-only change.

## Dependabot history

Every pull request Dependabot opened, to 2026-10-25: 26. Actions groups merged on the day they
opened. Cargo: `spin` 0.10.0 to 0.12.2 (#3) and 0.12.2 to 0.12.3 (#459), `smoltcp` 0.13.1 to
0.14.0 (#460) and `argon2` 0.5.3 to 0.6.0 (#596) merged the day they opened. `aarch64-cpu` and
`tock-registers` majors (#2, #4) closed because a hand commit had taken both together. The four
RustCrypto majors (#144 to #147) and their group (#151) closed; the group rule in
`.github/dependabot.yml` and §81 (a dependency stays upgradable) record that incident.
