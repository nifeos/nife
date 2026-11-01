---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: 697, 976, 909
decision_dependencies: 208, 235
machine_requirements: none
specific_machine: none
needs_person: no
---
# The base programs carry their manifests in notes, and the progenitor endows them from the note

Raised 2026-11-01 (UTC) by lane `milestone/909-item-3-composition-checks-the-approval`, from the
ruling on #2981. Every name below is provisional.

Ruled 2026-11-01 (UTC), option B on #2981: for a program the image's archive carries, the
composer's approval check and the first installed-programs table read the program's own manifest
note. The base programs carry their real manifests in notes, and the progenitor endows an archive
program from its note as it endows an installed one. The compiled table in `grant_plan` retires a
few programs at a time, as each program's note takes over. The base is to become ordinary packages,
so that milestone 976 (an OS update is a set of packages that lands through a slot) can update it
with packages. This milestone starts after milestone 697 (the trust root leaves the kernel binary)
lands its code, which waits on milestone 976's item 4.

Reuse: `capability_manifest_note`'s `carry!` and its one encoding, `program_asks` as the one
reading, `endowed_image` as the one endowment path, `approval_file::File::check` in
`cargo xtask base-image`, and `installed_programs`' grant records. No new dependency.

## What the tree does today

A program the archive carries runs by name, and the progenitor endows it from
`grant_plan::Program::manifest`, compiled into the progenitor. An installed program runs by its
digest, and `endowed_image` endows it from the note in its own bytes. Of the programs
`packages/base-image.toml` lists, only `jig` carries a note, and that note asks for nothing; the
compiled table gives the image's `jig` the network, the installer endpoint and the catalog. So
milestone 909 (basalt composes the bootable image from its packages) item 3's approval check passes the base against any approval file (measured
2026-10-31 UTC on aarch64), and `packages/base-image.toml` names no approval file.

## The work

1. Each base program carries its manifest in a note, through `carry!`, taken from the row the
   compiled table holds today, so the note and the table agree while both exist.
2. The progenitor endows an archive program from its note, through the path an installed program
   takes. While a program has both a note and a table row, a gate holds the two equal.
3. The compiled rows retire a few programs at a time. Each step removes the rows whose notes the
   boot now reads, and the swish-check rows pass on all three architectures at every step.
4. `packages/base-image.toml` names the base image's approval file once a base program carries a
   note that asks, and milestone 990 (the base image's first installed-programs table) writes the
   grants that check approved.

## Done when

No base program takes its authority from a compiled table, the composer's check refuses a base
image whose approval file does not allow what the base programs' notes ask, and the first table's
grant rows equal those asks.

## The open question: what stops an ordinary package asking for a base-only authority

Not ruled. The maintainer brings it to an architect before step 1 reaches `jig`.

A manifest note today cannot ask for seven authorities.
`capability_manifest_note::encode` refuses `reboot`, `sync`, `installer`, `catalog`, the index
store, the activation directory and console input at compile time. The reason in the code is that
only the owner's console grants them, and only to a program the image names. That refusal is what
keeps a third-party package from asking for the installer endpoint. The base needs these asks once
its notes are real: `jig` holds the installer, the catalog, the index store and the activation
directory, `reboot` holds the reboot object, and the shell reads the console.

The compile-time refusal binds an honest builder only. Anyone can write a note's bytes by hand, so
the encoder alone stops nothing at install. The approval file grammar already spells
`installer = true` for a package entry (milestone 911 (scripted installs grant only what the
owner's approval file allows)), which a note cannot ask for today.

The question: when the base's notes ask for these seven, what refuses the same asks from an
ordinary package?

- **Recommended, A: the install path.** The progenitor's install refuses any of the seven asks
  from a package that the image's signed base set does not vouch for, whatever the approval file
  or the console answers. The encoder keeps a second constant set for third-party builds, so an
  honest builder still learns at compile time. One check, at the one place every package passes.
- B: the format. A separate note type that only the base's build emits. A format cannot tell who
  wrote the bytes, so this still needs A's check behind it.
- C: the updater. Milestone 976's updater carries base packages only, and only its path may
  deliver the seven. `jig install` takes another path, so this also needs A's check there.
- D: the owner. The approval file and the console may grant any of the seven to any package. This
  is the most general and the easiest to misuse: one careless approval hands out the installer.

Prior art, from memory and not re-read: Android's `signature|privileged` permissions go only to
apps signed with the platform key or installed in the system image's privileged directory, and the
package manager enforces it at install, whatever the app declares. Fuchsia separates base packages,
resolved from the system image, from universe packages, and routes several system capabilities
only to components the base names.

A concrete example. On a system where `jig`'s note asks for the installer, a package `helper`
arrives whose note asks for the installer too, and the owner's approval file says
`installer = true` for it. Under A, `jig install helper` refuses with the installer named as an ask
outside the base. Under D it installs, and `helper` can now install anything the owner's catalog
vouches for.

## Index row

The base programs carry their manifests in notes and the progenitor endows them from the note, so
the base installs, updates and is checked the way every package is.
