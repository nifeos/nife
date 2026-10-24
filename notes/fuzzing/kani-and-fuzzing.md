# Where Kani stops and fuzzing starts: three worked examples

*Name: provisional.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md), which states the conclusion.

The place where they meet is the bound, and this tree has three worked examples of the bound
being the whole story:

1. `crates/elf`. The crate's own doc comment says the whole-parse totality proof *did not
   return*: an O(n^2) overlap loop over up to 64 program headers, unrolled 64 deep against symbolic
   slice offsets, is past what the solver can do. What is proved is `check_segment_bounds`, the leaf
   arithmetic, factored out so bounded model checking can reach it. The proved leaf and the unproved
   shell are exactly the split a fuzzer covers from the other side.
2. `crates/device_tree_blob`. Four harnesses, all on `be32`/`be64`, the leaf readers. The seven
   *walkers* above them are unbounded loops over a symbolic blob carrying a depth counter and two
   16-entry per-depth arrays. Both of the bugs found on 2026-08-02 were in that unproved region, and one of
   them was an out-of-bounds index into one of those arrays.
3. `crates/nifefs`. `Fs::parse` is proved total, with no bound on the image size at all
   (`the_validation_implies_reads_slice_is_in_bounds`). Fuzzing it for panics would be burning CI
   time to rediscover a theorem. What is *not* proved, and cannot be by a harness of that shape, is
   that the writer and the reader agree, and that is where its bug was.
