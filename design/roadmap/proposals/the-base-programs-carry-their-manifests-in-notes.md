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
4. The progenitor's install refuses any of the seven base-only asks from a package the base does
   not vouch for, whatever the approval file or the console answers, and the encoder keeps a
   second, shorter constant set for third-party builds. This lands before step 1 reaches `jig`.
   It needs the plumbing named under the rule, which hands the install path the base's list.
5. `packages/base-image.toml` names the base image's approval file once a base program carries a
   note that asks, and milestone 990 (the base image's first installed-programs table) writes the
   grants that check approved.

## Done when

No base program takes its authority from a compiled table, the composer's check refuses a base
image whose approval file does not allow what the base programs' notes ask, and the first table's
grant rows equal those asks. No ordinary package can carry any of the seven asks past the install
check.

## The rule: the install path refuses base-only asks from a package the base does not vouch for

Ruled 2026-11-01 (UTC), option A. The progenitor's install refuses seven asks from a package the
image's base does not vouch for: `reboot`, `sync`, `installer`, `catalog`, the index store, the
activation directory and console input. Neither the approval file nor the console changes that. The
encoder keeps a second, shorter constant set for third-party builds, so an honest builder still
learns at compile time. One check sits at the one place every package passes. A separate note type,
the updater and the owner's approval did not win.

Why it matters. `capability_manifest_note::encode` refuses the seven at compile time today. The
base needs them once its notes are real: `jig` holds the installer, the catalog, the index store
and the activation directory, `reboot` holds the reboot object, and the shell reads the console.
The compile-time refusal binds an honest builder only, since anyone can write a note's bytes by
hand. The approval file grammar already spells `installer = true` for a package entry (milestone
911 (scripted installs grant only what the owner's approval file allows)). The check at install
holds against a hand-written note.

### What "the base vouches for" means in code, measured 2026-11-01 UTC

- The install runs in the progenitor, in `install` in `crates/system_initializer/src/activation.rs`,
  after `approval_gate`. It reads the staged package, the image's catalog
  (`package_archive::CATALOG`, the `package_catalog` archive entry), the installed-programs table,
  the approval document and the answer's trailer. Its one test of base membership is
  `Program::from_name`, the compiled name list of `grant_plan`. That test refuses a package program
  whose bare name a base program holds.
- The progenitor holds the `program_measurements` table at boot. `open_archive` in `boot_steps.rs`
  reads it from the archive, and `measure_spawnable` measures each compiled program against it. The
  boot does not pass it on. The `Activating` and `EditContext` structs the install reads carry the
  catalog, the manager, the approval and the installed-programs table, and no measurements. The
  install path cannot see the table today.
- The kernel hashes the table and checks it against its trust root (`require_program_measurements`
  in `kernel/src/trust.rs`). The proposal `how-the-loader-hands-the-kernel-two-digests.md` hands
  its digest over as `nife.base`. The tree holds no code for that handoff.
- No list of base packages exists. The catalog lists the packages the image vouches for to install.
  This measurement did not determine whether the catalog also holds the base's own packages. The
  measurements table lists programs by archive name and digest, not packages.

The step needs one small piece of plumbing. The progenitor passes the measured
`program_measurements` text (or the set of base program digests parsed from it) from `boot` to
`Activating`, and `install` tests each program of the staged package against it. The base
vouches for a program by its digest in that table, not by package stem, because the loader's
`nife.base` names the table and no base package list exists in the tree (ruled 2026-11-01 UTC).

Prior art, from memory and not re-read: Android's `signature|privileged` permissions go only to
apps signed with the platform key or installed in the system image's privileged directory, and the
package manager enforces it at install, whatever the app declares. Fuchsia separates base packages,
resolved from the system image, from universe packages, and routes several system capabilities
only to components the base names.

A concrete example. On a system where `jig`'s note asks for the installer, a package `helper`
arrives whose note asks for the installer too, and the owner's approval file says
`installer = true` for it. `jig install helper` refuses with the installer named as an ask outside
the base, and the approval file does not change that.

## Index row

The base programs carry their manifests in notes and the progenitor endows them from the note, so
the base installs, updates and is checked the way every package is.
