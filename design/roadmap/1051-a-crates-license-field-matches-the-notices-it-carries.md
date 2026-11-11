---
status: BUILT
built: 2026-11-10
promoted_from: a-crates-license-field-matches-the-notices-it-carries
raised: 2026-11-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1051. A crate's license field matches the notices it carries

Raised by lane/manifest-license-names-adapted-sources.

Reuse: `license_is_accepted` and `NOTICE_LICENSES` in `helpers/crate_release.py`, which
`script/crate-standalone` already runs. No new tool.

## Background

A crate that adapts permissively licensed source joins `MIT OR Apache-2.0` with `AND` to the SPDX
identifier of each adapted source's license, and each adapted file keeps its notice. Milestone 887
(crate repositories share one CI template) states the rule. `helpers/crate_release.py` checks only
the field's shape: `(MIT OR Apache-2.0) AND` followed by licenses from `NOTICE_LICENSES`. A field
that names a license no file carries passes, and so does one that misses a license a file carries.
Nobody can withdraw a published manifest, so that mistake outlives the fix.

The notices do not yet say their license in a form a program can read. `e1000e`'s notices open
with an `SPDX-License-Identifier:` line. The ISC and BSD-2-Clause notices in `designware_ethernet`
and the ISC notice in `designware_mobile_storage` open with prose ("The ISC license requires this
notice").

## The work

1. Every notice of adapted source that a crate carries opens with an `SPDX-License-Identifier:`
   line naming that source's license, inside the notice's text block. Add the line to the notices
   that lack it.
2. `helpers/crate_release.py` collects those lines from the crate's shipped files and compares the
   set with the `AND` terms of the manifest's `license` field, both ways. A term with no notice and
   a notice with no term each fail, naming the file or the term.
3. Run it over every crate that carries a notice, and list the result in the pull request.

## Done when

- `script/crate-standalone` fails a crate whose `license` field and carried notices disagree, and a
  test shows both directions failing.
- `e1000e`, `designware_ethernet` and `designware_mobile_storage` pass it.

## Follow-on

- **None.**

## Index row

A crate's `license` field can drift from the notices its repository carries, and nothing checks it. This adds a check to the standalone-crate script that fails either direction of disagreement.
