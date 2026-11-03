# Finishing the continuous fuzz setup: what runs, what is left, and who does each step

*Name: provisional, minted 2026-11-02 (UTC) by maintainer/fuzz-finish-setup-runbook. No architect
has ruled on it.*

An appendix to [`continuous-host.md`](continuous-host.md). The architect ruled the findings path on
2026-11-01 (UTC) in
[`fuzz-findings-without-a-person`](../../design/roadmap/1015-fuzz-findings-without-a-person.md):
after a person places one credential once, the host files, reruns and publishes held findings with no
person in the loop. This page lists what exists, what is not built, and the order of the steps left
to a person. Each step has commands to paste and the output to expect.

Every step that needs root also works as a written spec handed to the agent that maintains the host,
except the two key steps, which a person does by hand. No step needs an organization secret or
variable.

## What exists and what does not

| piece | state | where |
|---|---|---|
| Supervisor, four slots | built, running on the host at main | `helpers/continuous-fuzz.sh`, `helpers/systemd/nife-fuzz.service` |
| CI shards | built, running on every merge-queue entry | `fuzz-guests` in `.github/workflows/ci.yml`, #3046 |
| Held-findings store | built, running, mode 700 | `$NIFE_FUZZ_HOME/state/held/` |
| Daily report, local | built, installed on the host; the timer runs it at 14:00 UTC | `helpers/fuzz-daily-publish.sh`, `nife-fuzz-publish.timer` |
| Daily release (phase 2) | built, waits on the key and on steps P5 to P7 | `--publish` in the same script |
| Watchdog workflow and its label | built, scheduled, quiet until the first release; it creates the label `fuzz-loop-broken` at its first failure | `.github/workflows/fuzz-heartbeat.yml`, #3049 |
| The host follows main | built, running on the host | `helpers/fuzz-follow-main.py`, [continuous-host.md](continuous-host.md#following-main) |
| Clean-range fix | built, on the host; a slot that held a finding publishes no more ranges, because nothing yet signals that a finding turned public | `helpers/fuzz-daily-publish.sh`, [milestone](../../design/roadmap/1016-the-clean-range-table-does-not-locate-a-held-seed.md) |
| Mint helper | built and merged in nifeos/fuzz-host (pull requests 2 and 3), not installed | `fuzz-token` and [docs/fuzz-token.md](https://github.com/nifeos/fuzz-host/blob/main/docs/fuzz-token.md), [milestone](../../design/roadmap/1022-the-fuzz-host-mints-its-tokens-from-the-app-key.md) |
| Advisory permission | verified: an App holding only Repository security advisories write, installed on one repository, creates, completes, publishes and publicly reads an advisory | [below](#the-advisory-permission) |
| Finding filer | built and merged in nifeos/fuzz-host (pull request 5), not installed; it runs as `lane`; unverified against GitHub and systemd | `fuzz-filer` and [docs/fuzz-filer.md](https://github.com/nifeos/fuzz-host/blob/main/docs/fuzz-filer.md), [milestone](../../design/roadmap/1020-the-fuzz-host-files-held-findings-as-advisories.md) |
| Fix lanes read the advisory: script, skill rule text, replay-line lint | built and merged | `script/read-advisory`, `helpers/replay-line-check.sh`, the developer-lane skill, [milestone](../../design/roadmap/1014-fix-lanes-read-the-advisory.md) |
| Reader token helper | built and merged in nifeos/fuzz-host (pull request 4) as the `reader` profile of `fuzz-token`, with `issue reader` for the sudo rule; not installed; unverified under a real sudo rule | [docs/fuzz-token-reader.md](https://github.com/nifeos/fuzz-host/blob/main/docs/fuzz-token-reader.md), [milestone](../../design/roadmap/1013-fix-lanes-get-a-one-hour-reader-token.md) |
| Publish after a clean rerun | not built; reads the filer's ledger | [milestone 1010](../../design/roadmap/1010-a-held-finding-turns-public-after-its-fix-reruns-clean.md) |
| Host replays a red CI shard | not built | [proposal](../../design/roadmap/proposals/the-fuzz-host-replays-a-red-ci-shard.md) |
| `nife-fuzz` App | created, installed, no key placed | App ID 5571801, installation 177641059 |
| `nife-fuzz-reader` App | created, installed, no key placed | App ID 5571868, installation 177642435 |

Neither ID is secret. The installations hold exactly the ruled permissions: `nife-fuzz` holds
Contents write, Repository security advisories write and Metadata read; `nife-fuzz-reader` holds
Repository security advisories read and Metadata read.

## P1 to P3: done on the host

P1 to P3 are done. Each stays here as the procedure to repeat after a rebuild of the host.

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

Step P3 is this check, and it passed. An App holding only Repository security advisories read and
write, installed on one repository, creates a draft advisory, adds an affected version range to it,
publishes it, and the public reads it without a credential. A throwaway App on a throwaway repository showed it, with HTTP statuses 201
for the create, 200 for the range update, 200 for the publish and 200 for the unauthenticated read.
The App, repository and key are deleted. GitHub documents the caller as "a security manager or
administrator". An App with the advisory permission alone qualifies.

Publishing a draft needs at least one vulnerability entry that names a package (ecosystem and name)
and a `vulnerable_version_range`. Without a range GitHub answers 422, "Advisory is not publishable".
So the filer files each advisory with a package and range the host can supply. It leaves
`patched_versions` unset, because no fix exists at filing; milestone 1010 sets it. To repeat the
test, mint an installation token for a throwaway App and call create, then PATCH `vulnerabilities`
with the range, then PATCH `state` to `published`.

## The sequence for a person: P4 to P11

Every piece below is built and merged, and none is installed. P5 and P10 place a key, and only a
person does them. Every other step may go to the agent that maintains the host as a written spec:
hand it the step, its commands and its verify lines, and have it report the output. That agent runs
P4 to P8 on the fuzz host and P9 and P11 on the machine that launches fix lanes. It never holds a
key, and it asks for nothing but the root those steps need.

The first step to take yourself is P5. P4 can run before it or in parallel.

Two machines take part. The **fuzz host** gets the `nife-fuzz` key (P5) and the publish and filer
helpers (P6 to P8). The **fix-lane machine** is the one that launches fix lanes; it gets the
`nife-fuzz-reader` key (P10) and the reader rule. The reader key never goes to the fuzz host, because
code from main runs there unattended as `lane`. Pin one commit of nifeos/fuzz-host for the whole
sequence:

```sh
FUZZ_HOST_COMMIT=a3f3d443de13cb27a01c8eff25016a91d4c1651b
```

That is main of nifeos/fuzz-host when this page was written. Read the diff from it before using a
later one.

### P4. Fetch and test the fuzz-host commit (both machines; root is not needed)

Fetch it into your own account, never the fuzz account, and review what you are about to run as
root. `script/test` makes its own throwaway keys and never touches GitHub.

```sh
git clone https://github.com/nifeos/fuzz-host.git ~/fuzz-host
git -C ~/fuzz-host checkout --detach "$FUZZ_HOST_COMMIT"
git -C ~/fuzz-host log --oneline -1
~/fuzz-host/script/test
```

Verify: `git log` prints `a3f3d44 Merge pull request #4 from nifeos/lane/fuzz-reader-token`, and
`script/test` ends with every selftest passing and exit status 0. It needs `python3` and `openssl`.
Run it as yourself, not as root.

### P5. Generate the `nife-fuzz` key and place it on the fuzz host (person only, root)

The key is the one credential that the whole path rests on. It stays in a file that root alone reads,
and it never enters a repository, a chat, a pull request, a log, a shell history or a lane's
environment.

1. At <https://github.com/organizations/nifeos/settings/apps/nife-fuzz>, open Credentials, Key pairs
   (older layouts say Private keys) and generate a private key. The browser downloads one
   `nife-fuzz.<date>.private-key.pem`. Note the SHA256 fingerprint GitHub shows beside the key.
2. At once, `chmod 600 ~/Downloads/nife-fuzz.<date>.private-key.pem`.
3. Copy it to your own account on the fuzz host. Then, in one command, make the directory only root
   can enter, install the key owned by root with mode 0400, and shred the copy:

```sh
scp ~/Downloads/nife-fuzz.<date>.private-key.pem <you>@<fuzz host>:nife-fuzz.pem
ssh -t <you>@<fuzz host> 'sudo install -d -o root -g root -m 0700 /etc/nife-fuzz \
  && sudo install -o root -g root -m 0400 ~/nife-fuzz.pem \
       /etc/nife-fuzz/nife-fuzz.private-key.pem \
  && shred -u ~/nife-fuzz.pem'
```

4. On your own machine, delete the download (`rm -P ~/Downloads/nife-fuzz.<date>.private-key.pem` on
   macOS, `shred -u` on Linux) and empty the trash.

Verify without showing the key. After each command, the text after the colon is its expected output:

- `sudo stat -c '%U %G %a' /etc/nife-fuzz /etc/nife-fuzz/nife-fuzz.private-key.pem`: `root root 700`,
  then `root root 400`.
- `sudo -u lane cat /etc/nife-fuzz/nife-fuzz.private-key.pem >/dev/null 2>&1 || echo refused`:
  `refused`.
- `sudo openssl rsa -in /etc/nife-fuzz/nife-fuzz.private-key.pem -pubout -outform DER 2>/dev/null |
  openssl sha256 -binary | openssl base64`: the fingerprint from step 1, without its `SHA256:` prefix.
- `ls ~/nife-fuzz.pem`: `No such file or directory`.

If the fingerprint differs, the file on the host is not the key you generated. Delete it on the host
and on GitHub, and start again at step 1.

### P6. Install the mint helper and its publish token (root on the fuzz host; may go to the agent)

```sh
sudo groupadd --system fuzz-token-publish
sudo usermod -a -G fuzz-token-publish lane
sudo install -o root -g root -m 0555 ~/fuzz-host/bin/fuzz-token /usr/local/sbin/fuzz-token
sudo install -o root -g root -m 0644 ~/fuzz-host/systemd/fuzz-token@.service \
    ~/fuzz-host/systemd/fuzz-token@.timer /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now fuzz-token@publish.timer
sudo systemctl start fuzz-token@publish.service
```

Verify:

- `getent group fuzz-token-publish`: the group with `lane` as its only member.
- `sudo stat -c '%U %G %a' /run/fuzz-token /run/fuzz-token/publish.env`: `root root 711`, then
  `root fuzz-token-publish 440`.
- `sudo /usr/local/sbin/fuzz-token check publish`: a `repositories: nifeos/nife` line, an `expires:`
  line about an hour ahead, and exit status 0. It never prints the token.
- `sudo /usr/local/sbin/fuzz-token check publish --fresh`: the same, with `permissions:` naming
  contents write and metadata read, and `revoked: yes`.
- `systemctl list-timers 'fuzz-token@*'`: the next run within 20 minutes.

If `check` lists any repository besides `nifeos/nife`, open
<https://github.com/organizations/nifeos/settings/installations/177641059> and set Only select
repositories to `nifeos/nife` alone. If a mint fails, `journalctl -u fuzz-token@publish.service -n 20`
names the refusal: the key's owner or mode, an HTTP status, a token wider than asked, or a group that
holds more than its reader.

### P7. Turn on the daily release (root on the fuzz host; may go to the agent)

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

The `Wants=` and `After=` lines mint a fresh token before each run. systemd reads `EnvironmentFile=`
as root before the service starts as `lane`. Save, then:

```sh
sudo systemctl start nife-fuzz-publish.service
```

Verify: `systemctl show -p Result nife-fuzz-publish.service` prints `Result=success`. A release
`fuzz-<today>` appears at <https://github.com/nifeos/nife/releases> with `report.md` and
`clean-ranges.tsv`, made by `nife-fuzz[bot]`. The watchdog stops passing quietly from that day. If the
start fails, `journalctl -u nife-fuzz-publish.service -n 30` names the refusal: a missing
`NIFE_FUZZ_PUBLISH`, a stale token or a `gh` error.

### P8. Turn on the filer (root on the fuzz host; may go to the agent)

The filer runs as `lane`, the account that also runs the fuzzed code. An architect ruled this on
2026-11-03 (UTC). The accepted cost: code that reaches main then runs as an account that can read the
filer token, which lets its holder publish any advisory on nifeos/nife, and a published advisory
cannot be unpublished. The protection is that main requires pull requests and checks. Tokens stay at
one hour, and the filer holds no long-lived credential.

The unit's identity lines, as `systemd/fuzz-filer.service` carries them:

```ini
[Service]
User=lane
Group=lane
Environment=NIFE_FUZZ_HOME=/home/lane/fuzz
ExecStart=/usr/local/bin/fuzz-filer run
```

The token side needs no unit edit. `fuzz-token@filer.service` runs as root and writes
`/run/fuzz-token/filer.env` for the group `fuzz-token-filer`, whose only member is `lane`.

```sh
sudo groupadd --system fuzz-token-filer
sudo usermod -a -G fuzz-token-filer lane
sudo install -o root -g root -m 0555 ~/fuzz-host/bin/fuzz-filer /usr/local/bin/fuzz-filer
sudo install -o root -g root -m 0644 ~/fuzz-host/systemd/fuzz-filer.service \
    ~/fuzz-host/systemd/fuzz-filer.timer /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now fuzz-token@filer.timer
sudo systemctl start fuzz-token@filer.service
sudo /usr/local/sbin/fuzz-token check filer
```

Verify `check filer`: `repositories: nifeos/nife`, `permissions:` naming advisories write and
metadata read, and an expiry about an hour ahead. Then look before filing. The dry run sends nothing:

```sh
sudo install -d -o lane -g lane -m 0700 /var/lib/fuzz-filer
sudo -u lane env NIFE_FUZZ_HOME=/home/lane/fuzz /usr/local/bin/fuzz-filer run --dry-run
```

Expected: one `would file finding <id>` line per held finding, or `none to file`. Then turn it on:

```sh
sudo systemctl enable --now fuzz-filer.timer
sudo systemctl start fuzz-filer.service
```

Verify:

- `journalctl -u fuzz-filer.service -n 20`: one `filed finding <id> (held <age>) as GHSA-...` line per
  held finding, with no seed, file name or advisory text.
- `sudo -u lane /usr/local/bin/fuzz-filer ledger`: one `filed` line per held finding.
- <https://github.com/nifeos/nife/security/advisories>: one draft per held finding, made by
  `nife-fuzz[bot]`.
- Start the service again: the journal says `none to file`.
- `systemctl list-timers fuzz-filer.timer`: the next run within 30 minutes.

If a run fails, the journal names the refusal: the token file (its owner, mode, group, profile or
age), the ledger, a finding by id, or an HTTP status. A 422 that names the range field means GitHub
refused `<= <commit>` as a range for the ecosystem `other`; the finding stays pending.

### P9. Install the reader helper (root on the fix-lane machine; may go to the agent)

P4 must be done on this machine. `/usr/local/sbin` and each directory above it must be owned by root
and writable by no one else (`ls -ld /usr /usr/local /usr/local/sbin`). On a macOS host where a
package manager made `/usr/local` its user's, install the helper where root owns the path, and use
that path in P11.

```sh
sudo install -d -o root -g 0 -m 0755 /usr/local/sbin
sudo install -o root -g 0 -m 0555 ~/fuzz-host/bin/fuzz-token /usr/local/sbin/fuzz-token
sudo install -d -o root -g 0 -m 0700 /etc/nife-fuzz-reader
```

Verify: `sudo /usr/local/sbin/fuzz-token profiles` prints three rows, `filer`, `publish` and `reader`,
and `sudo ls -ldn /etc/nife-fuzz-reader` shows `drwx------` and owner `0`.

### P10. Generate the `nife-fuzz-reader` key and place it on the fix-lane machine (person only, root)

1. At <https://github.com/organizations/nifeos/settings/apps/nife-fuzz-reader>, under Credentials, Key
   pairs, generate a private key. Note the SHA256 fingerprint. `chmod 600` the download at once.
2. Install it and remove the download:

```sh
sudo install -o root -g 0 -m 0400 ~/Downloads/nife-fuzz-reader.<date>.private-key.pem \
    /etc/nife-fuzz-reader/nife-fuzz-reader.private-key.pem
rm -P ~/Downloads/nife-fuzz-reader.<date>.private-key.pem   # macOS; on Linux: shred -u
```

3. Empty the trash if the browser or a file manager put a copy there.

Verify without showing the key:

- `sudo ls -ln /etc/nife-fuzz-reader/nife-fuzz-reader.private-key.pem`: `-r--------`, owner `0`.
- `sudo openssl pkey -in /etc/nife-fuzz-reader/nife-fuzz-reader.private-key.pem -pubout -outform DER |
  openssl sha256 -binary | openssl base64`: the fingerprint, without its `SHA256:` prefix.

Never put this key on the fuzz host or in a lane's environment. On macOS, `ls -lde` on the directory
and `sudo ls -le` on the key must show no ACL line, because the helper cannot see a macOS ACL.

### P11. Write the sudo rule for fix lanes (root on the fix-lane machine; may go to the agent)

An architect ruled on 2026-11-03 (UTC) that the rule names exactly `fuzz-token issue reader`. Name the
account that lanes run as in place of `<lane account>`, and give it no other sudo rule.

```sh
sudo visudo -f /etc/sudoers.d/fuzz-token-reader
```

```
# fuzz-token-reader: fix lanes may ask for one nife-fuzz-reader token, and nothing else.
Defaults!/usr/local/sbin/fuzz-token env_reset
Defaults!/usr/local/sbin/fuzz-token !log_output, !log_input
<lane account> ALL=(root) NOPASSWD: /usr/local/sbin/fuzz-token issue reader
```

The rule line matches those arguments exactly: no `mint`, no `check`, no flag. The second `Defaults`
line keeps classic sudo's input and output logging from recording the token. sudo-rs, the default on
recent Ubuntu, refuses that setting as unknown, so leave the line out there. `visudo` refuses a file
with a syntax error. Never widen the rule with a wildcard.

A lane then takes a token on demand:

```sh
NIFE_ADVISORY_READER_TOKEN="$(sudo -n /usr/local/sbin/fuzz-token issue reader)" script/read-advisory <id>
```

Verify. After each command, the text after the colon is its expected output:

- `sudo -l -U <lane account>`: the one `NOPASSWD: /usr/local/sbin/fuzz-token issue reader` line and no
  rule that grants anything more.
- `sudo /usr/local/sbin/fuzz-token check reader --fresh`, as yourself: `app: nife-fuzz-reader (App ID
  5571868, installation 177642435)`, `permissions: metadata:read,repository_advisories:read`,
  `repositories: nifeos/nife`, an `expires:` line about an hour ahead, `revoked: yes`, exit status 0.
- As the lane account, `sudo -n /usr/local/sbin/fuzz-token issue reader | wc -c`: a number near 41,
  and one `issued one reader token` line on standard error.
- As the lane account, `sudo -n /usr/local/sbin/fuzz-token issue reader` with nothing after it: a
  refusal, because standard output is a terminal.
- As the lane account, `cat /etc/nife-fuzz-reader/nife-fuzz-reader.private-key.pem`: permission
  denied.

Any lane on the machine can take a reader token, because every lane runs as one account, and the
token reads every advisory on nifeos/nife. An architect accepted that on 2026-11-03 (UTC). The
protections are the fix-lane skill, the replay-line check and sudo's log, and the first fix lane
starts a revisit of the account split. If sudo gives `issue` a terminal where the caller's output is
a pipe, every request is refused until the rule or the helper changes. If `check` lists another
repository, open <https://github.com/organizations/nifeos/settings/installations/177642435> and set
Only select repositories to `nifeos/nife` alone.

## What is not built

- **Publish after a clean rerun.** Nothing replays a filed finding's seed after its fix lands, sets
  `patched_versions` and publishes the advisory, and nothing moves its held log out of the held
  directory. Until it exists, a filed advisory stays a draft, and the clean-range table keeps
  withholding every range of a slot that held a finding. Milestone
  [1010](../../design/roadmap/1010-a-held-finding-turns-public-after-its-fix-reruns-clean.md) is its
  block. It reads the filer's ledger, which exists.
- **The host replays a red CI shard.** A red `fuzz-guests` job names a commit and a shard, and nobody
  replays it. The proposal is
  [the fuzz host replays a red CI shard](../../design/roadmap/proposals/the-fuzz-host-replays-a-red-ci-shard.md).
- **Held rows' seeds under the held directory.** `windows.tsv` carries a held window's first seed and
  seed count, and the fuzz account reads it. That is a change to nife's supervisor, recorded in the
  `BUGS` of [`continuous-host.md`](continuous-host.md#bugs).

## Risks

- A leaked `nife-fuzz` key mints tokens until someone revokes it. With Contents write a token can
  create, move or delete any `fuzz-*` tag, create, edit or delete any release, and create, rewrite or
  delete any branch except main. Main requires a pull request with no bypass actor. The main ruleset
  requires zero approving reviews, so a token that pushes to an open pull request's branch with
  auto-merge armed can reach main through the merge queue when its checks pass. A change to workflow
  files needs the Workflows permission, which the App lacks. With advisories write a token can
  publish any held finding early.
- Revoke: suspend the installation at once, at
  <https://github.com/organizations/nifeos/settings/installations/177641059> (Suspend), which stops
  every token. Then delete the key on the App's page, place a fresh one as in P5, and unsuspend. For
  the reader, suspend installation 177642435 and place a fresh key as in P10.
- The permissions are the least a single App can hold for the ruled path: a release needs Contents
  write, and filing and publishing need advisories write. A token can narrow further than its App, so
  the mint helper mints one token per service: Contents alone for the release, advisories alone for
  the filer. Both default to the reader `lane`, so the split narrows each token and isolates neither.

## Not yet verified

No one has run these against GitHub or a real host. Each is a check for the first install.

- GitHub accepting `<= <commit>` as a range for the ecosystem `other`.
- The replay command the filer writes into an advisory. The author took it from the supervisor's
  build steps, and no fix lane has run one.
- The seed count 1000000 the filer writes for a crash or a hang. The author copied it from the
  supervisor, which records it in no row.
- sudo-rs matching the rule's arguments as classic sudo does, and each sudo handing the helper a
  pipe, not a terminal, for standard output.
- The hardening lines of the fuzz-token and fuzz-filer units under systemd. `systemd-analyze verify`
  passes on them.
- A comment on a repository advisory, which the publish step wants for a failing replay. The REST API
  may have no endpoint for it (from memory, not checked).
