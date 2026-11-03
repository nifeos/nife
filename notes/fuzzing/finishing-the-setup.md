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
| Supervisor, four slots | built, running on the host at main | `helpers/continuous-fuzz.sh`, `helpers/systemd/nife-fuzz.service` |
| CI shards | built, running on every merge-queue entry | `fuzz-guests` in `.github/workflows/ci.yml`, #3046 |
| Held-findings store | built, running, mode 700 | `$NIFE_FUZZ_HOME/state/held/` |
| Daily report, local | built, installed on the host; the timer runs it at 14:00 UTC | `helpers/fuzz-daily-publish.sh`, `nife-fuzz-publish.timer` |
| Daily release (phase 2) | built, blocked on the token and on the host carrying the clean-range fix | `--publish` in the same script |
| Watchdog workflow and its label | built, scheduled, quiet until the first release; it creates the label `fuzz-loop-broken` at its first failure | `.github/workflows/fuzz-heartbeat.yml`, #3049 |
| The host follows main | built, running on the host | `helpers/fuzz-follow-main.py`, [continuous-host.md](continuous-host.md#following-main) |
| Clean-range fix | built, on the host; a slot that held a finding publishes no more ranges, because nothing yet signals that a finding turned public | `helpers/fuzz-daily-publish.sh`, [proposal](../../design/roadmap/proposals/the-clean-range-table-does-not-locate-a-held-seed.md) |
| Mint helper | built in nifeos/fuzz-host, not installed on the host | `fuzz-token` in [nifeos/fuzz-host](https://github.com/nifeos/fuzz-host), [proposal](../../design/roadmap/proposals/the-fuzz-host-mints-its-tokens-from-the-app-key.md) |
| Advisory permission | verified: an App holding only Repository security advisories write, installed on one repository, creates, completes, publishes and publicly reads an advisory | [below](#the-advisory-permission) |
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

## Stage 1: done on the host

P1 and P2 are done. Each stays here as the procedure to repeat after a rebuild of the host.

### P1. Bring the host to main (person, root)

The follower ([`continuous-host.md`](continuous-host.md#following-main)) cannot move a host that
predates it, so this step moves the host by hand once. From then on the supervisor fast-forwards to each green commit on main by itself, as the fuzz account, and
prints one journal line when a unit file changes; installing a changed unit stays this step's `diff`
and copy. The follower needs `python3` and `curl` on the host.

```sh
sudo systemctl stop nife-fuzz.service
sudo -u lane git -C /home/lane/nife fetch origin main
sudo -u lane git -C /home/lane/nife checkout --detach origin/main
for n in 0 1 2 3; do sudo -u lane git -C /home/lane/fuzz/work/$n checkout --detach origin/main; done
sudo diff /etc/systemd/system/nife-fuzz.service /home/lane/nife/helpers/systemd/nife-fuzz.service \
  && echo "unit unchanged"
sudo systemctl start nife-fuzz.service
```

The diff needs `sudo` because a normal administrator account cannot read `/home/lane`.

If it prints a difference, copy the new unit to `/etc/systemd/system/` and run `sudo systemctl
reenable nife-fuzz.service`, which rereads the unit files, before the start. Verify: `systemctl
is-active nife-fuzz.service` prints `active`; within about 5 minutes `journalctl -u
nife-fuzz.service -g fuzz-follow-main -n 5` shows a look (the clone already at main, or a move); and
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

## The advisory permission

Step P3 is this check, and it passed. An App holding only Repository security advisories read and write, installed on one repository,
creates a draft advisory, adds an affected version range to it, publishes it, and the public reads
it without a credential. A throwaway App on a throwaway repository showed it, with HTTP statuses 201
for the create, 200 for the range update, 200 for the publish and 200 for the unauthenticated read.
The App, repository and key are deleted. GitHub documents the caller as "a security manager or
administrator". An App with the advisory permission alone qualifies.

Publishing a draft needs at least one vulnerability entry that names a package (ecosystem and name)
and a `vulnerable_version_range`. Without a range GitHub answers 422, "Advisory is not publishable".
So the filer files each advisory with a package and range the host can supply, and sets
`patched_versions` as well. To repeat the test, mint an installation token for a throwaway App and
call create, then PATCH `vulnerabilities` with the range, then PATCH `state` to `published`.

The REST API has no endpoint for comments on a repository advisory (from memory, not checked), so
the test did not try the comment step that the publish milestone lists. That milestone settles it.

### Lane steps that can start now

- L1, the clean-range fix: built in `helpers/fuzz-daily-publish.sh` with its selftest, as
  [the proposal](../../design/roadmap/proposals/the-clean-range-table-does-not-locate-a-held-seed.md)
  describes, except that withheld ranges do not return. It is on the host and unblocks phase 2.
- L2, the host follows main: built in `helpers/fuzz-follow-main.py` and the supervisor. It is on the
  host.
- L3, the mint helper: built as `fuzz-token` in [nifeos/fuzz-host](https://github.com/nifeos/fuzz-host),
  from [the proposal](../../design/roadmap/proposals/the-fuzz-host-mints-its-tokens-from-the-app-key.md).
  It reads the key at `/etc/nife-fuzz/nife-fuzz.private-key.pem` and mints one token per service:
  Contents write alone for the daily release, Repository security advisories write alone for the
  filer, each on nifeos/nife alone. Its own unit, timer and install section live in that
  repository.
- L4, the filer: build
  [the proposal](../../design/roadmap/proposals/the-fuzz-host-files-held-findings-as-advisories.md)
  to its selftest; a real run waits on P4 and L3.
- L5, fix lanes read the advisory: the script, the skill rule text for held-finding lanes, and the
  lint for replay lines, from [the proposal](../../design/roadmap/proposals/fix-lanes-read-the-advisory.md).
- L6, the reader token helper, from
  [the proposal](../../design/roadmap/proposals/fix-lanes-get-a-one-hour-reader-token.md).
- L7, a proposal for the host's replay of a red CI shard, which has no file yet.

## Stage 2: after L3 merges and the architect gives the go

### P4. Generate the `nife-fuzz` key and place it on the host (person, root)

1. At <https://github.com/organizations/nifeos/settings/apps/nife-fuzz>, under Credentials, Key pairs,
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
5. Install the mint helper as the install section of nifeos/fuzz-host's
   [`docs/fuzz-token.md`](https://github.com/nifeos/fuzz-host/blob/main/docs/fuzz-token.md) says:
   a group `fuzz-token-publish` that holds `lane` alone, a root-owned copy at
   `/usr/local/sbin/fuzz-token`, outside `/home/lane/nife`, and `fuzz-token@publish.timer` enabled.
   Verify with `sudo /usr/local/sbin/fuzz-token check publish`, which prints the token's repository
   list and expiry and never the token. It must list `nifeos/nife` alone.

If the fingerprint differs, the file on the host is not the key you generated: delete it on the host
and on GitHub, and start again at step 1. If the mint check lists any repository besides
`nifeos/nife`, open
<https://github.com/organizations/nifeos/settings/installations/177641059> and set Only select
repositories to `nifeos/nife` alone.

## Stage 3: after P4 holds

### P5. Turn on the daily release (person, root)

```sh
sudo systemctl edit nife-fuzz-publish.service
```

In the drop-in, paste:

```ini
[Unit]
Wants=fuzz-token@publish.service
After=fuzz-token@publish.service

[Service]
EnvironmentFile=/run/fuzz-token/publish.env
Environment=NIFE_FUZZ_PUBLISH=1
ExecStart=
ExecStart=/home/lane/nife/helpers/fuzz-daily-publish.sh --publish
```

The `Wants=` and `After=` lines mint a fresh token before each run, and systemd reads the token file
as root before the service starts as `lane`. Save, then `sudo systemctl
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
<https://github.com/organizations/nifeos/settings/apps/nife-fuzz-reader>, under Credentials, Key
pairs. Place it in a directory
only root can enter, on the machine that launches fix lanes. The file is owned by root, mode 0400,
at the path the helper's install section names. Verify the mode and fingerprint as in P4. It never goes to the
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
  further than its App, so the mint helper mints one token per service: Contents alone for the
  release, advisories alone for the filer.
