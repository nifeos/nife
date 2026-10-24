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

**PARTIAL, 2026-10-14 (UTC).** basalt builds and publishes `demos`, and `jig` fetches it and
accepts its digest on all three architectures. The installer then refuses it, because every image
still carries `least_authority_demo`. Left: running an installed demo, which waits on milestone 909
and on a granted name longer than 16 bytes ("BUGS" below).

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
- `least_authority_demo` is 20 bytes, and a path component at the prompt is at most 16 (`grant_plan`
  refuses it: *"that is not a name: one component, at most 16 bytes"*). An installed copy could
  not be run by its package path even if it installed. The image's copy runs because a catalog
  name is not a path. The name stays; it is ratified (calef, 2026-10-20 UTC: "B"). This waits on
  lifting `filesystem_contract::grant::MAX_NAME`, which `grant_plan::MAX_FILE_NAME` mirrors (the
  proposal below).
- The package carries the program at basalt's pinned nife commit, so it lags nife's `main` until
  the pin moves.

## Follow-on

- **Milestone 909.** Milestone 909 takes the demos out of the shipped image, since only base
  packages compose it, and the installer then takes `demos`.
- **Milestone 946.** A granted file name longer than 16 bytes:
  `design/roadmap/0946-a-granted-file-name-longer-than-sixteen-bytes.md`. Running the installed
  `least_authority_demo` by its package path waits on it (calef, 2026-10-20 UTC, kept the name).

## Index row

The demonstration programs reach a customer as an optional package basalt builds from nife's own
source, and the installed program is granted what its note declares.
