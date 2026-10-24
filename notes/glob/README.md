# The glob matcher: the appendix to notes/glob.md

[`notes/glob.md`](../glob.md) carries what `crates/glob` does, its surface, the four scope
decisions, the algorithm and why it cannot hang, the proof harnesses and their summary, and the
corners. This directory carries the record of how the proofs were made tractable, which its prose
budget has no room for.

- [`proofs.md`](proofs.md): why the Kani domain is three bytes, the two restructurings that reached
  it, the solver-time cost, the defect Kani found in a harness, and the host-test enumeration that
  carries the length-independent claims.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/`
beside `notes/X.md`. calef names directories.*

The stem here is named for its content, and calef names files.
