---
status: NOT-STARTED
promoted_from: the-system-manifest-check-refuses-a-stem-held-twice
raised: 2026-11-03
milestone_dependencies: 976
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1032. The system manifest check refuses a stem held twice

Raised 2026-11-03 (UTC) by the security audit
[2026-11-03-from-the-package-to-the-grant.md](../audit-reports/2026-11-03-from-the-package-to-the-grant.md),
finding 2. Severity: low. Every name below is provisional.

Reuse: `crates/system_manifest` as it is; one loop in `check`.

## The failure

`system_manifest::SystemManifest::check` documents "every listed package present at its listed digest,
and nothing else". It looks each listed stem up in `held` with `find`, which takes the first match,
and then refuses a held stem only if the manifest lists no digest for it. A `held` list that names
one stem twice, first at the listed digest and then at another, passes.

Unreachable today: the only caller, `xtask/src/base_image.rs`, passes what `render` produced, which
is already one entry per stem. Milestone 976 (an OS update through a slot) names the device updater
as a later consumer. An updater that builds `held` from the files a slot holds would accept a slot
with a second, unlisted copy of a listed package.

## The work

`check` refuses a stem that `held` names more than once, with its own `Refusal` variant naming the
stem, and a host test holds it. This lands before the updater reads `check`, and can fold into the
part of milestone 976 that adds that reader.

## Index row

The system manifest check accepts a held list that names one stem twice, first at the listed digest and then at another. Closing it before the device updater arrives keeps the rule that the slot holds nothing else.
