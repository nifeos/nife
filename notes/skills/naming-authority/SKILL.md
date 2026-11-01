---
name: naming-authority
description: >-
  Who names things in nife: load before minting, choosing or changing the name of a crate, a
  program, a module, a script or a public function, or before ratifying or performing a rename.
  Names are an architect's call: ship a provisional name, say so in the report, and never rename on
  your own initiative. Points to design/naming.md for the conventions.
---

# Naming authority

Where this says "this file", it means `AGENTS.md`. Its name was ratified by calef on 2026-10-06
(UTC); notes/skills/README.md records it.

## An architect names the crates, the programs, and the shared modules

The name of a crate, a program, a module, or a public function is an architect's call, not a lane's
and not yours (2026-08-01, widened to functions 2026-08-23). It is global to the tree, so it is
decided by an architect, who can see the whole tree. The reason is calef's: names are what make this
OS accessible to humans and to LLMs, and in a capability system the name is often the only thing
that says what a program may *do*.

So: propose, ship a provisional name, say so in your report, and never rename on your own initiative
(a rename is a naming decision with extra steps). The block reads `Name: provisional.` and what was
considered. Who minted it and when are in `git blame` (§298 (a document states what is in force now)). That mechanism is what makes it safe not to have
read the conventions before you start: a provisional name is expected to change, and the maintainer
surfaces it.

[design/naming.md](../../../design/naming.md) is the rule, §155 (the naming conventions move out of the
constitution). It holds the spelling conventions per domain, the acronym test, nouns over verbs and
the failure modes. It also holds what `script/lint` can and cannot check, how to perform a ratified
rename, and the refusals that shaped all of it. Read it before you ratify or rename; a lane
inventing a provisional name does not have to. Where it and this file disagree, that file is the
rule for naming conventions and this one is the bug; this file keeps only the authority above.

Where a permissively licensed source names a thing, copy its name: OpenBSD first, then FreeBSD, and Linux's only when no BSD source names it (new and provisional names; [the ruling](https://github.com/nifeos/nife/pull/3005#issuecomment-6559086117), 2026-11-01 UTC).

A name you mint or rename must pass the mechanical naming checks, which `script/lint` runs as
"names meet the current naming rules" (`helpers/name_checks.py`). Renaming an unratified name to
comply leaves it provisional; only calef ratifies.

Contributors are referred to by their GitHub username in prose, attributions, records and lane
reports. Legal names appear only in legal and authorship strings (`Cargo.toml` authors, licenses,
patch `From:` headers), so a grep for a contributor finds them rather than everyone sharing a first
name.
