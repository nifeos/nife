# Code scanning: what CodeQL looks at, and what became of its alerts

GitHub's CodeQL ran on every push and pull request through default setup, with no workflow file
in the tree, until calef ordered the move to advanced setup on 2026-10-08 (UTC); the committed
workflow is `.github/workflows/codeql.yml`. It scans each push to `main` and no pull request, so
an alert opens after the merge; the workflow's trigger comment says why. DECISIONS §36 (the repository is part of the TCB)
says why the repository is scanned at all; its stated trigger for leaving default setup is what
fired. Every
alert gets one of the three dispositions in DECISIONS §35 (what a scanner is for here, and how
its findings get dispositioned): fixed, dismissed with a written reason, or deferred to a
milestone. This note records the dispositions where a reader of the tree meets them, because
GitHub's dismissal comments are invisible from the source and do not survive a change of tool.

## Which languages are scanned

The workflow's matrix analyzes **actions, c-cpp, python and rust**, the four default setup
analyzed before it. Default setup's own configuration is still worth reading, because it must be
OFF before the workflow's uploads are accepted (below):

```
gh api repos/nifeos/nife/code-scanning/default-setup --jq '{state, languages}'
```

Rust was off for about four hours on 2026-10-07 (UTC). Six `Analyze (rust)` jobs started between
16:47 and 16:54 UTC failed about 17 seconds into the SARIF upload, with no error text in the log
(for example run [37655236254](https://github.com/nifeos/nife/actions/runs/37655236254) on
`main`), so calef removed Rust from default setup. All twelve Rust jobs outside that window
succeeded, seven of them running concurrently, and extraction was identical in passing and failing
jobs. That points at a transient upload incident on GitHub's side, but the cause is unconfirmed.
At about 20:40 UTC default setup was toggled off and on with no language list (the API rejects
`rust` by name; auto-detection includes it), and run
[37684382643](https://github.com/nifeos/nife/actions/runs/37684382643) on `main` analyzed Rust
successfully.

The 43 Rust alerts open that day came from analyses before the gap. They were each read and
dismissed by hand, below; a dismissed alert stays dismissed when later analyses report it again.

§36's caveat still applies to every Rust result: the extractor runs against the host target with
default features, for a kernel that does not build for the host, and reports macro expansion
failures across the tree. A clean Rust result means less than it looks.

## The 2026-10-07 triage

44 alerts open, all read at the alert's commit (`f8feff7f3` for the 43 Rust alerts, `9c9e16a49`
for the Python one) and at `main`. No flagged line had moved. One was a real defect; the other 43
were dismissed, each with a comment naming the file and line and why.

| Rule | Count | Disposition | Reasoning class |
|---|---|---|---|
| `py/redos` | 1 | Fixed (#26) | Real. See below. |
| `rust/hard-coded-cryptographic-value` | 23 | Used in tests | Fixture salts and the phc-winner-argon2 reference vector inside `#[cfg(test)] mod tests` in `crates/credentialer/src/lib.rs` |
| `rust/hard-coded-cryptographic-value` | 2 | Used in tests | GCM test case 2's all-zero key and IV in `cryptography_exerciser`, a known-answer program |
| `rust/hard-coded-cryptographic-value` | 2 | False positive | `components/src/credentialer.rs:216` and `:286`: zeroed buffers that `fill()` overwrites from the entropy service before use, or the service refuses |
| `rust/hard-coded-cryptographic-value` | 1 | False positive | `vendor/redoxfs/src/key.rs:71`: a zeroed output buffer Argon2 writes into |
| `rust/access-invalid-pointer` | 9 | False positive | `uefi_loader`: UEFI protocol out-pointers dereferenced only after `SUCCESS` and an `is_null()` check. CodeQL tracks the `ptr::null_mut()` initializer and does not see the firmware write. Firmware is in the trusted computing base, and no untrusted input reaches these pointers |
| `rust/cleartext-logging` | 4 | False positive | `vendor/redoxfs/src/bin/` host tools printing a filesystem UUID, sizes and paths, none derived from the password |
| `rust/cleartext-logging` | 1 | False positive | `kernel/src/arch/x86_64/timer.rs:424`: a TSC rate in Hz in a boot assert |
| `rust/cleartext-logging` | 1 | Used in tests | `crates/credentialer/src/lib.rs:809`: a test assert printing a fixture secret |

Totals: 1 fixed, 26 dismissed as used in tests, 17 as false positives. None of the flagged code had
been removed.

**Alert 26 was real.** `helpers/rust_source.py`'s comment-and-literal stripper matched a raw
string's body with `(?:.|\n)*?` under `re.S`, where `.` already matches a newline. Every newline
had two ways to match, so an unterminated `r"` followed by n newlines cost 2^n steps: 24 newlines
took about a second, 27 took seven. The input is any Rust file a branch adds, and `script/lint`
and `script/metrics` both run it, so one such file would have hung the gate rather than failed it.
The fix is `.*?`, the same language in linear time, and `python3 helpers/rust_source.py
--selftest` (run by `script/lint`) fails the old pattern. Python is still scanned, so the alert
closes on the first analysis of `main` after the fix lands.

The verdicts on the pointer alerts are from reading code, not from trying to break it. An attempt to
reach those dereferences with hostile firmware input was out of scope, and the argument rests on
the firmware being trusted, which is the boot model's assumption rather than something the loader
checks.

## BUGS

- The cause of the 2026-10-07 upload failures is unconfirmed. The workflow retries the upload
  once, in a second job 90 seconds later, which answers a transient repeat; a persistent failure
  still goes red. A same-job retry is impossible: the action refuses a second upload per job per
  tool and category (watched 2026-10-09, run 37863703201).
- Alert continuity rests on the category match: the workflow uploads under `/language:<name>`,
  default setup's own category (read from the analyses API, 2026-10-09 UTC). Accepted: the first
  post-toggle uploads landed under those categories on 2026-10-09 (UTC, run 37867349621's
  rerun), so the 43 hand dismissals below carry.
- §35 asks for a dismissal's reason at the code. For the 17 false positives the existing `SAFETY`
  and provenance comments at each site carry the argument, and this note carries the rest; no
  per-line suppression comment was added, because CodeQL reads none.

## Advanced setup, decided 2026-10-08

calef ordered option 1 on 2026-10-08 (UTC), the recommendation this section's earlier form
carried; option 2 (stay on default setup) is refused with it. The committed workflow keeps the
four languages and retries the SARIF upload. It excludes `vendor/**`, the §36 trigger. Every
language runs in `build-mode: none`: Rust because the kernel does not build for the host, the C
fixtures because they have no build. And it skips analysis on a prose-only diff: on #1870,
`Analyze (rust)` spent eight minutes on a six-file roadmap-docs pull request.

The toggle. Default setup must be OFF before the workflow's uploads are accepted, and toggling
it is calef's admin act in repository settings, outside this tree (§36 records why settings are
not committed). Until then every upload is rejected and the upload jobs are red on purpose;
CodeQL is not a required check, so that red blocks no merge. The first observed rejection is run
[37863703201](https://github.com/nifeos/nife/actions/runs/37863703201) (2026-10-09 UTC), on the
lane that landed the workflow. calef toggled default setup off on 2026-10-09 (UTC), between 00:52
and 00:57: PR #1871 merged at 00:57:06 and its push run started two seconds later, so that run
raced the change.

It went green, with one wrinkle worth a rerun before a diagnosis. The merge run's uploads, at
about 01:07, were still rejected with the same "default setup is enabled" message ten minutes
after the toggle; GitHub's upload path lags the settings change. A rerun of the failed jobs
alone, about 100 minutes later, went green on every language (run
[37867349621](https://github.com/nifeos/nife/actions/runs/37867349621), rerun). So: a red upload
soon after the toggle is propagation until a rerun says otherwise.
