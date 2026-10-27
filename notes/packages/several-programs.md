# A package carries several programs

The appendix to [notes/packages.md](../packages.md) for milestone 919 (a package carries several
programs), built 2026-10-14 (UTC).

A package may carry several programs. One `jig install` places each and writes one generation
with a row and a pointer for each. Each runs by its own bare name and with its own note's grants.
`jig remove` names the package and takes them all (calef, 2026-10-14 UTC: *"jig remove should work
on packages, not programs."*).

A program name another package holds refuses the whole second package, naming the program and the
holder, which the owner removes first (calef, 2026-10-14 UTC: *"If two packages offer the same
program name, refuse the second package. Need to remove the first one before installing the second
one."*). From the second boot of `script/swish-check`:

```
$ cd downloads
$ jig install ./duet.nifepkg
  installed; generation 7 is live
$ alto
alto: one of two programs in the package duet
clock: held at slot 1, as its manifest note asked
$ tenor
tenor: one of two programs in the package duet
clock: not held; slot 1 is empty
$ jig install ./solo.nifepkg
  refused: the package duet already provides the program alto; jig remove duet first; generation 7 is live
$ jig remove duet
  removed; generation 8 is live
$ alto
  no such program (try 'help' for the builtins)
```

## How it is built

- `package_archive::installable` finds every program member, the one named after the package and
  each `bin/<program>`, and checks each one's bytes against its digest before any is placed. A
  `bin/` name that is empty or nested, or a program named twice, refuses the package.
- The progenitor places every program under `packages/<name>/<version>/` and writes one
  generation through `installed_programs::with_entries`. One install is one generation (§208
  (installing a package is granting it, and the installed programs are versioned)). One refused
  program refuses the package with nothing written.
- A held name's refusal carries the refused member's index in the reply's word 2, and two more
  messages carry the holder's name (`spawnproto::holder_words`). `jig` names the program from the
  bytes it sent.
- `jig remove` is `installed_programs::without_package`.

## BUGS

- A generation is one 4,096-byte page, and a program's row and pointer take about 170 bytes. So
  the live set holds about two dozen installed programs, by arithmetic on `installed_programs`'s
  writer rather than measured.
- Every program of a package shares the 4 MiB staging limit (`spawnproto::IMAGE_MAX_PAGES`).
- A holder's name longer than 32 bytes, which only a hand-written table holds, is cut at that
  width in the refusal.
