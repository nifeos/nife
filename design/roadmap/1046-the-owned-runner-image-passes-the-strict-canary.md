---
status: PARTIAL
promoted_from: the-owned-runner-image-passes-the-strict-canary
raised: 2026-11-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1046. The owned runner image passes the strict canary

Raised by lane/record-follow-on-items, which gathered the follow-on work of the owned-runner lanes.
The work is on the owned hosts, which the maintainers
who operate the pool change off-tree; nothing in this repository changes until the last item.

Reuse: the canary's strict image checks and dry run (`.github/workflows/owned-runner-canary.yml`,
`helpers/owned-image-probe.sh`) are the acceptance test, unchanged. No new tool.

## Background

[notes/owned-runners.md](../../notes/owned-runners.md) keeps `NIFE_RUNNER_OWNED` false until the
strict canary and the dry run are both green on the pool in one run, with no `FINDING` line. The
proxy may refuse nothing in that run but the runner's own background requests to two named hosts.
Each item below says whether public run data shows it met, and what an open item still needs.

The evidence run is the push run 42934754693 of `.github/workflows/owned-runner-canary.yml`. Its
`canary-owned` and `canary-owned-dry-run` jobs are green, with no `FINDING` line, and the other
green canary runs on the pool show the same probe output. A canary run that fails now shows a
runner that lost contact with GitHub or a probe that a host outage cut short, not an image finding.

## The work

1. The runner's work folder is `/home/runner/work`, the hosted layout. Met: the dry run prints
   `workspace: /home/runner/work/nife/nife`, and its QEMU cache restores there.
2. apt carries an `Acquire::http::Proxy` and `Acquire::https::Proxy` setting for the pool's proxy,
   and `sudo` keeps the proxy variables (`Defaults env_keep`). Met: the `apt-proxy` probe prints
   an apt proxy setting and `sudo keeps http_proxy`, and the dry run's `apt-get update` succeeds
   through the proxy. The probe prints only the first setting, so the run does not show the
   `https` setting by itself.
3. The proxy's allowlist holds the two hosts pip fetches from, `pypi.org` and
   `files.pythonhosted.org`. Met: the `pip` probe downloads `wheel` and `qemu.qmp` through the
   proxy.
4. The image carries a Rust toolchain manager under a system path such as `/opt/rust`, put on
   `PATH` for every job through the runner's `.env` file, so a workflow edit cannot remove it. Met:
   the `tools` probe finds `rustup` and `cargo` in `/opt/rust/cargo/bin`, and no workflow, helper
   or script in the tree puts that directory on `PATH`.
5. The image preinstalls the packages `script/ci-qemu` and `script/bootstrap` install, so a job
   spends no time on them and fetches nothing the proxy must allow for them. Open: the `packages`
   probe checks each package in `helpers/apt-packages.txt`, the list both scripts install from, with
   `dpkg-query`. The next green canary run on the pool shows whether the item is met; a missing
   package is a `FINDING` line.
6. Once one merge group or push run meets the condition in notes/owned-runners.md, a maintainer
   sets the variable as that note says. Open: it waits for the proxy evidence and for the
   architect, who keeps the variable false until the canary is green.

## Done when

- A canary run on the pool shows both owned probe jobs green, with no `FINDING` line, and the proxy
  refusal log shows no refusal for that run except the runner's own background requests to the two
  hosts notes/owned-runners.md names. Run 42934754693 meets the canary half. The proxy half stays
  open.
- notes/owned-runners.md's description of the image states the work folder, the toolchain and the
  preinstalled packages. The note states the work folder and the toolchain. The packages stay open
  with item 5.

## Follow-on

- **Outstanding.** Three pieces remain, checked 2026-11-11 against run 42934754693 and the
  canary's job history. The proxy's refusal log for a named green run: the maintainers who operate
  the pool keep it off-tree, so one of them reads it and states the result on this block's pull
  request. The preinstalled packages of item 5: the `packages` probe exists, and the next green
  canary run on the pool shows whether the image has them all. Then item 6, the switch, which the architect
  holds.

## Index row

The owned runner image passes the strict canary and the dry run; the switch stays off. Open: the proxy refusal evidence for a green run, a green canary run that shows the preinstalled packages, then the switch.
