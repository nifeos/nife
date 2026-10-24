# CI's toolchain download, 2026-10-19 (provisional name)

*Name: provisional, minted 2026-10-19 (UTC) by maintainer/rustup-retry.*

Measured 2026-10-19 (UTC) by maintainer/rustup-retry, after a docs-only pull request (#2379) went red
because one TCP connect to static.rust-lang.org timed out. Two questions: how often does the
download fail CI, and should the toolchain be cached? The constraints are those of
[actions-cache-budget-2026-10-07.md](actions-cache-budget-2026-10-07.md) and calef's rulings on
#1814.

## 1. How often the download fails

Every failed job from 2026-10-04 to 2026-10-19 00:00 in the ten workflows that install a toolchain
(ci.yml, verify.yml, mutation-gate.yml, stick-maker-hosts.yml and six others) was read, including
the earlier attempts of runs that were rerun. That is 423 job logs from 343 runs, searched for
rustup's download errors.

| When (UTC) | Job | What failed | Cost |
|---|---|---|---|
| 2026-10-08 01:38 | clippy, merge group of #1843 | checksum mismatch on the floating `nightly` manifest, the CDN mid-publish | ejected from the queue; requeued 14 minutes later and merged |
| 2026-10-18 23:52 | swish-check-x86_64, #2379 | TCP connect timed out (30 s) on the pinned manifest's `.sha256` | one rerun |

Two in two weeks, out of about 3,300 pull request and merge-group runs of ci.yml. Both cleared on the
next attempt. rustup made one attempt each time.

The 2026-10-08 one came from a toolchain no job needed. Five ci.yml jobs ran `rustup toolchain
install nightly`, which is the floating channel, not the pin; cargo then fetched the pin on first use
outside any step that could retry. The floating install cost a median 9.9 s per job (n=129).

## 2. What changed

`helpers/rustup-retry.sh` runs rustup up to four times, waiting 10, 30 and 90 s, and only when the
output looks like a download failure. A real error fails at once. On the last failure it prints the
URL and the attempt count, and in Actions an `::error` annotation. `script/bootstrap` and every
direct rustup step in ci.yml, mutation-gate.yml and stick-maker-hosts.yml go through it, and the
ci.yml steps install the pin by `toolchain install` with no name. `helpers/rustup-retry-selftest.sh`
in `script/lint` holds it against a stub rustup, and was watched failing with the classifier broken
and with the bound removed.

Proved by hand with `RUSTUP_DIST_SERVER=http://127.0.0.1:9` and a scratch `RUSTUP_HOME`: four
attempts, then `failed 4 of 4 attempts fetching http://127.0.0.1:9/dist/2026-10-18/channel-rust-nightly.toml.sha256`
and exit 1. A non-download error (`--no-such-flag`) failed after one attempt.

## 3. The toolchain is not cached

CI does not cache the pinned toolchain in the Actions cache. calef ruled it on 2026-10-19 (UTC),
recorded on #2386: "No". Ruling (b) on #1814 stands. The measurement below is what he ruled on.

Measured against the cache as it stood at 2026-10-19 00:20 UTC: 10.72 GB in 408 entries, against
GitHub's 10 GB limit, with 10.0 GB of it in pull request rust-cache entries. The eviction horizon (the
least recently used entry's idle time) was 44.1 hours. Milestone 822 (the Actions cache horizon is charted) is not started, so these come
from one read of `gh api repos/nifeos/nife/actions/caches`, not from its chart.

| Quantity | Value | Source |
|---|---|---|
| Pin's download, every component and target in rust-toolchain.toml | 281 MB on an x86_64 runner, 257 MB on arm64 | `Content-Length` of each `.tar.xz` in the 2026-10-18 manifest |
| The same, installed | 1,440 MB on disk (x86_64 host) | `rustup toolchain install --force-non-host` into a scratch home |
| A cache entry of it | 339 MB (x86_64 host); arm64 not measured, about 310 MB by the download ratio | `tar \| zstd -T0 --long=30 -3`, what actions/cache runs |
| Install time in CI | median 15.2 s, p10 12.2, p90 18.5, max 39.8 (n=96) | rustup's own lines in the job logs above |
| Restore time | not measured in CI. 126 MB QEMU entries restore in a median 1.8 s (n=130); extracting this one took 10.3 s on a Mac | job logs; local `zstd -d \| tar -x` |
| Bytes held | about 0.65 GB per pin for both runner pools, and two pins are usually live: about 1.3 GB, 12% of the limit | the rows above |
| Projected hit rate | about 98.7% | below |

**Hit rate.** A pull request run and a merge group restore from `main`. The `main-caches` job
(milestone 870 (main keeps its CI caches warm)) runs on every push to `main` and could save the entry under the new pin within a
minute or two of the bump landing. Push runs take a median 0.3 minutes today (n=30). Over the 14
days measured, 14 bumps landed on `main` and ci.yml ran 2,524 pull request and 785 merge-group runs.
The misses would be the bump's own runs (24 pull request runs on the two bump branches, about 14
merge groups) and runs that start in the minute or two before `main` holds the new entry: about 42 of
3,309. Daily bumps cost a cache miss per bump, not a cold cache.

**What a hit buys.** At most the 15.2 s install, minus a restore that downloads 339 MB and writes the
same 1,440 MB of files rustup writes. How the 15.2 s splits between download and extraction was
not measured; the saving is somewhere between zero and about ten seconds a job, on no job's critical
path. Against
the download failures, the retry above already turns the two observed failures into passes, and a
cache restore that fails falls through to the same install.

**A CI container image with the toolchain baked in.** Not built. The image would carry the same
roughly 340 MB compressed and 1.4 GB unpacked, so a pull cannot beat a 15 s install by more than the
restore can. It also adds a daily image build per pin and a registry, plus `/dev/kvm` passthrough for
swish-check-x86_64. It would not use the Actions cache budget.

**Bumping less often.** Each job downloads the toolchain on every run whatever the cadence, so a
slower cadence changes neither the exposure to the CDN nor the time. With a cache it would change the
misses (about 38 of the 42 come from bump runs) and the bytes (one live pin instead of two, about
0.65 GB). toolchain-bump.yml opens the bump at 08:00 UTC daily and arms auto-merge.

Caveats: the failure counts rest on two events, and the restore cost is an estimate from a different
disk. A baked image and a slower cadence were weighed only as companions to a cache, so with no
cache neither was put to calef.
