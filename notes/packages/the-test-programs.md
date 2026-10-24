# The test programs a package carries, at the prompt

Name: provisional, minted 2026-10-24 (UTC) by lane `comments/929-fixtures-01`, named for its
content. calef names files.

An appendix to [notes/packages.md](../packages.md). Some fixtures exist only inside packages:
`fixtures` lists them in `packaged_only`, so no boot archive packs them. Each doc comment in
`fixtures/src/` says what the program holds and proves. This page holds what three of them print
at the prompt, as `script/swish-check` drives it. The fetch of `greeting` itself is in
[fetching.md](fetching.md), and the two-program package `duet` is in
[several-programs.md](several-programs.md).

## `greeting` reads its manifest note

`greeting` carries its manifest as an ELF note, and the note asks for a clock. No boot image has a
row for the program, so the note is the only place the progenitor can learn that. A vouched copy
that holds slot 1 is the proof the note was read, and the second line it prints says so:

```text
$ packages/greeting/0.1.0/greeting
hello from a package this image never carried
clock: held at slot 1, as its manifest note asked
```

## `greeting_v2` is a second installed version

`greeting_v2` is packaged as `greeting` at 0.2.0. Its line differs from version 0.1.0's, so the gate
can tell which copy ran:

```text
$ cd downloads
$ jig install 0.2.0/greeting.nifepkg
  installed; generation 3 is live
$ packages/greeting/0.2.0/greeting
hello from the second copy of the package
```

## `dialer` is refused what it asks for

`dialer`'s manifest note asks for the network, the zone `nife.test` and the root zone. The install
lists those asks, and nothing yet records an owner's grant of them, so the program is refused at
the prompt and never runs:

```text
$ cd downloads
$ jig install ./dialer.nifepkg
  dialer asks for:
    reaches the network (connects out)
    looks up names in nife.test
    LOOKS UP ANY NAME AT ALL (the root zone)
  none of these is granted: no owner's grant is recorded at install yet
  installed; generation 11 is live
$ dialer
  refused: its manifest asks for the network, and no owner's grant of the network is recorded for it among the installed programs
```
