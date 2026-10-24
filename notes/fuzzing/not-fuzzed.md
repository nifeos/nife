# Fuzzing: the parsers deliberately not fuzzed

*Name: provisional.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md), which states the conclusion.

Naming these matters as much as naming the six, because "we have fuzzers" is the kind of claim that
quietly grows to cover everything.

- `nifefs`'s `Fs::parse` on raw bytes. Kani proves it total with no size bound. A fuzz target
  there would be a fuzz target that cannot find anything.
- `filesystem_contract`, `byte_sink_contract`, `socket_contract`. These *are* a trust boundary (a malicious client on
  the other end of an endpoint), and they are unproved. But their decoders are written with
  `slice::get` throughout and are total by construction. The real hazard at that boundary is not a byte string at all. It is the double fetch, where a server validates a length out of a
  shared page and then reads the page again, and the client changed it in between. A byte-string
  fuzzer cannot express a concurrent mutation. That is milestone 43's lens, and it needs a different
  tool.
- `video_terminal`. 63 escape sequences, no proofs, and a byte stream as input, so it looks like
  a good candidate. It is a candidate, and it is not in this first set because its input comes from a
  program *this system started*, one rung inside the boundary the other four sit on. Worth adding
  when the terminal serves output from something we did not build.
- `grant_plan`. Its parser was being rewritten in a concurrent lane while this was built.
