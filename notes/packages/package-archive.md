# The package archive's decisions, step by step

`crates/package_archive` reads and writes the one file a nife package is.
[packages.md](../packages.md), "The format", has the layout's three choices. This page holds the
reasons behind the crate's installer decision and its catalog lookup, which its doc comments link.

## The installer's order

`installable` decides on bytes alone whether a package may be installed, and what is installed. The
order of its steps is the argument.

1. The header is parsed first, because the stem the catalog is keyed by is written in it. That is
   hostile input reaching `Package::parse` before any digest is checked, which is what that
   function's fuzz target and Kani harnesses exist for.
2. The whole file's SHA-256 must be the catalog's line for that stem. The catalog is the image's
   own (`package_catalog`), measured with every other archive entry. This is DECISIONS §195's "the
   image's measured table becomes the first source" taken literally: a person cannot install what
   the image does not vouch for, whatever file they point at.
3. Every program member is found (milestone 919 (a package carries several programs); calef,
   2026-10-12 UTC: "A package can contain many programs and they may be different from the package
   name."). The member named after the package is a program, and so is every member named
   `bin/<program>`, whose program is the name after the prefix. The package `ripgrep` carries
   `bin/rg`, as Debian's does. That convention is the installer's and is provisional, since nothing
   in the format marks which member is executable. A package with no program installs nothing. A
   `bin/` name that is empty or nested, or a program named twice, refuses the whole package. Where a
   program's manifest travels is DECISIONS §197 (a package is one archive file)'s open question,
   and the installer does not answer it.
4. Each program's bytes are checked against the table of contents' digest before any is returned.
   What is recorded is then a digest this installer computed rather than one it was told.

## A bare name means one version

`cataloged_stem` finds the catalog line for a package asked for by name. `name@version` asks for
one version, the same spelling `jig remove` takes, because a word with `@` in it is never a package
name. A bare name the catalog vouches for at two versions is refused as `SeveralVersions`.

Refusing is the reversible answer. Ruling 3 of milestone 614 says a bare *run* means the newest
install, but nothing in the catalog orders versions to say which cataloged one is newest. Taking the
first matching line instead would let the order recipe filenames sort in choose the version.

## The writer refuses rather than mangles

`write_package` refuses a name too long, a name with a NUL, a duplicate name, too many members, or
a buffer of the wrong size. An archive is a mapping, and a writer that silently changes a key has
lost data.
