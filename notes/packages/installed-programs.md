# The installed-programs table, line by line

`crates/installed_programs` holds what is installed, as a table that is never edited, only
succeeded (§208 (installing a package is granting it, and the installed programs are versioned)).
[packages.md](../packages.md) has where it sits in install, remove and rollback. This page is the
format the crate reads and writes.

## A generation

A generation is a text file that is never rewritten. Install and remove write the next generation,
and a one-line `current` names the live one, so a rollback rewrites one line, as a Nix profile
does. A generation holds three kinds of line:

```text
<digest> <program> <version> <package> <manager>   a row: what is installed, and who installed it
default <program> <digest>                         the pointer: what the bare word runs
grant <digest> network                             the owner granted those bytes the network
grant <digest> zone <zone>                         ... and names in one resolver zone
```

Any malformed line makes the whole table unreadable. A half-parsed table would vouch for whatever
survived.

## Rows key on the digest

A `<digest>` is `sha256:<64 hex>`, as `measured_boot::digest_text` writes it. Rows key on the
digest, which is the packager's attestation; `program`, `version` and `package` are labels. A
rebuild claiming a live version is a second, visible row, and two versions coexist.

There is no architecture column, because the digest is of target-specific bytes. There is no clock
either; the generation index orders.

The default pointer lives in the generation file, so a rollback restores it too, and every install
moves it. An owner's vouch has version `-` (`NO_VERSION`), claims no name (§229 (how a bare name at
the prompt reaches an installed program)) and never writes a pointer.

## Grant lines

A `grant` line is the owner's grant (calef, 2026-10-19 UTC, on #2405: "a"), what milestone 911
(scripted installs grant only what the owner's approval file allows) calls the grant record. It is
keyed on the executable's digest, one line per grant, sorted bytewise and each once, after the
pointers. A zone is spelled as the resolver spells it
(`domain_name_resolution_contract::is_canonical_zone`). The root zone, meaning any name, is `.`, and
that spelling is provisional.

A grant whose digest names no row makes the table unreadable, as a pointer to nothing does. A
narrowing writes the next generation without the line. A removal takes a removed row's grants with
it.
