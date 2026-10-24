# Fuzzing: one ELF seed per machine

*Name: provisional.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md), which states the conclusion.

There are three of them, one per machine nife runs, and that is milestone 288's repair. `elf`
picks the `e_machine` it accepts at compile time, so a seed carries a machine or it carries nothing
useful. There was one seed, `EM_AARCH64`, under a note saying a riscv64 build would reject it; an
x86_64 build rejects it too, which nobody wrote down because `x86_64` became a target after that
sentence was written. On an x86_64 host the corpus really was empty. The two seeds a given build
refuses cost that build's fuzzer one rejected input each, which is the cheapest form this can take.

**A seed that stops parsing is not a seed, and nothing about a fuzz run would say so.** The target
returns immediately on anything the parser rejects, so a corpus of rejected inputs reports the same
"no crashes" a working one does. `crates/elf/tests/fuzz_seed.rs` holds the seed for this build to
actually parsing, and holds the directory to carrying a seed for every machine in
`elf::KNOWN_MACHINES`. So a fourth architecture with no seed fails on whatever host adds it rather
than on the first machine that tries to fuzz there. The other three targets seed from fixtures that
already have tests.

The seed's effect, measured from an empty corpus: `elf_parse` starts at `cov: 65` on its very first
execution and is at 90 edges by input 256. Without it, that first execution is a four-byte magic
check and nothing else.