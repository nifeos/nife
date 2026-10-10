---
status: IN-PROGRESS
raised: 2026-10-06
branch: milestone/855-the-tls-graph-enters-the-gated-build
promoted_from: the-tls-graph-enters-the-gated-build
milestone_dependencies: 501
decision_dependencies: 196, 198
machine_requirements: none
specific_machine: none
needs_person: no
---
# 855. The TLS graph enters the gated build

*(Minted 2026-10-09 (UTC) by lane/promote-proposals from the proposal `the-tls-graph-enters-the-gated-build`. The number is provisional until the merge queue lands it; the title and slug are drafts.)*


Raised 2026-10-06 (UTC) by the lane for milestone 501 (a TLS client that speaks to one pinned
peer). It built the client and found that nothing checks it keeps working.

## The gap

`cryptography_provider`, `pinned_tls_client` and the two programs over them are each their own
workspace. Their crates are fetched only when somebody runs `helpers/build-cryptography-exerciser.sh`
or `helpers/build-pinned-tls-exerciser.sh`. Their kernel tests skip in CI. Their host tests run in no
gate at all.

That was milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets)'s
posture, chosen while the primitives were still calef's to rule on. They are ruled now. §196 (nife
carries TLS) took `rustls`, and §198 (the glue is ours, the primitives are not) took the primitives,
`rsa` included. Milestone 801 (packages over the internet) cannot ship a client no gate builds. A
toolchain bump or a `std` overlay change can break this graph today, and no check would notice.

## What it would cost, measured 2026-10-06 on patagonia

- Crates: 70 in `pinned_tls_client`'s normal graph, 73 in `pinned_tls_exerciser`'s
  (`cargo tree -e normal`). `deny.toml` and `script/supply-chain` already scan the provider's.
- Build time: about 28 s per architecture for `pinned_tls_exerciser` from clean. The `std` farm
  was already built. Under 90 s for all three.
- Run time: The kernel test is under 5 s per architecture. The host tests are under 2 s.
- Image size: 1.27 MB (x86_64) to 1.89 MB (riscv64) per program before the archive strips it.

Reuse: the two build helpers as they are and CI's existing crates.io fetch; nothing here is
written, and the crates are the ones §196 and §198 already took.

## The options

1. Build both programs in CI's kernel legs and the host tests in `script/test`'s host phase,
   fetching from crates.io as the rest of the workspace does.
2. The same, vendored: §46 (thin primitives or whole subsystems) says to vendor only what needs a
   patch, and nothing here does.
3. A scheduled, non-gating job: the graph is built nightly and a break is a notice rather than a
   red pull request.
4. Leave it out until milestone 801 needs it.

The lane recommends the first. The second buys nothing §46 asks for. The third finds a break
days late, on nobody's pull request. The fourth leaves the client unchecked through the toolchain
bumps most likely to break it.

## Ruled 2026-10-10 (UTC): option 1

calef launched this milestone on 2026-10-10 (UTC) with "launch 855". Option 1 was the lane's
recommendation and he raised no objection, so option 1 is the ruling; options 2 to 4 were not
contenders. The fork was this block's alone, so the ruling lives here. The dependencies it rests on
were already ruled: §196 (nife carries TLS) and §198 (the glue is ours, the primitives are not),
which are this block's decision dependencies in place of `unwritten`. No new section was written,
as none was for milestone 121 (`ripgrep` on nife)'s crates.io fetch in `swish-check`.

## Built (lane `milestone/855-the-tls-graph-enters-the-gated-build`, PR #1902)

- The kernel legs. `cargo xtask test` builds the TLS graph's programs for every leg it boots,
  after `std_exerciser` and before the archive (`xtask::farm::tls_graph`, provisional). It runs the
  two helpers as they were, with `NIFE_CRYPTO_TRIPLES` set to the legs in the run, so `--arch
  riscv64` pays for one triple. That puts `cryptography_exerciser`, `pinned_tls_exerciser` and
  milestone 801 (packages over the internet)'s `package_fetch_exerciser` in every suite archive,
  and their three tests stop skipping in `script/test` and in CI's `test` job alike. A build that
  breaks fails the gate rather than turning three tests into skips.
- Not a `script/ci-build` row, which is where `rg` is built. A row would have made `script/test`
  and CI's kernel legs two different suites, and would have needed a CI-only refusal to keep the
  skip from coming back; building inside `test` needs neither. The CPU-model matrix and the
  falsification replays go through `cargo xtask test` too, and build their one triple.
- The host phase. `cryptography_provider` and `pinned_tls_client` run their own tests, the
  client's against `helpers/tls-peer` (OpenSSL through Python's `ssl`), each by `--manifest-path`
  and `--locked`, beside `redoxfs_server`. The one test that needs the internet stays `#[ignore]`d.
- The helpers pin `CARGO_TARGET_DIR` to the package and build `--locked`: the copy step reads
  `$SRC/target`, and a gate builds the graph its lockfile names.
- `script/supply-chain` scans `pinned_tls_client` and `pinned_tls_exerciser` beside the
  provider. Both were clean against `deny.toml` on 2026-10-10.

## Architectural parity

All three architectures, by the same suite: each leg builds its own triple and boots its own
archive. Nothing here is per-ISA code.

## BUGS

- `package_index_tests`' falsification is attested, not replayable. When it was attested no sweep
  built the TLS graph; the suite now does, so a patch (`package_index::public_address` admitting
  every address) can be written and replayed. Owed by whoever next touches that test.

## Follow-on

- **Milestone 801.** Its whole-fetch gate runs in CI from this merge; its `BUGS` line is removed.
- **Milestone 501.** Its "absent from CI" `BUGS` line is removed; the host tests it named run in
  the host phase.

## Index row

The TLS graph enters the gated build: both exerciser programs build in CI's kernel legs, their host tests run in `script/test`'s host phase, and the crates fetch from crates.io as the rest of the workspace does. Milestone 801 (packages over the internet) cannot ship a client no gate builds.
