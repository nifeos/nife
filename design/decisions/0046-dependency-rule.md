---
status: AMENDED
raised: 2026-07-30
decided: 2026-07-30
ratified_by: calef
---

# 46. Thin primitives, whole subsystems and everything in between: we take what meets our bar, and write with a reason

Ruled 2026-11-14 (UTC). The project does not set out to write every crate it uses. It sets out to
use only crates that meet its standards. That holds in every layer, the kernel and the crates Kani
proves included: a third-party crate that meets the bar below may be a dependency anywhere in the
shipping graph, and an architect rules on each one.

## The test, in order

1. Does it touch the ABI or a capability? Then write it. Nothing outside nife implements either, so
   there is no candidate to take.
2. Does a crate meet the bar? Then take it, in any layer. A whole subsystem goes the same way:
   smoltcp is the TCP/IP stack and RedoxFS the filesystem, each larger than the thesis it would
   serve, and confining them is the thesis.
3. Does a candidate fall short of the bar? Then contribute upstream until it meets the bar, and take
   it then.
4. Write our own only with a stated reason, recorded in the milestone block's `Reuse:` line.

The bar applies to every third-party crate a nife image links, including one already in the tree.
A shipped crate short of the bar stays in use, is listed as debt in
[notes/third-party-dependency-debt.md](../../notes/third-party-dependency-debt.md), and gets upstream
contributions until it meets the bar.
Host tools stay outside it: a crate that only a build or a test on the host runs is gated by
`deny.toml` and `script/supply-chain` alone, as §240 (TOML in xtask) records for `xtask`.

## The bar

A crate meets all four. The block or pull request that proposes it shows each one.

1. **A safety argument at every `unsafe` use.** Each `unsafe` block carries a comment that says why
   it is sound, and each `unsafe fn` and `unsafe trait` states its preconditions in a `# Safety`
   section. That is the rule nife holds its own code to (`clippy::undocumented_unsafe_blocks`, and
   milestone 112 (the SAFETY comments that bind nobody) for `# Safety`). The unsafe code a crate's macros generate counts as the crate's
   own. A crate with no `unsafe` meets this condition as it stands.
2. **Proofs matching ours.** Where nife would prove a property of this code with Kani, because a
   claim nife makes rests on it, the crate carries a proof of the same property. Where nife would
   prove nothing, the crate owes no proof. A proof here means a harness that runs in the crate's own
   CI or in ours against its public API. Adding harnesses to code that could have been taken does
   not put that code on the verification path after the fact: the block argues the claim.
3. **All three architectures.** It builds and works on aarch64, riscv64 and x86_64, for each
   target nife builds it for: §19 (architectural parity is a tenet). For the kernel those are the
   three bare-metal targets.
4. **Maintained.** Upstream releases fixes and answers reports. The proposal records the evidence:
   the last release, open advisories, whether the repository is archived, and how far the crate
   lags its own dependencies. A finished crate with no open defect can pass. A reviewer judges, and
   no date alone decides it.

`deny.toml` and `script/supply-chain` gate every crate as well: licenses, advisories, the
unmaintained list, yanked versions, crates.io as the only registry and one version per crate.
§81 (a dependency stays upgradable) holds for every crate taken.

## A crate short of the bar

We contribute upstream until it meets the bar: the safety comments, the harnesses, the restructuring
a proof needs, the port to a missing architecture. Writing is cheap for agents, so "we can write
it" argues for nothing. The same effort spent upstream leaves a crate that carries both our
standards and upstream's exposure, and third-party code that many systems run carries evidence of a
kind we cannot produce for ourselves. Fatal risks 3 (the tests do not test anything) and 7 (the
confinement claim is false) both come down to that gap.

The worked case is the kernel's device tree parser. Milestone 784 (reuse what the tree wrote)
measured our `device_tree_blob` against Google's `dtoolkit`. The four properties we prove hold on
`dtoolkit`'s code, and it survived our corpus and fuzzing. It fell short on a name-length refusal, a
quadratic walk and a panic in `Display`, so we filed the three reports and opened a contribution for
the walk. Retiring `device_tree_blob` for `dtoolkit` is an architect's ruling once `dtoolkit` meets
the bar.

### Stated reasons to write

The reason goes in the block's `Reuse:` line (or `## Reuse` section), which `script/roadmap
--check` requires of every block raised on or after 2026-10-05 (UTC). The line names at least one
candidate, or says `none exists` and what was searched. Reasons that hold:

- Upstream does not take our contributions, so a contribution cannot bring the crate to the bar.
  Redox is the standing case (below).
- A proof needs a restructuring that upstream will not take. A model checker often needs code shaped
  for it, and the shape can be a change to the crate's API. Register access is the ruled case:
  ruled 2026-10-26 (UTC), the kernel spells aarch64's system and MMIO registers in the tree and
  links no register crate, because a proof stubs an accessor in `arch/aarch64/instructions.rs` and
  cannot stub another crate's trait method. That replaces §3 (use the crate ecosystem) for register
  access.
- No candidate builds `no_std`, and making one do so is a fork we would carry forever.
- The license is incompatible. GPL code in the shipping graph is the question of §135 (running GPL
  software is aggregation), and §135 answers it with aggregation across a capability boundary, not
  linking.
- Bringing a candidate to the bar would cost more than writing, with numbers: lines to change
  against lines to write, or a measured performance gap. An adjective is not a number.

"Ours would be cleaner" is not on the list. If the honest reason is effort, the block says so in
those words (AGENTS.md, *Elegance and performance beat implementation convenience*).

## Where correctness is won by exposure, take it

A specification that is complete and checkable makes a reason to write easier to state, since a
proof over the whole input range settles correctness and nothing about wide use adds to it.

Cryptography is the opposite: take it, do not write it. Correctness there includes resistance to
attacks not yet published and side-channel behavior no specification states, and years of exposure
and review buy it. A proof that our AES matches the specification would not make it safe to use.
For the same reason condition 2 asks no proof of a cipher: nife would prove nothing about one.

So the distinguishing question is not size. It is whether the specification is the whole of
correctness.

## Taking is not vendoring

Whether we write or take is one decision. Whether a taken crate is vendored or depended on is
another, and it does not follow from the first. The trigger for vendoring is "we must patch it".
RedoxFS is vendored because it needed a divergence patch to build `no_std`, and
`script/vendor-verify` proves exactly that: upstream plus our recorded patches. smoltcp is a whole
subsystem and an ordinary dependency, because nothing needed changing.

Cryptography is an ordinary dependency, pinned in `Cargo.lock` and gated by `deny.toml` and
`script/supply-chain`. Advisories are the point in that category, and `cargo-deny` and `cargo-audit`
work against registry versions, so a vendored copy would hide an advisory until a person noticed.
Vendor only what must be patched.

## Who rules

Taking a third-party dependency is a decision (AGENTS.md's codebase rule 6). A lane proposes it in
its block and pull request, showing each condition of the bar, the pull request carries
`needs-architect`, and an architect rules. The conditions of §34 (RedoxFS is the primary
filesystem, on three conditions) apply to anything taken as a whole subsystem.

Redox is a source we consume and never contribute to. Its drivers are MIT-licensed, and its
CONTRIBUTING refuses LLM-generated contributions. So we take its code under the license and send no
code back. A patch we need is carried here (§34, `script/vendor-verify`), never offered upstream.
Bug reports to Redox are allowed: disclosed, each approved, security findings privately first (ruled
2026-10-07 (UTC) on #1815; §176 (offering the RedoxFS patches upstream)).

A block raised before 2026-10-05 (UTC) owes no `Reuse:` line, because a check must not fail the tree
it lands in. The check's comment marks that exception: an older block that is reopened does not owe
the line, and the check cannot tell.

## The kernel's bar

Ruled 2026-10-25 (UTC). Every crate the kernel links runs with full privilege and joins its trusted
code, so a crate the kernel links meets the bar above and these five conditions too. The block or
pull request that proposes it shows each one.

1. It builds `no_std` on all three kernel targets, and it needs no allocator or uses the kernel's.
2. If it parses input, it survives ours: the tree's fixtures and fuzz corpus for that input, with no
   panic, hang or out-of-range read.
3. Its dependency graph is small: it adds at most three new crates compiled into the kernel, its
   own crate included, as `cargo tree -e normal` counts them for each kernel target. Build-time
   crates (procedural macros, build scripts and what they pull in) have no cap, and the proposal
   lists each one. `Cargo.lock` pins every crate in the graph, build-time ones included, and
   `deny.toml` and `script/supply-chain` gate each one like any other dependency.
4. Its proposal lists the crate's `unsafe`: the count and location of its hand-written blocks, and
   the unsafe code its macros generate, which counts as the crate's own. For each, it names what
   checks it, upstream (Kani, Miri, fuzzing, a published soundness policy) or here. No cap applies.
   Any `unsafe` that a claim in nife rests on falls under condition 2 of the bar: the crate carries
   the proof, we contribute it, or a stated reason to write decides.
5. It stays upgradable, under §81 (a dependency stays upgradable).

The kernel's registry crate is `uuid` on x86_64, which arrives through `boot_slot` and
`globally_unique_identifier_partition_table`. It meets conditions 1, 2, 3 and 5 on the record, and
adds no more than three crates on any target. It builds `no_std` with default features off and
depends on nothing. The kernel only builds identifiers from bytes the partition table crate has
already reordered, and calls its text parser nowhere outside tests. Its listing of `unsafe` for
condition 4 is `notes/kernel-dependency-unsafe.md`, and a proposal that touches it rereads it. The
kernel's lock is the in-tree `crates/spin_lock`, which milestone 953 (the kernel's lock proves what
it takes from spin) wrote because the kernel's ordering claims rest on proofs of it. Nothing here
changes the ABI, capabilities or the verification path.

## Our own crates

Ruled 2026-11-09 (UTC): we approve in advance any new dependency on one of our own crates, a crate
from the `nifeos` GitHub organization. That includes kernel and image dependencies. Such a
dependency does not wait on an architect and does not get the `needs-architect` hold. The test above
and the kernel's bar still govern the choice of what to depend on. This removes only the
per-dependency approval for our own crates.

Two conditions come with it:

- A git dependency pins a full 40-hexadecimal-digit commit with `rev`: no branch, no tag and no
  floating reference.
- That commit sits on the repository's main.

Third-party dependencies keep the hold unchanged. `helpers/architect-label-rules.py` applies the
exception to a git dependency under `https://github.com/nifeos/` with a full `rev`. It does not
yet recognize a crates.io dependency by a first-party name. Nor does it check that the commit
sits on main. A reviewer checks the second until a lint does.

## How other systems decide this

Read on 2026-11-14 (UTC) unless marked.

- **seL4** proves the kernel's C code, with functional correctness to C for its ARM, AARCH64, X64
  and RISCV64 configurations and to the binary on ARM and RISC-V 64. It states the rest as
  assumptions: the machine interface, boot code, a few lines of assembly and the hardware. Device
  drivers run in user mode outside that proof, and the FAQ invites developers to build on the proofs
  to verify their own components (<https://sel4.systems/About/FAQ.html>). The line is drawn by
  proof, not by authorship. nife draws the same line with condition 2, and differs in letting a
  third-party crate onto the proven side when it carries the proofs.
- **Asterinas** lets only its framework, OSTD, use `unsafe`; its services and drivers must be safe
  Rust, and `kernel/src/lib.rs` says `#![deny(unsafe_code)]`
  (<https://asterinas.github.io/book/kernel/the-framekernel-architecture.html>). OSTD itself links
  third-party crates: `bitflags`, `spin`, `smallvec` and `zerocopy`, and per target `x86_64`,
  `riscv`, `fdt`, `acpi` and `multiboot2` (`ostd/Cargo.toml` on `main`). We found no written bar
  for admitting them.
- **Redox** links crates.io crates straight into its kernel: `arrayvec`, `bitflags`, `hashbrown`,
  `linked_list_allocator`, `spin`, `smallvec` and `object`, `raw-cpuid` and `x86` on x86, `sbi-rt`
  on RISC-V, and `fdt` pinned at a git commit (`kernel/Cargo.toml` on `master`). We found no
  written bar. That its CONTRIBUTING refuses LLM-generated contributions is from an earlier
  reading, not reread here: the page refused our fetch.
- **Linux** imports third-party crates into its tree and adapts them case by case, and asks that
  "somebody needs to step up" to maintain each (<https://rust-for-linux.com/third-party-crates>).
  Its Rust guidelines require a `// SAFETY:` comment before every `unsafe` block and a `# Safety`
  section on every unsafe function (<https://docs.kernel.org/rust/coding-guidelines.html>), which
  is our condition 1. It vendors every crate; nife depends through `Cargo.lock` and vendors only
  what it patches.
- **Fuchsia** vendors external crates under `third_party/rust_crates`. Its Open Source Review Board
  approves each one, with every transitive crate listed, before the change goes to review
  (<https://fuchsia.dev/fuchsia-src/development/languages/rust/external_crates>). Its `unsafe`
  guidance asks each block to state its assumptions and invariants, and says `unsafe` code that
  depends on a third-party crate's correct behavior "will likely be rejected"
  (<https://fuchsia.dev/fuchsia-src/development/languages/rust/unsafe>). Per-crate approval is our
  architect's ruling.
- The census of nife's own dependencies
  ([notes/dependency-standards-census.md](../../notes/dependency-standards-census.md)) read Google's,
  Chromium's, Mozilla's and Android's policies as well. Every one gates dependencies on unsafe review
  and supply chain, and none requires proofs of code it does not own. Condition 2 is stricter than
  all of them, and it binds only where nife would prove the property itself.
