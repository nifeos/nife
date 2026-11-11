*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

I've opened a pull request that covers part of this: an unstable `--target <TRIPLE>` for `kani` and `cargo kani`, `cargo build-dev --lib-target <TRIPLE>` so one Kani build can hold libraries for several targets, and a `riscv64gc-unknown-linux-gnu` machine model. It only handles 64-bit little-endian targets. The 16-bit `usize` case from the original report, and the big-endian case @jswrenn described, would each still need their own machine-model work (and `goto-cc -m32` for 32-bit, per #2086).

Our use is proving a kernel's riscv64 code from x86_64 and Apple Silicon machines. A goto program's machine model is just data, so no riscv64 host is needed. Feedback on the shape is very welcome, including whether you'd rather see an RFC first.

(Written with an AI coding agent under my direction.)
