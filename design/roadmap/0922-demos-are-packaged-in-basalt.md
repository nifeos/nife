---
status: PARTIAL
raised: 2026-10-14
milestone_dependencies: 611, 809, 810, 921
decision_dependencies: 297, 195, 197, 250
machine_requirements: none
specific_machine: none
needs_person: no
---
# 922. The demos are packaged in basalt

**PARTIAL, 2026-10-24 (UTC).** basalt builds and publishes `demos`, and `jig` fetches it and
accepts its digest on all three architectures. The installer then refuses it, because every image
still carries `least_authority_demo`. Left: running an installed demo, which waits on milestone 909
alone. The shell opens a program by a path whose last component is `least_authority_demo`'s 20
bytes (milestone 946), so the name no longer stands in the way ("BUGS" below).

*(Minted 2026-10-14 (UTC) by lane `milestone/922-demos-are-packaged-in-basalt`, under §297
(software reaches a nife system as a package `jig` installs from basalt's index, the kernel
included). The number is provisional until the merge queue lands it; the title and slug are
drafts.)*

calef approved on 2026-10-13 (UTC) a packaging order for first-party programs after the
benchmarks: rmle and the demos as the first first-party optional packages, then `core-tools`.
`packages/demos.package.toml` makes `demos` `optional` and gives it one program,
`least_authority_demo`. This follows milestone 921 (rmle is packaged in basalt, and `jig`
installs it).

## The work

1. A basalt recipe, `recipes/demos.toml`, on milestone 921's first-party shape: the `[[bin]]`
   `least_authority_demo` of `components`, built at the pinned nife commit for each bare-metal
   target, `MIT OR Apache-2.0`, kind `optional`.
2. `script/swish-check` types `jig install demos`, and once it installs, runs the installed
   program by its package path with an argument, which only a vouched package's note can be
   granted.
3. nife's tests keep the image's own `least_authority_demo`, built from source.

## Built 2026-10-14 (UTC)

Lane `milestone/922-demos-are-packaged-in-basalt`, with nifeos/basalt#25.

- The recipe packs `bin/least_authority_demo`. The release build keeps its manifest note, a
  76-byte `PT_NOTE` on aarch64 (item 1). The first published Release is
  `rolling-2026-10-14-4d05b6c`, and `xtask/basalt-pin` names a basalt commit whose recipe records its digests.
- `swish-check` types `jig install demos`. It must fetch the package from basalt's index and
  then answer the installer's refusal: *"the image carries the program least_authority_demo; a
  new base updates it, not install"* (item 2, half). Found 2026-10-14 (UTC) by a local boot: the
  run line this block planned, `packages/demos/0.1.0/least_authority_demo 9`, met that refusal and
  then a second one, below.
- The image's own copy and its prompt lines are unchanged (item 3).

## Exit

On aarch64, riscv64 and x86_64: a nife at basalt's pinned commit runs `jig install demos` against
basalt's index, and the installed `least_authority_demo` squares the integer its line names.

Reuse: milestone 921's first-party build, and milestone 810 (`ripgrep` is packaged in basalt and
installed with `jig`)'s pins and publishing.

## BUGS

- Every image still packs `least_authority_demo`, because every image packs every `components`
  `[[bin]]`, so the installer refuses `demos`. An `optional` package leaves the shipped image when
  milestone 909 (basalt composes the bootable image from its packages) composes the image from
  base packages.
- Item 2's run line, `packages/demos/0.1.0/least_authority_demo 9`, is not typed, because nothing
  installs the package for it to run. The name is not what stops it: `script/swish-check` runs the
  same bytes as `installed/least_authority_demo` (milestone 946), and the shell opens them by that
  20-byte name and sends them to the progenitor.
- The package carries the program at basalt's pinned nife commit, so it lags nife's `main` until
  the pin moves.

## Follow-on

- **Milestone 909.** Milestone 909 takes the demos out of the shipped image, since only base
  packages compose it, and the installer then takes `demos`.
- **Milestone 946.** A granted file name longer than 16 bytes:
  `design/roadmap/0946-a-granted-file-name-longer-than-sixteen-bytes.md`. It lets a path at the
  prompt end in `least_authority_demo` (calef, 2026-10-20 UTC, kept the name).
- **Outstanding.** Item 2's second half and the exit: `jig install demos` succeeding and the
  installed program squaring an integer, checked 2026-10-24 (UTC) against `script/swish-check`'s
  `after_reboot.rs`, which still expects the installer's refusal. It waits on milestone 909.

## Index row

The demonstration programs reach a customer as an optional package basalt builds from nife's own
source, and the installed program is granted what its note declares.
