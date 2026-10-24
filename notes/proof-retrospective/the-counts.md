# The counts, and how to re-derive them

*Name: provisional, minted 2026-10-11 (UTC) by lane/sixth-ten-longest-notes.*

Every figure [`notes/proof-retrospective.md`](../proof-retrospective.md) rests on, with the command
that produced it. Take these numbers from the tree, not from here: this page will drift the same way
the roadmap's "112+" did, and the commands below are the re-derivation.

## The numbers, every one counted here

Prose in this tree quotes "112+ harnesses" (the roadmap) and "over 100 across more than 20 crates"
(`notes/fuzzing.md`, correctly hedged as a floor). Neither is the tree's number today. Every figure
below was taken from the merged worktree at base `f1f138a8` on 2026-08-30, with the command shown.

| Quantity | Value | Command |
|---|---|---|
| Lines matching `kani::proof` anywhere | 148 | `grep -rn "kani::proof" --include="*.rs" . \| grep -v target \| wc -l` |
| Of those, doc-comment mentions in `scripts/kani-lint-shim/` | 3 | same, filtered to that path |
| Actual harnesses | 145 | `grep -rn "^\s*#\[kani::proof\]" --include="*.rs" . \| grep -v target \| wc -l` |
| Harnesses under `crates/` | 143 | same, scoped to `crates/` |
| Harnesses in `vendor/redoxfs` | 2 | same, scoped to `vendor/` |
| Packages carrying at least one harness | 25 (24 under `crates/`, plus vendored RedoxFS) | see below |
| Crates in the workspace | 65 | `ls -d crates/*/ \| wc -l` |
| Harnesses `script/verify` actually runs | 140 | sum the shard table in `script/verify` |
| Source lines in harness-carrying crates | 31,725 | see below |
| Source lines of Rust outside `vendor/` and `target/` | 206,728 | `find . -name "*.rs" ... \| xargs wc -l` |
| Source lines of `kernel/src`, which no harness then compiled | 64,818 | `find kernel/src -name "*.rs" \| xargs wc -l` |
| `kani::cover!` vacuity guards, and the four crates holding them | 19 (calendar 7, paging 5, dma_validator 5, glob 2) | `grep -rn "kani::cover" --include="*.rs" crates/` |

The shim directory was `scripts/kani-lint-shim/` when these were measured. It is
`helpers/kani-lint-shim/` since 2026-09-23, and the captured output below is left as it ran.

Two of those rows are findings rather than facts, and the parent picks them up: the gap between 145
and 140, and the fact that twenty of the twenty-four harness crates carry no vacuity guard at all.

## Re-deriving them

Run from the repository root. The looser pattern most readers reach for first, and why it
over-counts by three:

```
$ grep -rn "kani::proof" --include="*.rs" . | grep -v target | wc -l
     148
$ grep -rn "kani::proof" --include="*.rs" . | grep -v target | grep kani-lint-shim
scripts/kani-lint-shim/kani.rs:28://! `kani::proof_for_contract`, ... are all real and
scripts/kani-lint-shim/kani_attributes.rs:6://! next door has no choice about its name, ...
scripts/kani-lint-shim/kani_attributes.rs:23:/// `#[kani::proof]`: keep the function, ...
```

Source lines the harnesses can and cannot reach:

```
$ find kernel/src -name "*.rs" | xargs wc -l | tail -1
   64818 total
$ find . -name "*.rs" -not -path "./target/*" -not -path "./vendor/*" | xargs wc -l | tail -1
  206728 total
```

Every harness name, which is the input to [the reverse pass](the-reverse-pass.md):

```
$ for f in $(grep -rln "^\s*#\[kani::proof\]" --include="*.rs" . | grep -v target); do
      grep -A3 "^\s*#\[kani::proof\]" "$f" | grep -oE "fn [a-z0-9_]+" | sed 's/fn //'
  done | sort
```

Vacuity guards, by crate:

```
$ grep -rn "kani::cover" --include="*.rs" crates/ | sed -E 's|crates/([^/]*)/.*|\1|' \
    | sort | uniq -c | sort -rn
   7 calendar
   5 paging
   5 dma_validator
   2 glob
   1 work_steal_slot
   1 memory_regions
   1 memory_corruption_canary_gate
```

The last three are loom crates, not Kani harness crates, which is why the figure quoted above is 19
rather than 22.

## Which harness packages no shard proves

Moved verbatim from the parent's EXAMPLES under §212 (a prose budget) on 2026-10-13 (UTC).

Packages carrying harnesses, and which ones the suite actually proves (as captured on 2026-08-30,
before the crate rename):

```
$ grep -rln "^\s*#\[kani::proof\]" --include="*.rs" crates/ vendor/ \
    | sed -E 's|^(crates\|vendor)/([^/]*)/.*|\2|' | sort -u | while read c; do
        grep -q "^$c	" script/verify || echo "in no shard: $c"
      done
in no shard: jh7110_trng
in no shard: redoxfs
```
