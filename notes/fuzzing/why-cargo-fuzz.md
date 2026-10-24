# Why cargo-fuzz

*Name: provisional. Named after the main page's own heading.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md), which states the conclusion.

DECISIONS §46 (thin primitives or whole subsystems) says taking a dependency is a decision, and that
holds for a dev tool. Three candidates were real: cargo-fuzz (libFuzzer), cargo-afl, and
honggfuzz-rs.

cargo-fuzz wins on one property the other two do not have here. It is in-process and coverage-guided
with no external harness process, so a target is a plain Rust function over `&[u8]` that the crates
under test link into directly. Our parsers are `no_std`, pure, and take a byte slice, which is the
exact shape libFuzzer wants. AFL's fork-server model buys robustness against a target that corrupts
its own address space, which safe-Rust parsers cannot do, and pays for it in exec rate. cargo-fuzz
is also what rustc, `regex`, and most of the ecosystem use, so a crash reproduces for a stranger
reading this repository.

The costs, stated rather than glossed: it needs nightly (already pinned), it needs `rust-src` for
`-Zbuild-std` (already listed for the kernel), and it adds three crates (`libfuzzer-sys`,
`arbitrary`, `cc`) to a graph nothing ships. It also brought the license exception below.

The one license decision. `libfuzzer-sys` is `(MIT OR Apache-2.0) AND NCSA`, because it vendors
LLVM's libFuzzer C++ source. `deny.toml` carries it as a `[[licenses.exceptions]]` scoped to that
one crate rather than as an allow-list entry, so a second NCSA crate would still stop the build.
NCSA is OSI approved and FSF free and reads as BSD-3-Clause and MIT stapled together, and nothing
that boots links it.