---
status: DECIDED
raised: 2026-10-07
decided: 2026-10-07
ratified_by: calef
---

# 258. Names for two installed versions of one program

## The ruling

calef ruled on 2026-10-07 (UTC) on every name milestone 614 (two installed versions of one
program, each runnable, and a caller granted the one it needs) shipped provisionally, and on the
wording of its one new message. Its block's Follow-on listed them. None is left provisional.

## Ratified as they stood

- `program@version`, the qualified spelling. It is the same `@` that `package install` and
  `package remove` take, ratified 2026-10-03.
- The `as` recipe key, which packs a program member under another name.
- The activation set's `default` line kind, the pointer to what the bare word runs.
- `activation_set::NO_VERSION` (`-`), what an owner's vouch row carries as its version.
- `activation_set::versions_of` and `activation_set::without_version`.

## Renamed

| Was | Is | Refused, and why |
|---|---|---|
| the version-set file `versions` | `.jig-versions` | `versions`: a common word, so it collides with what a project already keeps. `.package-versions`, `.basalt-versions`, `.nife-versions`, `.catalog-versions`: calef chose the package manager's name |
| the module `swish::versions` | `swish::jig_versions` | it follows the file |
| `package_archive::StemMiss` | `package_archive::CatalogMiss` | `StemMiss`: "stem" is internal jargon (calef: "StemMiss is horrible") |
| `activation_set::Error::Ambiguous` | `activation_set::Error::SeveralVersions` | `Ambiguous` (calef: "ambiguous"); `AmbiguousName`, because the name is not ambiguous and the version is |
| the fixture `greeting_two` | `greeting_v2` | `greeting_two`: says "second", not what differs. `greeting_0_2_0`: noisy, and stale if the test version changes |
| the divergence notice `uptime 0.2.0 (repo specifies 0.1.0)` | `uptime 0.2.0 (.jig-versions asks for 0.1.0)` | `(repo specifies <v>)`: "repo" assumes a repository and does not name the file to fix. The file's full path: too long on every spawn, and the shell finds the file by walking up from the working directory anyway |

calef gave two rules for the file. It must be a dotfile, and it must qualify the word. `jig` is the
basalt package manager, ratified 2026-10-06 in milestone 809 (the package client becomes a
program), so the file is named for the tool that will install what it asks for.

`Error::SeveralVersions` takes the word `CatalogMiss::SeveralVersions` already used for the same
condition: more than one version is live, or cataloged, and nothing orders them. One condition now
has one word in both crates.

The notice's form is `<program> <ran> (.jig-versions asks for <wanted>)`. The shell prints the file
name from `swish::jig_versions::FILE`, so a later rename of the file cannot leave the notice naming
the old one.

## What did not move

The renames follow each name's provenance rule in `design/naming.md`. Each name's `Name:` line now
says ratified, with this section and the refusals. A dated account keeps the names it used: the
614 block's 2026-10-02 correction and ruling 4's own example, the mutation census of 2026-10-03,
and the audit report of 2026-10-03 that counts `greeting_two`.

`grant_plan::spawnproto::ActivationStatus::Ambiguous` is a different name. It is the wire status
the shell prints for both conditions, and the ruling did not cover it, so it is unchanged. Three
`package_archive` names keep the British spelling of "catalog": the stem lookup function, the
constant naming the catalog's archive entry, and the install refusal for bytes the catalog does
not vouch for. Renaming a public name or an on-disk entry belongs to the American-spelling sweep's
own batch, not to this one. Prose in the touched files now reads "catalog".
