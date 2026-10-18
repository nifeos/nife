# Would local inference hardware replace part of the Claude subscription?

**Provisional name** (`notes/local-inference.md`). Written 2026-09-27, condensed from a same-day
scratch report that measured this question before any purchase, rental, or model run. Method:
`classify.py` (new), reusing `usage.py`/`price.py`'s pricing table from the earlier vendor-cost pass
folded into this note below.

## Short answer

1. **The offloadable share, measured over 28 days (2026-09-27):** work with a crisp pass/fail gate
   (rebases, citation fixes, PR landings, lint repairs, ruling records, condensing to a word budget)
   is 36% of tokens and 36% of API-equivalent cost. That is about $980/week of a measured
   $2,733/week mean, double the ~18% [notes/open-model-lanes.md](open-model-lanes.md) guessed from
   one example; this measurement replaces that guess.
2. Only a slice of the 36% is safely offloadable today. The narrowest, most lookup-like tasks (plain
   rebases, renumbering, reference sweeps, format and lint fixes) are about 8% of the gated class by
   cost, roughly $80-100/week. The rest still ends in a gate but needs more autonomy, which is the
   shape that failed badly in this tree's own qwen-next trial (2026-09-26: no commits, fabricated
   "gates green" in all four roles). Call the realistic near-term share **5-10% of total spend**, and
   the full 36% a stretch ceiling that needs tighter briefs and a caching fix first.
3. Context length decides this, not model quality. Even inside the gated class, per-request context
   runs p50 165K tokens, p90 358K (sub-agent lanes only). A quarter of requests need over 256K, up to
   854K measured. Every open model plausible for a home rig caps out at or under 256K natively, or
   needs a hardware tier with no published long-context benchmark at all.
4. No researched hardware tier is a clear buy. The 128GB unified-memory tier fits
   Qwen3.5-122B-A10B at 4-bit and Qwen3.8-27B at 8-bit; quality untested in this tree. The 2026-09-27
   stage-1 bake-off (#1425) found the different, already-proven Qwen3-Coder-480B-A35B only 2 of 6
   correct, so the tier question is open for these two models. The two tiers that can hold
   Qwen3-Coder-480B-A35B itself at long context, a maxed Mac Studio or four RTX PRO 6000 Blackwell
   cards, cost $10K-70K+ with zero published prefill numbers at this size. Every such number below is
   this note's own arithmetic.
5. The recommended next step is a bake-off, not a purchase. Rent the candidate model on OpenRouter
   against 15-20 real past nife tasks with known-good outcomes, reusing the blind-graded protocol
   already built for the Opus 5 vs 5.5 pilot. Estimated cost is $40-500, against $10K-70K+ for
   hardware that a bad bake-off result would make useless.

## How the offloadable share was measured

Source: every assistant message with token usage in
`~/.claude/projects/-Users-calef-projects-nife/**/*.jsonl` (main thread) and
`**/subagents/*.jsonl`, 28 days (2026-08-31 to 2026-09-28 UTC), deduplicated on
`(message.id, requestId)`, `<synthetic>` dropped. Totals cross-check within 0.3% against the earlier
vendor-cost pass folded in below: 18,940M tokens here versus its ~19,004M, $10,934 versus $10,952.

Two kinds of unit. **Subagent runs** (598) are classified from the launching call's own recorded
description, plus a scan of its Bash calls for gate commands (`script/lint`, `script/verify`,
`script/citations`, `cargo test`, `gh pr merge`). **Main-thread segments** (7 sessions, split at
45-minute gaps between calef's messages) are classified from the first user message. Six classes, by
keyword: `mechanical_gated` (rebase, lint, renumber, citation, sweep, gate, squash, condense...),
`build_milestone`, `design_proposal`, `review`, `maintainer_conv` (main-thread default), `other`
(subagent default, kept small on purpose). A run that hit a gate command but matched no keyword is
promoted to `mechanical_gated`, unless it already matched `build_milestone`. Milestone lanes gate
too, and running a gate does not tell the classes apart
([notes/open-model-lanes.md](open-model-lanes.md): "the routing rule is about the oracle, not about
difficulty"). Spot-checked at 70-85% precision by eye across two 25-sample passes; `mechanical_gated`
is the noisiest, at roughly 70%.

| class | tokens (4wk, M) | tok % | cost ($, 4wk) | cost % |
|---|---:|---:|---:|---:|
| build_milestone | 7,373 | 38.9% | 4,158 | 38.0% |
| **mechanical_gated** | **6,834** | **36.1%** | **3,916** | **35.8%** |
| maintainer_conv | 2,828 | 14.9% | 1,662 | 15.2% |
| design_proposal | 1,117 | 5.9% | 693 | 6.3% |
| review | 537 | 2.8% | 365 | 3.3% |
| other | 251 | 1.3% | 141 | 1.3% |
| **total** | **18,940** | | **$10,934** (4wk, $2,733/wk mean) | |

`mechanical_gated` is the class the tree's own routing rule already sends to an open model.
`build_milestone`, `design_proposal`, `review` and `maintainer_conv` stay on Claude even when gated,
because the judgment is in the design, not the exit code.

**Context, gated class, subagent lanes only** (n=29,024 requests; main-thread segments carry more
context and would not run locally): p50 164,763 tokens, p75 257,891, p90 358,224, p99 661,175, max
854,555. Share of requests fitting under a window: 64K, 5.9%; 128K, 36.1%; 256K, 75.9%. A
128K-context model misses nearly two thirds of even this narrowest class as lanes are briefed today.

A rough subtype split by cost: plain "hard mechanical" work (rebase, renumber, citation, sweep,
format, lint) is $329 of the class's $3,916, and steward and queue work is $505. The rest, $3,082
(falsification sweeps, condensing to a word budget, PR triage), carries the gate-hit fallback. It is
the biggest line but the least like the one proven win below.

The one measured success, a rebase against Qwen3-Coder for $0.055 (2026-09-22, first try, no redo),
was not logged for tokens directly. At OpenRouter's listed rate for the open-weight `qwen/qwen3-coder`
($0.30/M input, $1.00/M output, uncached), $0.055 implies roughly 80,000-90,000 tokens, well under
the gated class's own median. **It worked because the brief encoded judgment already made** (named
conflict resolutions, an abort condition), not because the model is clever. That is why briefs live
as checked-in assets in `briefs/`, rather than retyped from memory each time.

## Model and hardware shortlist

Research pass, live web lookups 2026-09-27, each source dated. Anything not independently re-checked
here is marked (F), for "from that pass, not re-verified."

| model | size (total/active) | native context | agentic score | note |
|---|---|---:|---|---|
| Qwen3.8-27B | 27.8B dense | 1M (OR listing) | Terminal-Bench 2.1: 73.0 | vendor ran its own benchmark inside the Claude Code harness |
| Qwen3-Coder-30B-A3B | 30.5B/3B | 262K | unverified, not found in readable card text | needs the qwen3_coder tool-call parser |
| GLM-4.5-Air | 110.5B/12B | 131K | no isolated coding score found | vendor measured tool-call success inside Claude Code |
| Qwen3.5-122B-A10B | 125.1B/10B | 262K (~1M w/ YaRN) | SWE-bench Verified: 72.0 | |
| **Qwen3-Coder-480B-A35B** | 480.2B/35B | 262K native, ~1M w/ YaRN | SWE-V ~69.6, unverified (vendor social post) | this tree's own proven case |
| gpt-oss-120B | 116.8B/5.1B | 131K, below this workload's p50 | SWE-V 52.6-62.4 | needs OpenAI's "harmony" response format |

Disqualified by context alone: DeepSeek-V3 (163,840 max) and gpt-oss-120B, both under the gated
class's own p50. Kimi K2 (1.03T total) fits no tier below at 4-bit. Benchmark scores come from
vendor cards, not independently re-run, and versions of the same benchmark do not compare (GLM-5.3
scores 88.2 on Terminal-Bench 2.1 and 28.3 on 3.0, per the vendor's own numbers).

| tier | price (2026-09-27) | usable memory | fits |
|---|---|---:|---|
| 128GB unified memory (DGX Spark, Strix Halo) | $3,449-5,000 | 128GB | Qwen3.5-122B-A10B at 4-bit, Qwen3.8-27B at 8-bit; quality untested (see below) |
| Mac Studio, 256GB | $9,499-10,799 | 256GB | Qwen3.5-122B-A10B, comfortably |
| Mac Studio, 512GB (not yet priced, ships "late October," may slip) | absent | up to 512GB | Qwen3-Coder-480B-A35B, tight: ~300GB weights plus 67-178GB KV at 256K-700K context (Est) |
| 1x RTX PRO 6000 Blackwell | $16,000 | 96GB | Qwen3.8-27B or GLM-4.5-Air at 8-bit |
| 4x RTX PRO 6000 | $68-72K all-in | 384GB | Qwen3-Coder-480B-A35B: ~300GB weights, ~45GB left for KV; fits the class's p50 context only with FP8 KV (~354K tokens), not its p90 |

No tier reaches the class's p90 context (400-700K) with headroom. The two tiers that reach it at all
do so at the edge, on this note's own unverified arithmetic (Est), for a model with one real success
in this tree and no independent long-context benchmark found anywhere.

Correction, same pass, 2026-09-27: the 128GB row above first failed Qwen3.5-122B-A10B by checking
its 4-bit fit against the 8-bit weight size. 125.1B parameters is about 125GB at 8-bit (1
byte/parameter), and about 62.5GB at 4-bit (0.5 bytes/parameter). At 4-bit, 62.5GB of weights plus
about 6GB of KV cache for the model's full 262K native context (about 24 KiB/token) totals about
68.5GB, well inside 128GB. Qwen3.8-27B at 8-bit is 27.8GB. At about 64 KiB of KV per token it adds
about 10GB at this workload's p50 context (165K) and about 22GB at its p90 (358K), both comfortably
inside 128GB too. Corrected verdict: the tier fits Qwen3.5-122B-A10B at 4-bit and Qwen3.8-27B at
8-bit; quality untested in this tree. It still does not fit Qwen3-Coder-480B-A35B, the model with a
proven result in this tree, which needs the larger tiers below. 2026-09-27's stage-1 bake-off (#1425)
tested that different model and found it 2 of 6 correct, so the tier question is open again
specifically for Qwen3.5-122B-A10B and Qwen3.8-27B.

## The bake-off, drafted and not run

Reuses `notes/model-comparison/pilot-protocol.md`'s isolation and blind-grading design: a fresh clone at
the task's base commit, no remote, a wall-clock and budget cap, and a grader that never sees which
model produced which packet. The 19 tasks are real merged pull requests from the `mechanical_gated`
class with known-good outcomes: rebases, PR landings, gate additions, triage, six word-budget
condensing tasks, ruling records, and citation-tooling fixes. Measured Claude cost across the 19 totals $696, individual
tasks ranging $7.42-$82.75; context p50 ranges 79K-573K per task, including one long-context stress
case at 573K.

Scoring per task: right files touched (0-2), and whether it ran the gate, read from the transcript
rather than the report. Also checked: whether its claim of "green" matched a gate re-run afterward,
the same check that caught qwen-next's fabricated claims. Two more measures: how many rounds it took
to reach green, and a count of false claims of green, the single most important number, since it is
what actually failed in the qwen-next trial.

Re-pricing the 19 tasks' measured tokens onto candidate OpenRouter models, uncached (gateway caching
is unverified) and cached (optimistic) gives: `qwen/qwen3-coder`, the proven model, $353 uncached,
$138 cached. `qwen/qwen3-coder-30b-a3b-instruct` $82; `z-ai/glm-4.5-air` $153 uncached, $40 cached;
`qwen/qwen3.8-27b` $495 uncached, $134 cached; `qwen/qwen3.5-122b-a10b` $307. Every row is cheaper
than Claude's own $696 on these tasks, but this reprices token counts onto another tokenizer and
pricing, not a prediction of real cost. A model needing more retry rounds, or failing outright like
qwen-next, spends money for nothing. Running the one proven candidate alone, with a handful of the
cheapest tasks as a smoke test, costs under $50; a full run of all five models against all 19 tasks
costs an estimated $1,000-1,400.

## What this corrects in the earlier vendor-cost pass

`vendor-cost-report.md`, this same 2026-09-27 pass, measured what nife's Claude usage would cost
elsewhere. Its conclusions: the Max 20x subscription is far cheaper than any API alternative, at
$2,738/week measured mean and $5,450 in the heaviest week, against $46/week for the plan. OpenRouter
adds nothing for Claude models itself, at the same per-token price plus a 5.5% credit fee. The one
open model already measured on real nife tasks, qwen-next on 2026-09-26, failed on every role. Its
recommended order was a second Max 20x subscription (unverified against Anthropic's terms), then
tracing why Opus 5.5 writes six times the cache Opus 5 did, then a DeepSeek V4 Pro pilot on
gate-checked lanes only.

That report's OpenRouter pricing table priced `qwen/qwen3-coder-plus` at $0.65/M input and $3.25/M
output. **That id is Alibaba's closed, hosted "Plus" tier, not the open-weight Qwen3-Coder.** It was
the wrong SKU for a self-hosting question, though the right SKU for its own question (renting
inference, not owning weights). Checked against OpenRouter's model listing fetched 2026-09-27: the
open-weight 480B-A35B flagship is `qwen/qwen3-coder`, priced at $0.30/M input, $1.00/M output, with
cache reads at $0.10/M (a 0.1x multiplier on input). No separate cache-write price is listed for it,
so a write bills as ordinary input. This is the price used throughout the bake-off section above.

## BUGS

- Classification is heuristic (keyword match plus gate-command sniffing), spot-checked on 50 of 706
  units, not hand-graded on all of them.
- The Qwen3-Coder rebase's token count was never logged, only its cost; the 80-90K estimate above is
  arithmetic from that one number, not a measurement.
- No hardware tier here has a published benchmark for any shortlisted model at this workload's real
  context lengths. Every prefill and decode number for the two tiers that could hold
  Qwen3-Coder-480B-A35B at long context is this note's own estimate (Est), not a measurement.
- The 512GB Mac Studio is not yet priced; every dollar figure for that configuration is absent, not
  estimated.
- Prompt caching through the cordoba gateway is billed as a miss
  ([notes/open-model-lanes.md](open-model-lanes.md) BUGS); every "cached" figure above is optimistic
  until that is fixed or independently confirmed.
- Nobody has run the bake-off yet. This note is written from documentation, transcripts, and measured
  hardware prices; it has not driven a single lane.

## Sources

Transcripts: `~/.claude/projects/-Users-calef-projects-nife/`, measured 2026-09-27. In-tree:
[notes/open-model-lanes.md](open-model-lanes.md), `notes/model-comparison/pilot-protocol.md`,
`notes/model-comparison/2026-09-24-pilot.md`,
`notes/model-comparison/2026-09-26-qwen-next-results.md`. OpenRouter model list: `GET
https://openrouter.ai/api/v1/models`, fetched 2026-09-27. Model shortlist and hardware tiers: a
same-day research pass (WebSearch/WebFetch). Primary sources: Hugging Face model cards, vendor
benchmark cards, NVIDIA's DGX Spark spec page, Apple's Mac Studio spec page, and
llama.cpp/hardware-corner.net benchmark threads for the only measured prefill and decode numbers
found. The full 38-source citation list is in the original scratch report and was not carried into
the tree.
