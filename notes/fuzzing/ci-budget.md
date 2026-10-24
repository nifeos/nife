# Fuzzing: the CI budget, measured

*Name: provisional.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md), which states the conclusion.

Why sixty seconds. Measured rather than felt. Ten minutes on each target, on the developer machine
(Apple Silicon, and busy with another lane's QEMU for part of it):

| Target | Executions in 600s | Per second |
|---|---:|---:|
| `dtb_walk` | 11,702,017 | 19,470 |
| `elf_parse` | 269,592,178 | 448,572 |
| `gpt_table` | 14,532,550 | 24,180 |
| `nifefs_roundtrip` | 21,469,277 | 35,722 |
| **total** | **317,296,022** | |

The targets were named `gpt_table` and `dtb_walk` when this table was measured; they have been
`globally_unique_identifier_partition_table` and `device_tree_blob_walk` since calef's 2026-09-19
ruling that a fuzz target follows the crate it fuzzes. The rows keep the names the run printed.

The spread is the shape of each parser rather than noise: `elf_parse` rejects most inputs in its first
fifteen lines and returns, while `device_tree_blob_walk` runs seven full walks over a blob for every
input it accepts. The slowest target is the floor, so a minute buys at least a million inputs on
every target, and four to six million per CI run, from a corpus already past every magic-number
check. CI's runner is a different machine; the order of magnitude is what the budget is chosen
against.

What the CI budget actually does, measured the way CI does it, from a deleted corpus so nothing
carries over:

```
$ rm -rf fuzz/corpus fuzz/artifacts && script/fuzz --time 60
dtb_walk               1,495,284 execs    24,512/s
elf_parse             51,234,895 execs   839,916/s
gpt_table              1,548,246 execs    25,381/s
nifefs_roundtrip    1,763,965 execs    28,917/s
==> fuzz: no crashes in 4 targets at 60s each
```

And it is weaker than that number makes it sound, which is worth knowing before anyone leans on
it. The draft of this paragraph claimed the job would now catch `Region::end`'s overflow immediately
from the seeds. That was a guess, so it got tested: both `dtb` fixes were reverted and
`script/fuzz --time 60 dtb_walk` run from a deleted corpus. It found nothing. Reverted the same
way with a fifteen-minute budget, it found the panic after 13,124,546 executions, about ten
minutes, which is the same order as the original discovery.

So the sixty-second job is a sweep, not a guarantee, and a bug of this depth is outside it. **What
actually keeps these two bugs from coming back is `crates/device_tree_blob/tests/hostile.rs`**,
which runs in milliseconds on every `script/test` on both ISAs. That is the division of labor: the
fuzzer finds things once, and a host test holds them forever. A CI fuzz job that had to catch every
regression it ever found would need a budget nobody would pay.
