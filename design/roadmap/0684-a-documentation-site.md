---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: a-documentation-site
milestone_dependencies: 611
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 684. A documentation site, and documentation that ships in packages

*Title and slug are drafts.*
Raised by the lane of milestone 611 (every program and crate belongs to a package) while giving every
tracked path a home. It builds nothing. It is the tracked home for an idea of calef's, so that the
undecided homes it leaves behind point somewhere.

## calef's words

2026-09-27 (UTC), relayed by the maintainer: "I'm thinking of building a website for much of our
documentation. It may also ship in packages. I don't know exactly what that division looks like
yet. But I'd like a presentation of the documentation beyond what github provides eventually."

## The open question

- Which documents go to a site, which ship in packages as on-device documentation, and which stay
  project records (`design/`, `briefs/`).
- Whether they are single-sourced. The likely shape is one source file, rendered to a site and also
  shipped in a package, which keeps one home per file. A format that assumes one destination per
  document is the thing to avoid.
- Where the site's own source and build live, which is a home like any other.

Until this is answered, every file under `notes/` has an undecided home in `packages/homes.toml`, and
the weekly metrics page counts them. That count is this proposal's measure of progress.

## Prior art to read, not yet read

Named from memory, to be read before anything is designed:

- FreeBSD: the Handbook on the web and `man` pages in the base system, from separate sources.
- rustup: `rustup doc` opens the book and the standard library documentation, offline, from files
  the toolchain package installs.
- Debian: `*-doc` packages, split from the program they describe.
- mdBook and Sphinx, which render one source tree to a site.
- This tree's own `mdr` and `crates/documentation`, from milestone 40 (documentation as a system
  service), which already carry pages to the device.

## Index row

calef wants a website presenting much of the documentation, which may also ship in packages. Proposed: decide which documents go to a site, which ship on-device and which stay project records, and whether they are single-sourced.
