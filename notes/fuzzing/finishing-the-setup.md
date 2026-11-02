# Finishing the continuous fuzz setup: what runs, what is left, and who does each step

*Name: provisional, minted 2026-11-02 (UTC) by maintainer/fuzz-finish-setup-runbook. No architect
has ruled on it.*

An appendix to [`continuous-host.md`](continuous-host.md). The architect ruled the findings path on
2026-11-01 (UTC) in
[`fuzz-findings-without-a-person`](../../design/roadmap/proposals/fuzz-findings-without-a-person.md):
after a person places one credential once, the host files, reruns and publishes held findings with no
person in the loop. This page lists what exists, what remains, and the order. The steps that only a
person can take come first in each stage, with commands to paste.

Every step that needs root on the host also works as a written spec handed to the agent that
maintains the host, except the key steps, which a person does by hand. No step needs an organization
secret or variable.

## What exists and what does not

| piece | state | where |
|---|---|---|
| Supervisor, four slots | built, running on the host at a commit 251 commits behind main | `helpers/continuous-fuzz.sh`, `helpers/systemd/nife-fuzz.service` |
| CI shards | built, running on every merge-queue entry | `fuzz-guests` in `.github/workflows/ci.yml`, #3046 |
| Held-findings store | built, running, mode 700 | `$NIFE_FUZZ_HOME/state/held/` |
| Daily report, local | built, not installed on the host | `helpers/fuzz-daily-publish.sh`, `nife-fuzz-publish.timer` |
| Daily release (phase 2) | built, blocked on the clean-range fix and the token | `--publish` in the same script |
| Watchdog workflow and its label | built, scheduled, quiet until the first release; it creates the label `fuzz-loop-broken` at its first failure | `.github/workflows/fuzz-heartbeat.yml`, #3049 |
| The host follows main | ruled, not built | [proposal](../../design/roadmap/proposals/the-fuzz-host-follows-main.md) |
| Clean-range fix | not built; it blocks phase 2 | [proposal](../../design/roadmap/proposals/the-clean-range-table-does-not-locate-a-held-seed.md) |
| Mint helper | not built | [proposal](../../design/roadmap/proposals/the-fuzz-host-mints-its-tokens-from-the-app-key.md) |
| Advisory permission test | not run; a person runs it, step P3 below | [proposal](../../design/roadmap/proposals/an-app-with-only-the-advisory-permission-files-an-advisory.md) |
| Finding filer | not built | [proposal](../../design/roadmap/proposals/the-fuzz-host-files-held-findings-as-advisories.md) |
| Fix lanes read the advisory: script, skill rule text, replay-line lint | not built; §90 (the claim is a draft pull request) carries the amendment, the skills do not | [proposal](../../design/roadmap/proposals/fix-lanes-read-the-advisory.md) |
| Reader token helper | not built | [proposal](../../design/roadmap/proposals/fix-lanes-get-a-one-hour-reader-token.md) |
| Publish after a clean rerun | not built; waits on the filer | [proposal](../../design/roadmap/proposals/a-held-finding-turns-public-after-its-fix-reruns-clean.md) |
| Host replays a red CI shard | not built, and no proposal file exists | [the CI proposal's follow-on](../../design/roadmap/proposals/a-short-sharded-fuzz-pass-in-ci.md) |
| `nife-fuzz` App | created, installed, no key | App ID 5571801, installation 177641059 |
| `nife-fuzz-reader` App | created, installed, no key | App ID 5571868, installation 177642435 |

Neither ID is secret. The installations hold exactly the ruled permissions: `nife-fuzz` holds
Contents write, Repository security advisories write and Metadata read; `nife-fuzz-reader` holds
Repository security advisories read and Metadata read.

## Stage 1: now, with nothing blocking

### P1. Bring the host to main (person, root)

The host runs the commit it was installed at. It has no self-update yet, and a restart alone moves
nothing: each slot keeps its own worktree at the commit it started on. The supervisor's toolchain
step from #3045 is missing until this runs, and the self-update code can only arrive this way the
first time.

```sh
sudo systemctl stop nife-fuzz.service
sudo -u lane git -C /home/lane/nife fetch origin main
sudo -u lane git -C /home/lane/nife checkout --detach origin/main
for n in 0 1 2 3; do sudo -u lane git -C /home/lane/fuzz/work/$n checkout --detach origin/main; done
diff /etc/systemd/system/nife-fuzz.service /home/lane/nife/helpers/systemd/nife-fuzz.service \
  && echo "unit unchanged"
sudo systemctl start nife-fuzz.service
```

If `diff` prints a difference, copy the new unit to `/etc/systemd/system/` and run `sudo systemctl
reenable nife-fuzz.service`, which rereads the unit files, before the start. Verify: `systemctl is-active nife-fuzz.service` prints `active`, and
after the next window the last column of `tail -1 /home/lane/fuzz/state/windows.tsv | cut -f7,8`
shows the new short commit. If the service fails, `journalctl -u nife-fuzz.service -n 50` names the
cause; the toolchain step and the KVM check each print their own refusal.

### P2. Install the local daily report (person, root)

This writes `report.md` and `clean-ranges.tsv` under `/home/lane/fuzz/out/` once a day and publishes
nothing, so it is safe before the clean-range fix.

```sh
sudo cp /home/lane/nife/helpers/systemd/nife-fuzz-publish.service \
        /home/lane/nife/helpers/systemd/nife-fuzz-publish.timer /etc/systemd/system/
sudo systemctl enable --now nife-fuzz-publish.timer
```

`enable` rereads the unit files itself.

Verify: `systemctl list-timers nife-fuzz-publish.timer` shows the next run at 14:00 UTC, and `sudo
systemctl start nife-fuzz-publish.service` followed by `ls /home/lane/fuzz/out/` shows today's date.

### P3. The advisory permission test, on a throwaway repository (person)

GitHub's documentation names "a security manager or administrator" as the caller that creates a
repository security advisory, and does not say whether an App with the advisory permission alone
qualifies. The filer depends on the answer, so the `nife-fuzz` key waits for it. A person runs this,
because it creates a repository and an App and handles a key. Run it on your own machine, not the
fuzz host.

1. Create a public repository `nifeos/advisory-permission-test` with a README.
2. At <https://github.com/organizations/nifeos/settings/apps/new> create an App named
   `nife-advisory-test`, webhook off, Repository security advisories Read and write, every other
   permission No access, installable only on this account. Note its App ID.
3. Install it on `nifeos/advisory-permission-test` alone. The installation page's address ends in
   the installation ID.
4. Generate a private key on the App's page and `chmod 600` the downloaded file.
5. In one bash or zsh terminal, set the three values and run the block. It prints HTTP status codes
   and the advisory's id, never the key or a token.

```sh
KEY=~/Downloads/nife-advisory-test.<date>.private-key.pem   # the file from step 4
APP_ID=<App ID from step 2>; INST_ID=<installation ID from step 3>
REPO=nifeos/advisory-permission-test; API=https://api.github.com
b64url() { openssl base64 -A | tr '+/' '-_' | tr -d '='; }
now=$(date +%s)
h=$(printf '{"alg":"RS256","typ":"JWT"}' | b64url)
c=$(printf '{"iat":%d,"exp":%d,"iss":"%s"}' $((now-60)) $((now+540)) "$APP_ID" | b64url)
s=$(printf '%s.%s' "$h" "$c" | openssl dgst -sha256 -sign "$KEY" -binary | b64url)
TOKEN=$(curl -sS -X POST -H "Authorization: Bearer $h.$c.$s" \
  "$API/app/installations/$INST_ID/access_tokens" | python3 -c 'import json,sys; print(json.load(sys.stdin)["token"])')
auth=(-H "Authorization: Bearer $TOKEN" -H "Accept: application/vnd.github+json")
curl -sS -o /tmp/adv-test.json -w 'create %{http_code}\n' "${auth[@]}" -X POST \
  "$API/repos/$REPO/security-advisories" -d '{"summary":"Permission test, not a vulnerability",
  "description":"A test of App permissions. Nothing here is vulnerable.","severity":"low",
  "vulnerabilities":[{"package":{"ecosystem":"other","name":"permission-test"}}]}'
GHSA=$(python3 -c 'import json; print(json.load(open("/tmp/adv-test.json")).get("ghsa_id",""))'); echo "id $GHSA"
curl -sS -o /dev/null -w 'publish %{http_code}\n' "${auth[@]}" -X PATCH \
  "$API/repos/$REPO/security-advisories/$GHSA" -d '{"state":"published"}'
curl -sS -o /dev/null -w 'public read %{http_code}\n' "$API/repos/$REPO/security-advisories/$GHSA"
unset TOKEN
```

Pass: `create 201`, `publish 200`, `public read 200`. A `create 403` or `404` means the advisory
permission alone does not suffice; stop there, keep the `nife-fuzz` key ungenerated, and put the
statuses to the architect, who decides what the App needs instead. A failure before `create` (no
token) means a wrong App ID, installation ID or key file; fix the value and rerun.

6. Delete the App (its settings page, Advanced, Delete) and the repository (its settings, Delete
   this repository), then delete the key file and `/tmp/adv-test.json`.
7. Post the three status lines, and nothing else, on the pull request that records the result; a
   lane writes them into [`the-fuzz-app.md`](the-fuzz-app.md) and the filer's proposal.

The REST API has no endpoint for comments on a repository advisory (from memory, not checked),
so this test does not try the comment step the publish milestone lists. That milestone settles it.

### Lane steps that can start now

- L1, the clean-range fix: build
  [the proposal](../../design/roadmap/proposals/the-clean-range-table-does-not-locate-a-held-seed.md)
  in `helpers/fuzz-daily-publish.sh` with its selftest. It unblocks phase 2.
- L2, the host follows main: build
  [the proposal](../../design/roadmap/proposals/the-fuzz-host-follows-main.md), including moving the
  slot worktrees, which a restart does not do today. It needs P1 once to reach the host.
- L3, the mint helper: build
  [the proposal](../../design/roadmap/proposals/the-fuzz-host-mints-its-tokens-from-the-app-key.md),
  reading the key at `/etc/nife-fuzz/nife-fuzz.private-key.pem`, shipped with its own unit and timer
  and an install section that a person follows outside the clone.
- L4, the filer: build
  [the proposal](../../design/roadmap/proposals/the-fuzz-host-files-held-findings-as-advisories.md)
  to its selftest; a real run waits on P3, P4 and L3.
- L5, fix lanes read the advisory: the script, the skill rule text for held-finding lanes, and the
  lint for replay lines, from [the proposal](../../design/roadmap/proposals/fix-lanes-read-the-advisory.md).
- L6, the reader token helper, from
  [the proposal](../../design/roadmap/proposals/fix-lanes-get-a-one-hour-reader-token.md).
- L7, a proposal for the host's replay of a red CI shard, which has no file yet.

## Stage 2: after P3 passes, L3 merges, and the architect gives the go

### P4. Generate the `nife-fuzz` key and place it on the host (person, root)

1. At <https://github.com/organizations/nifeos/settings/apps/nife-fuzz>, under Private keys,
   generate a private key. Note the SHA256 fingerprint GitHub shows beside it. Run `chmod 600` on
   the download at once.
2. Copy it to your own account on the host and move it into the root-only directory, which already
   exists with mode 0700:

```sh
scp ~/Downloads/nife-fuzz.<date>.private-key.pem <you>@<fuzz host>:nife-fuzz.pem
ssh -t <you>@<fuzz host> 'sudo install -o root -g root -m 0400 ~/nife-fuzz.pem \
  /etc/nife-fuzz/nife-fuzz.private-key.pem && shred -u ~/nife-fuzz.pem'
```

3. Delete the download and empty the trash.
4. Verify without showing the key. After each command, the text after the colon is its expected
   output:
   - `sudo stat -c '%U %G %a' /etc/nife-fuzz/nife-fuzz.private-key.pem`: `root root 400`.
   - `sudo -u lane cat /etc/nife-fuzz/nife-fuzz.private-key.pem >/dev/null 2>&1 || echo refused`:
     `refused`.
   - `sudo openssl rsa -in /etc/nife-fuzz/nife-fuzz.private-key.pem -pubout -outform DER | openssl
     sha256 -binary | openssl base64`: the fingerprint from step 1, without its `SHA256:` prefix.
5. Install the mint helper as its own install section says, outside `/home/lane/nife`, and enable its
   timer. Verify with the helper's own check, which prints the token's repository list and expiry
   and never the token. It must list `nifeos/nife` alone.

If the fingerprint differs, the file on the host is not the key you generated: delete it on the host
and on GitHub, and start again at step 1. If the mint check lists any repository besides
`nifeos/nife`, open
<https://github.com/organizations/nifeos/settings/installations/177641059> and set Only select
repositories to `nifeos/nife` alone.

## Stage 3: after L1 is on the host and P4 holds

### P5. Turn on the daily release (person, root)

```sh
sudo systemctl edit nife-fuzz-publish.service
```

In the drop-in, name the token file the mint helper writes with `EnvironmentFile=`, add
`Environment=NIFE_FUZZ_PUBLISH=1`, and replace the start line: `ExecStart=` on its own line, then
`ExecStart=/home/lane/nife/helpers/fuzz-daily-publish.sh --publish`. Save, then `sudo systemctl
start nife-fuzz-publish.service`. Verify: a release `fuzz-<today>` appears at
<https://github.com/nifeos/nife/releases>, made by `nife-fuzz[bot]`, and the watchdog stops passing
quietly from that day on. If the start fails, `journalctl -u nife-fuzz-publish.service -n 30`
names the refusal: a missing `NIFE_FUZZ_PUBLISH`, a stale token or a `gh` error.

### P6. Turn on the filer, then the rerun and publish (person, root)

After L4 merges, install its unit and timer as its install section says. Verify that one draft
advisory per held finding appears under <https://github.com/nifeos/nife/security/advisories>, and
that the journal names advisory ids and ages only. Do the same for the rerun-and-publish service once
its milestone merges.

### P7. Place the reader key (person, root on the machine that launches fix lanes)

After L6 merges, generate a key at
<https://github.com/organizations/nifeos/settings/apps/nife-fuzz-reader> and place it in a directory
only root can enter on the machine that launches fix lanes, owned by root, mode 0400, at the path
the helper's install section names. Verify the mode and fingerprint as in P4. It never goes to the
fuzz host.

## Risks

- A key placed before the mint helper exists does nothing, and nothing reads it. The danger is the
  shortcut it invites. A token minted by hand into a static file lasts one hour, so the next day's
  release fails, and publishing before L1 puts a held seed in public, one less than the first seed of
  the next clean range.
- A leaked key mints tokens until someone revokes it. With Contents write a token can create, move
  or delete any `fuzz-*` tag, create, edit or delete any release, and create, rewrite or delete any
  branch except main. Main requires a pull request with no bypass actor. The main ruleset requires
  zero approving reviews, so a token that pushes to an open pull request's branch with auto-merge
  armed can reach main through the merge queue when its checks pass. A change to workflow files needs
  the Workflows permission, which the App lacks. With advisories write a token can publish any held
  finding early.
- Revoke: suspend the installation at once, at
  <https://github.com/organizations/nifeos/settings/installations/177641059> (Suspend), which stops
  every token. Then delete the key on the App's page, place a fresh one as in P4, and unsuspend.
- The permissions match the ruling and are the least a single App can hold for the ruled path: a
  release needs Contents write, and filing and publishing need advisories write. A token can narrow
  further than its App, so the mint helper can mint one token per service: Contents alone for the
  release, advisories alone for the filer.
