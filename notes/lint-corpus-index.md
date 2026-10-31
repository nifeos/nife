# A shared corpus index for the text gates: what to build, and what to measure first

*Name: provisional, minted 2026-10-31 (UTC) by lane lane/980-corpus-index-note. Milestone 980
(script/lint stops paying per-item costs), part 5. This is a design note for review. Nothing in it
exists in code yet.*

Milestone 980 proposed one helper that lists the tracked files, their text and their hashes once
per run, so the text gates read it instead of each walking the tree. It estimated 10 to 15 seconds.
This note measures the premise, finds it half true, and proposes a different shape that keeps the
saving. No verdict changes in either shape.

## What the measurement says

Twelve text gates in `script/lint` read the corpus: the prose ratchet, `house_style.py`,
`history_ratchet.py`, `cap_abbreviation.py`, `unit_symbols.py`, `line_citations.py`, the comment-block
and module-block ratchets, `file_length_ratchet.py`, `parent_index.py`, `name_checks.py` and
`packages.py`. Each ran alone with `open` counted, on a Linux host while other lanes ran.

| gate (`--check`) | files opened | opened twice or more | user CPU (s) |
|---|---:|---:|---:|
| `name_checks.py` | 2,764 | 2,609 | 12.6 |
| `prose_ratchet.py` | 2,992 | 1,080 | 11.4 |
| `comment_block_ratchet.py` | 1,384 | 262 | 6.0 |
| `house_style.py` | 5,447 | 2 | 5.2 |
| `cap_abbreviation.py` | 1,353 | 0 | 3.9 |
| `unit_symbols.py` | 1,353 | 0 | 3.8 |
| `history_ratchet.py` | 2,745 | 1 | 3.7 |
| `line_citations.py` | 5,012 | 2 | 2.7 |
| `module_block_ratchet.py` | 2,651 | 1,236 | 2.6 |
| `parent_index.py` | 1,384 | 0 | 1.1 |
| `file_length_ratchet.py` | 1,384 | 0 | 0.1 |

The gates add up to about 53 seconds of CPU. Two facts matter for the design.

1. Reading is cheap. `git ls-files -s` lists 6,325 entries with their blob hashes in 0.01 seconds.
   Reading all 4,493 Markdown and Rust files, 42 MB, takes 0.13 seconds with a warm page cache.
   `file_length_ratchet.py` opens 1,384 files and costs 0.1 seconds. The cost of a gate is what it
   does with the text: lexing Rust, splitting sentences, counting passives, matching patterns.
2. Sharing text cannot cross a section boundary. `helpers/lint-sections.sh` runs each section in a
   fresh `sh`, and each gate is its own Python process. A list held in memory by one helper is gone
   when the next starts. A per-run index therefore needs either one process for all gates or a file
   on disk, and a file on disk is a cache.

So an index of text and hashes saves roughly the 0.13 seconds times twelve, under two seconds. The
10 to 15 second estimate does not hold for that shape. It holds only for an index of derived data.

## The shape to build: derived data, keyed by blob hash

Keep a small cache of what gates compute from a file, keyed by the file's blob hash and the version
of the code that computed it. A gate asks the cache for a file's derived record, computes it on a
miss and writes it back. It then applies its rule to the records, as it does today.

The cache stores facts about a file and never a verdict. Whether `prose_ratchet.py` passes is
decided every run from the records and the baseline. That keeps the property the cost note insists
on: the same tree gets the same answer with a warm cache, a cold one or none.

Candidate records, in the order of what they save:

- Stripped Rust. `name_crossings.strip` output per file, and the block table that
  `enclosers` builds. `name_checks.py` computes them for 1,353 files, and other gates lex the same
  files again.
- Prose measures. Per document: words, sentence lengths, longest sentence, passive count,
  bold lead-ins. The prose ratchet re-derives all of them for 2,743 documents each run.
- Line facts. The per-file counts that `house_style.py`, `unit_symbols.py` and
  `cap_abbreviation.py` each scan for.

### The key

A record's key is `(blob hash, deriver id)`. The blob hash is the first column of `git ls-files -s`,
so it names content and ignores timestamps and checkout order. The deriver id is a hash of the
deriving function's own source, so editing a lexer rule invalidates its records without anyone
remembering to bump a number. A file with uncommitted edits has no blob hash that matches its
content, so the index hashes the working text for those few paths (`git diff --name-only` lists
them) and keys on that.

### Where it lives and how it fails

One directory under the worktree's untracked state, for example `target/lint-index/`, one file per
deriver. A worktree has its own, so lanes never write the same file. Writes go to a temporary name
and rename, so a killed run leaves no partial record. A record that fails to parse is a miss.
Deleting the directory is always safe, and a gate prints how many records it hit and computed, so
the saving is visible in the log in the way the section time is.

## Prior art and where nife differs

- ccache and sccache key compiler output by a hash of the input and the compiler. They cache an
  artifact, as this proposal caches a record, and they share the rule that the key must contain
  everything the output depends on.
- Bazel's action cache and Gradle's build cache key by inputs and tool version, and trust the
  result. nife has a stricter need: a gate is evidence, so the cache must not carry a verdict.
- ruff and eslint `--cache` store per-file lint results keyed by modification time, size and
  configuration. A stale key can hide a finding. nife keys on content and stores facts, which costs
  a hash lookup per file and removes that failure.
- pre-commit runs hooks on changed files only. The cost note rejected that for nife, because it
  changes what lint refuses. A cache of facts runs every gate over every file and changes nothing
  but the work.
- git's untracked cache and commit-graph are the same idea inside git: derived data, keyed by
  content, safe to delete.

Where nife differs: lanes run many cold worktrees, CI starts cold every time, and the gates are
shell sections that share no process. The first two limit the gain. A new worktree pays full price
once. CI sees no gain unless a job restores the directory, which would be a separate decision about
CI caching and is not proposed here.

## A concrete example

The prose ratchet today, for a lane that edits three documents:

1. `git ls-files '*.md'` lists 2,743 documents.
2. It reads each one and computes its measures. This is 5.2 seconds of the section on the reference
   host.
3. It compares each measure with the baseline row.

With the cache, step 2 becomes: run `git ls-files -s`, look up 2,743 `(blob, deriver)` keys, find
2,740 hits, compute three records, write three records. Step 3 is unchanged and still covers all
2,743 documents, so a regression in an untouched file still fails. The first run in a fresh
worktree computes all 2,743 and costs about what it does today plus the writes. Every later run
costs the listing, the lookups and the changed files.

## What this saves, honestly

On a warm cache the twelve gates above fall from about 53 seconds of CPU toward the cost of listing
and lookup, plus any selftests. Reruns are the common case: the cost note counted 61 full and 39
`--no-cargo` runs for 16 lanes in one evening, and 74 failures. A cold run saves nothing. The estimate for part 5 should read "a rerun of the text gates falls by most of 30 to 40
seconds", not "10 to 15 seconds once per run".

Two cheaper changes sit under the index and need no cache:

- `name_checks.py` opens 2,609 files more than once and `prose_ratchet.py` 1,080. Reading each once
  in-process is a few lines and saves little time, but it removes the repeat.
- `Reach` and `survey` in the naming gates compute per-file tables that other gates recompute.
  Sharing those functions between callers is the first thing the cache would store.

## Risks

- A stale record hides a finding. The key holds content and deriver source, which is the whole
  defense. A deriver that reads something else (a config file, another document) must put that in
  its key, or not be cached. The prose ratchet's baseline is an input to the verdict and never to a
  record.
- A deriver edit that the source hash misses, such as a change in a helper it calls. The
  mitigation is that each deriver names the functions it covers, and a selftest recomputes a sample
  of records and compares them to the stored ones. A mismatch fails lint.
- The cache grows. Records for old blobs stay. A run prunes records whose blob is not in the
  tree, which is one pass over a directory.
- Cargo and git state. None of this touches cargo sections or the tree.

## Proposed build order

1. Read each file once per process in `name_checks.py` and `prose_ratchet.py`. No cache.
2. Add `helpers/lint_index.py` with `get(deriver, blob)`, `put`, the key rule, atomic writes and
   pruning, and a selftest that plants a stale record, a torn file and an edited deriver and checks
   that each is a miss.
3. Move one gate onto it, the prose ratchet, and show warm and cold section times from the
   `helpers/lint-sections.sh` rows beside the verdict test the ratchet already has.
4. Move stripped Rust and the block table, then the line-fact gates, one pull request each.

Each step lands alone and changes no verdict. Step 3 is the point at which to decide whether the
measured gain justifies steps 4 and beyond.

## Questions for review

- Is a worktree-local cache under `target/` acceptable, given that `target/` is already ignored and
  already deleted by a clean?
- Should CI restore the directory? It would turn the cold run into a warm one, at the price of a
  cache key to maintain. This note does not propose it.
- Should a gate print its hit and miss counts on every run, or only on request? The proposal says
  every run.
