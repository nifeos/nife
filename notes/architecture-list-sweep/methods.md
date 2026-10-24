# Methods, and what each one cannot see

*Name: provisional. After the main page's own heading.*

An appendix to [`notes/architecture-list-sweep.md`](../architecture-list-sweep.md), which states the
result.

Four methods, run in this order. Stated because a later reader needs to know the shape of the hole
rather than trust the count.

1. Grep for architecture names and triples across `script/`, `helpers/`, `xtask/src/`,
   `.github/`, `bench/`, every `Cargo.toml`, `.cargo/config.toml`, `rust-toolchain.toml`,
   `deny.toml`, `kernel/build.rs`, and `notes/`, ranked by hits per file, then read every file with
   a hit. Found findings 1 through 7.
   Blind to: a list that derives its members instead of spelling them, and a list that spells
   them in a language I did not think to grep. (There is no Makefile or Dockerfile matrix here, but
   a future one would be missed by this method).
2. Per-architecture file-set completeness, by normalizing every tracked path's architecture
   token and counting the families. Found the `bench/fastpath-` divergence and confirmed twelve
   families complete at three.
   Blind to: a per-architecture thing whose files do not carry the token in the name.
   `crates/paging/src/sv39.rs` is riscv64's page-table format and this method scored that family as
   incomplete until it was read by hand. So the method produces false positives, which is safe, and
   would produce a false negative for any family named the way `sv39` is.
3. `#[cfg(target_arch)]` arm counting per file, then reading every file with a nonzero count
   and a zero in one column. Found findings 9 and 10, and the `not(target_arch` hazard class.
   Blind to: a runtime dispatch on architecture rather than a `cfg`, and a `cfg` written through
   a build-script-generated `cfg` name. `kernel/build.rs`'s `cfg(initial_ramdisk)` is exactly the second
   shape, and it is complete, but this method would not have told me that; reading `build.rs` did.
4. Reading the prose claims: every "both ISAs", "both architectures", "two architectures", and "both
   bare-metal" in the tree, filtered to those with no x86 mention nearby. About forty hits, and
   almost all of them are correct. A note recording that something was proven on both ISAs on a
   given day is a record of what happened, not a list a gate reads, and rewriting it would be
   falsifying a record. Only two were coverage claims rather than history: `notes/arch-audit.md`
   (finding 8) and the CI comments [silent-gaps.md](silent-gaps.md) cites. Blind to: a claim phrased
   without those words.

What no method here covers. This sweep read `notes/`, `script/`, `helpers/`, `xtask/`,
`.github/`, the manifests, and the `#[cfg]` sites. It did not read `design/roadmap/` or
`design/decisions/` for incomplete architecture lists, on the ground that a roadmap block is intent
rather than a gate and a decision records what was decided when it was decided. If a decision's
list is load-bearing for a current gate, this sweep missed it. It also did not run any of the gates
against x86_64 to see whether widening them would pass, which is deliberate: that is the proposed
milestone's first job and doing it here would have turned a sweep into a fix.
