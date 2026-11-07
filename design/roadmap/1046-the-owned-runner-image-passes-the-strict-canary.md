---
status: NOT-STARTED
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
proxy may refuse nothing in that run but the runner's own background requests to two named hosts. The push run 41959340528 on `main` printed these findings in
`canary-owned` and `canary-owned-dry-run`:

- `rustup` and `cargo` are not on `PATH`, so the probe cannot check the pinned toolchain.
- The workspace is `/home/runner/actions-runner/_work/nife/nife`, not
  `/home/runner/work/nife/nife`, so a QEMU cache saved on a hosted runner restores outside `$HOME`
  and the cache step reports a hit with no QEMU behind it.
- apt has no proxy configured, and `sudo` drops `http_proxy`, so a tool run under `sudo` bypasses
  the proxy.
- pip cannot reach its index: the proxy answers 403 for the package index hosts.

## The work

1. The runner's work folder is `/home/runner/work`, the hosted layout.
2. apt carries an `Acquire::http::Proxy` and `Acquire::https::Proxy` setting for the pool's proxy,
   and `sudo` keeps the proxy variables (`Defaults env_keep`).
3. The proxy's allowlist holds the two hosts pip fetches from, `pypi.org` and
   `files.pythonhosted.org`.
4. The image carries a Rust toolchain manager under a system path such as `/opt/rust`, put on
   `PATH` for every job through the runner's `.env` file, so a workflow edit cannot remove it.
5. The image preinstalls the packages `script/ci-qemu` and `script/bootstrap` install, so a job
   spends no time on them and fetches nothing the proxy must allow for them.
6. Once one merge group or push run meets the condition in notes/owned-runners.md, a maintainer
   sets the variable as that note says.

## Done when

- A canary run on the pool shows both owned probe jobs green, with no `FINDING` line, and the proxy
  refusal log shows no refusal for that run except the runner's own background requests to the two
  hosts notes/owned-runners.md names.
- notes/owned-runners.md's description of the image states the work folder, the toolchain and the
  preinstalled packages.

## Index row

The owned runner image fails the strict canary, so the owned-runner switch stays off. Proposed: fix the work folder, the apt and sudo proxy, the pip allowlist and the toolchain path, then set the switch.
