# Finishing the continuous fuzz setup: what runs, what is left, and who does each step

*Name: provisional, minted 2026-11-02 (UTC) by maintainer/fuzz-finish-setup-runbook. No architect
has ruled on it.*

An appendix to [`continuous-host.md`](continuous-host.md). The continuous fuzzer ends at reporting.
An architect ruled on 2026-11-04 (UTC), narrowing the findings path of
[`fuzz-findings-without-a-person`](../../design/roadmap/1015-fuzz-findings-without-a-person.md):
after a person places one credential once, the host files one private advisory per held finding and
stops. A person works the advisories with the regular lane workflow on a separate fix-lane machine,
and publishes each one. The host never replays a fix and never publishes. This page lists what exists, what is not built, and the order of the steps left
to a person. Each step has commands to paste and the output to expect.

No step needs an organization secret or variable.

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
| Clean-range fix | built, on the host; a slot that held a finding publishes no more ranges, because nothing signals that a finding turned public | `helpers/fuzz-daily-publish.sh`, [milestone](../../design/roadmap/1016-the-clean-range-table-does-not-locate-a-held-seed.md) |
| Mint helper | built and merged in nifeos/fuzz-host (pull requests 2 and 3), not installed | `fuzz-token` and [docs/fuzz-token.md](https://github.com/nifeos/fuzz-host/blob/main/docs/fuzz-token.md), [milestone](../../design/roadmap/1022-the-fuzz-host-mints-its-tokens-from-the-app-key.md) |
| Advisory permission | verified: an App holding only Repository security advisories write, installed on one repository, creates, completes, publishes and publicly reads an advisory | [below](#the-advisory-permission) |
| Finding filer | built and merged in nifeos/fuzz-host (pull request 5), not installed; it runs as `lane`; unverified against GitHub and systemd | `fuzz-filer` and [docs/fuzz-filer.md](https://github.com/nifeos/fuzz-host/blob/main/docs/fuzz-filer.md), [milestone](../../design/roadmap/1020-the-fuzz-host-files-held-findings-as-advisories.md) |
| Fix lanes read the advisory: script, skill rule text, replay-line lint | built and merged | `script/read-advisory`, `helpers/replay-line-check.sh`, the developer-lane skill, [milestone](../../design/roadmap/1014-fix-lanes-read-the-advisory.md) |
| Reader token helper | built and merged in nifeos/fuzz-host (pull request 4) as the `reader` profile of `fuzz-token`, with `issue reader` for the sudo rule; not installed; unverified under a real sudo rule | [docs/fuzz-token-reader.md](https://github.com/nifeos/fuzz-host/blob/main/docs/fuzz-token-reader.md), [milestone](../../design/roadmap/1013-fix-lanes-get-a-one-hour-reader-token.md) |
| Host replays a red CI shard | not built | [proposal](../../design/roadmap/proposals/the-fuzz-host-replays-a-red-ci-shard.md) |
| `nife-fuzz` App | created, installed, no key placed | App ID 5571801, installation 177641059 |
| `nife-fuzz-reader` App | created, installed, no key placed | App ID 5571868, installation 177642435 |

The IDs are not secret. The installations hold exactly the ruled permissions: `nife-fuzz` holds
Contents write, Repository security advisories write and Metadata read; `nife-fuzz-reader` holds
Repository security advisories read and Metadata read.

## P1 to P3: done on the host

Repeat them after a rebuild of the host.

### P1. Bring the host to main (person, root)

The follower ([`continuous-host.md`](continuous-host.md#following-main)) cannot move a host that
predates it, so this step moves the host by hand once. From then on the supervisor follows main by
itself. Installing a changed unit stays this step's `diff` and copy.

```sh
sudo systemctl stop nife-fuzz.service
sudo -u lane git -C /home/lane/nife fetch origin main
sudo -u lane git -C /home/lane/nife checkout --detach origin/main
for n in 0 1 2 3; do sudo -u lane git -C /home/lane/fuzz/work/$n checkout --detach origin/main; done
sudo diff /etc/systemd/system/nife-fuzz.service /home/lane/nife/helpers/systemd/nife-fuzz.service \
  && echo "unit unchanged"
sudo systemctl start nife-fuzz.service
```

If the diff prints a difference, copy the new unit to `/etc/systemd/system/` and run `sudo systemctl
reenable nife-fuzz.service` before the start. Verify: `systemctl is-active nife-fuzz.service` prints
`active`; within about 5 minutes `journalctl -u nife-fuzz.service -g fuzz-follow-main -n 5` shows a
look; and after the next window `tail -1 /home/lane/fuzz/state/windows.tsv | cut -f7,8` shows the
new short commit.

### P2. Install the local daily report (person, root)

This writes `report.md` and `clean-ranges.tsv` under `/home/lane/fuzz/out/` once a day and publishes
nothing, so it is safe before the clean-range fix.

```sh
sudo cp /home/lane/nife/helpers/systemd/nife-fuzz-publish.service \
        /home/lane/nife/helpers/systemd/nife-fuzz-publish.timer /etc/systemd/system/
sudo systemctl enable --now nife-fuzz-publish.timer
```

Verify: `systemctl list-timers nife-fuzz-publish.timer` shows the next run at 14:00 UTC, and `sudo
systemctl start nife-fuzz-publish.service` followed by `ls /home/lane/fuzz/out/` shows today's date.

## Checking that it works

`script/fuzz-status` prints a PASS, FAIL or UNKNOWN line per check and exits 1 on any FAIL. It writes
nothing and prints counts, never a finding. Anywhere with `gh`, it reads the heartbeat runs, the
releases, the issue and advisory counts and main's tip. On the host, add `--host`, as a user in the
`systemd-journal` or `adm` group (no sudo); `--wait 300` reads the clean-window total twice.

## The advisory permission

Step P3 is this check, and it passed. A throwaway App holding only Repository security advisories
write, installed on one repository, created a draft advisory, added a version range, published it,
and the public read it without a credential (HTTP 201, 200, 200, 200). GitHub documents the caller
as "a security manager or administrator", and the permission alone qualifies.

A draft needs a vulnerability entry with a package and a `vulnerable_version_range`, or GitHub
answers 422, "Advisory is not publishable". So the filer files both. It leaves `patched_versions`
unset, because no fix exists at filing; the person who publishes the advisory sets it.

## The sequence for a person: P4 to P11

Every piece below is built and merged, and none is installed. P5 and P10 place a key, and only a
person does them. Every other step may go to the agent that maintains the host as a written spec:
hand it the step and have it report the output. It never holds a key.

Two machines take part. The **fuzz host** holds only the `nife-fuzz` key (P5) and gets the publish
and filer helpers (P6 to P8). The **fix-lane machine** is a separate machine where a person works the
advisories with the regular lane workflow. It gets the `nife-fuzz-reader` key (P10) and the reader
rule (P11), and nothing of P5 to P8. The reader key never goes to the fuzz host, because code from
main runs there unattended as `lane`. The fuzz host needs no reader step. Pin one commit of nifeos/fuzz-host for the whole
sequence:

```sh
FUZZ_HOST_COMMIT=a3f3d443de13cb27a01c8eff25016a91d4c1651b
```

Read the diff before using a later commit.

### P4. Fetch and test the fuzz-host commit (both machines; root is not needed)

Fetch it into your own account, never the fuzz account, and review what you will run as root.

```sh
git clone https://github.com/nifeos/fuzz-host.git ~/fuzz-host
git -C ~/fuzz-host checkout --detach "$FUZZ_HOST_COMMIT"
git -C ~/fuzz-host log --oneline -1
~/fuzz-host/script/test
```

Verify: `git log` prints `a3f3d44 Merge pull request #4 from nifeos/lane/fuzz-reader-token`, and
`script/test` (which needs `python3` and `openssl`, makes its own throwaway keys and never touches
GitHub) ends with every selftest passing. Run it as yourself.

### P5. Generate the `nife-fuzz` key and place it on the fuzz host (person only, root)

The key never enters a repository, a chat, a pull request, a log, a shell history or a lane's
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
sudo systemctl enable --now fuzz-token@publish.timer
sudo systemctl start fuzz-token@publish.service
```

`enable` rereads the unit files itself. Verify:

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
repositories to `nifeos/nife` alone.

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

The `Wants=` and `After=` lines mint a fresh token before each run. Save, then:

```sh
sudo systemctl start nife-fuzz-publish.service
```

Verify: `systemctl show -p Result nife-fuzz-publish.service` prints `Result=success`. A release
`fuzz-<today>` appears at <https://github.com/nifeos/nife/releases> with `report.md` and
`clean-ranges.tsv`, made by `nife-fuzz[bot]`. The watchdog stops passing quietly from that day.

### P8. Turn on the filer (root on the fuzz host; may go to the agent)

The filer runs as `lane`, the account that also runs the fuzzed code. An architect ruled this on
2026-11-03 (UTC). Code that reaches main then runs as an account that can read the filer token, and a
published advisory cannot be unpublished. Main's pull requests and checks are the protection. Tokens
stay at one hour.

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

If a run fails, the journal names the refusal.

### P9. Install the reader helper (root on the fix-lane machine; may go to the agent)

P4 must be done on this machine. `/usr/local/sbin` and each directory above it must belong to root and
admit no other writer (`ls -ld /usr /usr/local /usr/local/sbin`). Where a package manager made
`/usr/local` its user's, install the helper where root owns the path, and use that path in P11.

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

On macOS, `ls -lde` on the directory and `sudo ls -le` on the key must show no ACL line, because the
helper cannot see a macOS ACL.

### P11. Write the sudo rule for fix lanes (root on the fix-lane machine; may go to the agent)

An architect ruled on 2026-11-03 (UTC) that the rule names exactly `fuzz-token issue reader`. Name the
account that runs lanes on the fix-lane machine in place of `<lane account>`; the person fills it in.
Give that account no other sudo rule.

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
line keeps classic sudo's logging from recording the token. sudo-rs, the default on recent Ubuntu,
refuses that setting, so leave the line out there. Never widen the rule with a wildcard.

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
fix-lane skill, the replay-line check and sudo's log protect it, and the first fix lane starts a
revisit of the account split. If `check` lists another repository, open
<https://github.com/organizations/nifeos/settings/installations/177642435> and set Only select
repositories to `nifeos/nife` alone.

## What is not built

- **Nothing after filing, by ruling.** The host does not replay a fix, publish an advisory or move a
  held log. A person publishes each advisory after its fix. The clean-range table therefore keeps
  withholding every range of a slot that held a finding, and the held log stays on the host.
- **The host replays a red CI shard.** A red `fuzz-guests` job names a commit and a shard, and nobody
  replays it. The proposal is
  [the fuzz host replays a red CI shard](../../design/roadmap/proposals/the-fuzz-host-replays-a-red-ci-shard.md).
- **Held rows' seeds under the held directory.** A change to nife's supervisor, recorded in the
  `BUGS` of [`continuous-host.md`](continuous-host.md#bugs).

## Risks

The `nife-fuzz` App holds Repository security advisories write, and that permission can also publish
an advisory, a held finding's included. The filer never does: it calls GET to list advisories and POST
to create a draft, sends no state in the body, and has no PATCH or other call. The permission is
still there for anything that runs as `lane` with a filer token, which is the accepted risk of P8.
[`the-fuzz-app.md`](the-fuzz-app.md) states what a leaked key can do. To revoke, suspend the
installation at once, at <https://github.com/organizations/nifeos/settings/installations/177641059>
(Suspend), which stops every token. Delete the key on the App's page, place a fresh one as in P5, and
unsuspend. For the reader, suspend installation 177642435 and place a fresh key as in P10. Both fuzz-host
profiles default to the reader `lane`, so the split isolates nothing between them.

## Not yet verified

No one has run these against GitHub or a real host. Each is a check for the first install.

- GitHub accepting `<= <commit>` as a range for the ecosystem `other`.
- The replay command and the seed count 1000000 that the filer writes. The author took both from the
  supervisor's build steps, and no fix lane has run one.
- sudo-rs matching the rule's arguments as classic sudo does, and sudo handing the helper a pipe.
- The hardening lines of the fuzz-token and fuzz-filer units.
