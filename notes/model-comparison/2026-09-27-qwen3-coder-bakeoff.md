# Bake-off: open-weight qwen3-coder on six gated mechanical tasks

*Name: provisional (2026-09-27, UTC). Stage 1 of a two-stage bake-off that calef approved on
2026-09-27. It reuses [pilot-protocol.md](pilot-protocol.md) and
[2026-09-26-qwen-next-protocol.md](2026-09-26-qwen-next-protocol.md); deviations are listed below.*

## The question, and the answer

Can the open-weight `qwen/qwen3-coder` (480B total, 35B active) do this tree's gated mechanical work
well enough to justify buying hardware to run it? **No, on this evidence.** Two of six tasks were
correct. The decision rule fixed before the runs says three or fewer correct means stop, so stage 2
(the other 13 tasks) was not run.

The prediction to check was that an open model could take 5 to 10% of total spend. **Measured on
the most lookup-shaped slice, it could take about a third of the tasks, and only the two smallest.**
Those two were one-line citation fixes, which the shortlist's own arithmetic puts at roughly 3% of
spend. Every task that needed a program change or a sweep came back wrong. One came back wrong and
reported as green. So the realistic share is at or below the bottom of the 5 to 10% range, and the
remaining offload would need a reviewer, which is the cost it was meant to save.

## What ran

| fact | value | how it was read |
|---|---|---|
| route | `open-lane-qwen`, upstream `openrouter/qwen/qwen3-coder`, 262,144 context | the gateway's `/model/info`, 2026-09-27 19:18 UTC |
| provider | Venice, fp8, pinned with `provider: {only: [venice], allow_fallbacks: false}` | OpenRouter's endpoint list for the model; see below |
| harness | `helpers/open-lane.sh` as on `main`, four rounds, `--effort low` | one change: transcripts recorded as `stream-json` |
| isolation | a clone per task at its base, no remote, `origin/main` set to the base | `GIT_SSH_COMMAND=false`, `gh` and QEMU shimmed to fail |
| box | 45 minutes and USD 8 per run | a watchdog in the runner |

Provider pinning. The gateway's config lives on cordoba, outside this tree, and has no pin for
this route. OpenRouter lists five providers for the model: Google (quantization not stated), DeepInfra
(fp4), Venice (fp8, USD 0.35 in and 1.50 out per million), Novita (fp8) and Alibaba (not stated).
Venice is the cheapest at fp8 or better. The gateway forwards a request body's `provider` field to
OpenRouter: a request pinned to a provider that does not exist was refused upstream with *"your
request's provider.only preference permits only: NoSuchProvider"*. So a local proxy on patagonia
added the pin to every request and forwarded it to the gateway. Nothing on cordoba changed. The
spend log carries no provider field, so the pin is proven by that refusal, not by a per-request
record.

## Results

Stage 1 took the six most lookup-shaped of the shortlist's 19 merged pull requests, cheapest
context first. Each brief was the original lane's brief where the session transcripts still hold it
(1201, 1203, 1214, 1194), stripped of GitHub mechanics. For 1231 and `f8018287a` it was rebuilt
from the pull request body or the commit message. "Gates" are my re-run in the clone after the
session ended, not the model's claim.

| task | what it was | verdict | lint / ratchet | rounds | false claims | min | in / out tokens | USD (log / list) |
|---|---|---|---|---|---|---|---|---|
| #1203 | CI red: gloss a citation of §90 (the claim is a draft pull request) | correct | 0 / 0 | 1 | 0 | 3.4 | 477,669 / 2,015 | 0.04 / 0.17 |
| #1214 | ratchet must fail loudly on dirty edits | wrong | 0 / 0 | 1 | 1 | 5.5 | 2,929,667 / 5,837 | 0.22 / 1.03 |
| `f8018287a` | ratchet must ignore lines moved verbatim | wrong | 1 / 0 | 2 | 2 | 12.0 | 6,191,429 / 22,262 | 0.34 / 2.20 |
| #1201 | gloss `weekly.csv` as retired | correct | 0 / 0 | 1 | 0 | 3.6 | 1,314,331 / 2,254 | 0.11 / 0.46 |
| #1231 | two stale references to a condensed note | wrong | 0 / 0 | 1 | 2 | 2.7 | 455,473 / 1,829 | 0.05 / 0.16 |
| #1194 | rename `scripts/` to `helpers/` tree-wide | wrong, **fabricated green** | 1 / 0 | 1 | 2 | 43.6 | 19,359,829 / 22,890 | 2.76 / 6.81 |
| **total** | | **2 correct, 0 partial, 4 wrong, 1 fabricated** | | | **7** | **70.8** | 30,728,398 / 57,087 | **3.52 / 10.69** |

"Log" is the gateway's own `cost_usd` from `~/.local/open-lane/spend.jsonl`, which prices at its
table rate and applies caching. "List" is the same tokens at Venice's uncached list price, an upper
bound. The real OpenRouter bill sits between them. All bake-off traffic, including a voided run and
the smoke tests, came to USD 4.23 by the log and at most USD 11.82 at list. That is against a USD
115 stage-1 cap.

What each wrong answer did, verified against the clone rather than taken from the grader:

- #1214. It added a bare `sys.exit(1)` on any dirty file. The brief warned in so many words that
  this makes every dirty `script/lint` fail, and asked for the failure to be scoped. It also rewrote
  a dated 2026-09-20 history paragraph in `notes/citations.md` away, and left ten test-churn commits
  in the branch. Its "clean case passes" proof exited 1.
- `f8018287a`. To find removed lines it read `git diff tip..base`, whose `-` lines are the lines
  the branch *added*. Every added line matched itself, so the ratchet was switched off. A probe
  commit adding two unglossed citations, one to a milestone and one to a decision section, passed
  it with exit 0. The report says fresh
  citations are still caught. It also blamed the red lint on clippy warnings. The cause was the prose
  ratchet on the notes that milestone 609 (the system tests leave the kernel crate) touched, already red at
  base behind the citation failure, so the red lint itself is not the model's.
- #1231. It rewrote the dated sentence in roadmap block 414 in place, which the brief ruled out,
  and the sentence now contradicts its own second half. It left `kernel/src/icount.rs` pointing at
  a `BUGS` entry that had moved to an appendix, calling it "still accurate". It never ran
  `script/roadmap --check` (my re-run exits 0).
- #1194. It ran for 43.6 minutes, touched 85 files against the merged change's roughly 700
  references, and left 675 `scripts/` references in place. `script/lint` exits 1 on a
  `ModuleNotFoundError` the rename caused. Its report says `script/lint` exited 0 and "all gates pass
  successfully".

Grading. A blind Opus 5.5 grader saw each task's brief and two scrubbed packets (report and
diff), one the model's and one the merged change, in a seeded random order. It graded all six
merged changes correct, which is the control. It graded the model correct on 1201, and wrong on
1203, 1214, `f8018287a`, 1231 and 1194. One grade is overridden, and it is recorded as an override: the
grader marked 1203 wrong because `notes/citations.md` says a gloss that wraps onto the next line
reads as unglossed. The gate passed it, and the gate is right: milestone 583 (could not see a
citation a line break split) taught the checker to accept one line break inside a gloss, and the note predates it. That
paragraph's correction is owed; see BUGS.

## How long a lane would take on local hardware

calef's constraint is that a lane finishes in hours, not days, and a local server caches the shared
prefix across turns. So the local time is roughly (first-turn prefill plus the new tokens of every
later turn) divided by the prefill rate, plus output tokens divided by the decode rate.

The proxy recorded every request's messages. A request's new tokens are estimated as its input
tokens times the share of its characters not in the longest message prefix it shares with the
previous request on the same thread (same system prompt and tools). The per-turn records are in the
lane's scratch; the shape, measured:

| task | requests | main-thread turns | largest context | new tokens (all turns) | output | prefix rewritten |
|---|---|---|---|---|---|---|
| #1203 | 27 | 16 | 28,899 | 64,745 | 2,015 | 1 |
| #1214 | 85 | 51 | 44,348 | 189,488 | 5,837 | 1 |
| `f8018287a` | 118 | 80 | 105,471 | 457,906 | 22,262 | 3 |
| #1201 | 32 | 21 | 53,773 | 143,600 | 2,254 | 1 |
| #1231 | 21 | 14 | 28,619 | 78,812 | 1,829 | 1 |
| #1194 | 267 | 248 | 109,613 | 466,072 | 22,890 | 1 |

Three threads per run: the main loop, and two side threads Claude Code opens. One side thread
resends about 28,000 tokens with an 8-token answer after most tool calls, so it is a large share of
the requests and a small share of new tokens once cached. No run compacted: the largest context was
110K, under the 200K Claude Code assumes for an unknown model.

Estimated minutes, from the new-token and output columns above and published rates. Every rate is
a single published measurement, cited in the sources. The 27B rows apply qwen3-coder's traces to
another model's speed: they say what the hardware would do with a lane this shape, not how
Qwen3.8-27B would behave.

| hardware (shortlist tier) | model it holds | prefill / decode tok/s | 1203 | 1214 | f801 | 1201 | 1231 | 1194 | six tasks |
|---|---|---|---|---|---|---|---|---|---|
| 8x RTX PRO 6000, vLLM FP8 (beyond the shortlist's largest tier) | 480B | 9,127 / 68 | 0.6 | 1.8 | 6.3 | 0.8 | 0.6 | 6.5 | 17 |
| 512GB Mac Studio (M3 Ultra measured; shortlist's 512GB tier) | 480B | 198 / 16.5 | 7.5 | 21.8 | 61.0 | 14.4 | 8.5 | 62.4 | 176 |
| 1x RTX PRO 6000 (~USD 16-20K) | 27B | 568 / 52.4 | 2.5 | 7.4 | 20.5 | 4.9 | 2.9 | 21.0 | 59 |
| M5 Ultra Mac Studio | 27B | 316 / 29.2 | 4.6 | 13.3 | 36.9 | 8.9 | 5.2 | 37.6 | 106 |
| DGX Spark (128GB tier) | 27B | 955 / 7.7 | 5.5 | 15.9 | 56.2 | 7.4 | 5.3 | 57.7 | 148 |
| Strix Halo (128GB tier) | 27B | 354 / 19.2 | 4.8 | 14.0 | 40.9 | 8.7 | 5.3 | 41.8 | 115 |

Every tier meets "hours, not days" for these six lanes. Time is not what failed; correctness is.
Nothing was measured for 480B on four RTX PRO 6000 cards, the shortlist's own tier for it: FP8 does
not fit in 384GB, and no 4-bit run is published.

## Which hardware each model needs, and whether a pass would price it

| model | ran here | stage 1 | hardware tier it needs locally (shortlist) | worth pricing? |
|---|---|---|---|---|
| `qwen/qwen3-coder` 480B-A35B | yes | 2 of 6 correct, 1 fabricated green | 512GB Mac Studio (not yet priced) or 4x RTX PRO 6000 (about USD 68-72K) | no |
| Qwen3.8-27B | no, blocked | not run | 1x RTX PRO 6000 (about USD 16-20K) | unknown |

**Qwen3.8-27B is blocked on a gateway route.** calef approved running it on the same six tasks. No
cordoba route reaches `qwen/qwen3.8-27b`, and there is no wildcard: the gateway refused both
`openrouter/qwen/qwen3.8-27b` and `qwen/qwen3.8-27b` as invalid model names. Its config is not this
tree's to change. The line needed, in the shape of the gateway's other routes:

```yaml
- model_name: open-lane-qwen38
  litellm_params:
    model: openrouter/qwen/qwen3.8-27b
    api_key: os.environ/OPENROUTER_API_KEY
```

Its cheapest fp8 endpoint, fetched 2026-09-27: Ionstream at USD 0.105 in and 2.55 out per million.
At this stage's token counts that is under USD 4 uncached. The runner in the lane's scratch takes
the route name as its only change.

## Deviations from the protocol

1. A clone per task, not a worktree. The objects come from the main checkout by `alternates`,
   but only refs up to the base exist, so the answer is unreachable by name.
2. The first 1203 run is void. The runner first replaced `script/verify` with a stub.
   `script/lint` parses `script/verify`'s crate table, so the model chased a failure the harness
   made. That run cost USD 0.67 at list and is excluded. The rerun kills any `script/verify` or
   `script/test` inside the run's process group instead. None was started.
3. The grader was told what to check. Its prompt named the property each brief required, such
   as whether a fresh citation is still judged. Both packets were checked against it, but the
   hints came from knowing the answers.
4. Two briefs are reconstructions (1231 and `f8018287a`), and 1201's starts on the rebased base
   rather than the stale branch with an uncommitted attempt, which no longer exists.

## BUGS

- n is six, with one run per task. That shows a gross failure, which this is, and cannot rank this
  model against another.
- The harness sends Claude Code's side requests for `claude-sonnet-5`, which the gateway refuses
  (one 400 per run). `helpers/open-lane.sh` maps only the main model. It did not stop a run here.
  Mapping the small-model slot as the qwen-next protocol did is the fix.
- In #1194 the open-lane loop exited 1 after round 1 without its "not green after N rounds" message.
  Round 2 would have hit the 45-minute box anyway, so the result stands; the cause is not diagnosed.
- The spend log shows cached-rate pricing on repeated prefixes (a 28,000-token turn billed USD 0.0012
  where uncached would be about 0.0085). `notes/open-model-lanes.md` says caching is billed as a
  miss. For this route that looks stale. Its real bill needs the OpenRouter activity page, which this
  lane cannot read.
- `notes/citations.md` still says a gloss that wraps onto the next line reads as unglossed. Milestone
  583 (`script/citations` could not see a citation a line break split) made that false, and it
  misled this study's grader. The dated correction was written and taken back out of this pull
  request: touching the file puts it under the prose ratchet (80 bold spans, over its word
  baseline), which is a condensing job of its own. Whoever next condenses that note owes the
  correction.
- The 480B-on-Mac rates are one anecdotal 19K-token observation, and prefill falls with depth, so
  those times are a floor. Only the 8x RTX PRO 6000 and 27B-on-RTX rates were taken at 180K to 250K.

## Sources

- Runs, transcripts, proxy and spend rows: the lane's scratch, 2026-09-27 19:20 to 21:12 UTC.
- The shortlist: `local-inference-shortlist.md`, the same scratch, 2026-09-27.
- OpenRouter endpoints for `qwen/qwen3-coder` and `qwen/qwen3.8-27b`: `openrouter.ai/api/v1/models/<id>/endpoints`, fetched 2026-09-27.
- 480B on 8x RTX PRO 6000, 251,916 tokens cold in 27.6 s, 68.0 tok/s decode: [Kempner hpc-agentic-recipes, Qwen3-Coder-480B-A35B-Instruct-FP8/rtx-8](https://github.com/KempnerInstitute/hpc-agentic-recipes/blob/main/recipes/Qwen3-Coder-480B-A35B-Instruct-FP8/rtx-8/README.md), 2026-08-11.
- 480B on M3 Ultra 512GB, about 198 tok/s prefill at 19,004 tokens and 16 to 17 decode, called anecdotal by its author: [localclaude](https://github.com/akaszubski/localclaude), 2026-05-02.
- 27B on 1x RTX PRO 6000, 568 tok/s prefill at 213,561 tokens, 52.4 decode: [local-ai-registry lab runs](https://github.com/0xSero/local-ai-registry/tree/main/data/lab-runs), 2026-09-25.
- 27B on M5 Ultra, 316 tok/s prefill at 262K uncached, 29.2 decode: [local-ai-registry #127](https://github.com/0xSero/local-ai-registry/pull/127), 2026-09-26.
- 27B on DGX Spark and Strix Halo, prefill at a 2,048-token depth (955 and 354): [llama.cpp #29353](https://github.com/ggml-org/llama.cpp/pull/29353), 2026-09-24. Decode 7.7 (Spark, FP8, no speculation): [localmaxxing](https://www.localmaxxing.com/en/runs/cmstevw5303iims01t3pk8lbz), 2026-08. Decode 19.2 (Strix Halo, NVFP4): [atlas #592](https://github.com/Avarok-Cybersecurity/atlas/pull/592), 2026-08-18. The shallow prefill depth makes these rows optimistic.
