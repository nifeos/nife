# How a contract and a runner are versioned

An appendix of [a package declares what it needs at run time](../0849-a-package-declares-what-it-needs-at-run-time.md):
its fork 5, versioning. calef ruled all nine rules below on 2026-10-19 (UTC), each as written here,
on [#2417](https://github.com/nifeos/nife/pull/2417). It is a separate file because the rules would
put the block over §212 (a prose budget)'s cap.

Reuse: `package_index::version` (dpkg's ordering, already in the tree), milestone 614 (two installed
versions of one program, each runnable)'s side-by-side versions, `rendezvous::BADGE`, and
`capability_manifest_note`. No outside resolver fits, because none of them reads a capability
manifest.

## What the tree has today

- Nothing of milestone 849 is built. The `provides` table fork 2 ruled is in no package file and in
  no gate: `helpers/packages.py` has no `provides` key. `Manifest::contracts()` does not exist.
- `depends = [...]` takes package names only and is a link lint. `evaluate` in
  `helpers/packages.py` allows a cross-package link when the target is an interface or in
  `depends`; nothing else reads it.
- The `.nifepkg` header (`package_archive`'s module documentation) carries a name, a version and an
  architecture, and no dependency field.
- Package versions are ordered by `package_index::version::newest`, which is dpkg's `verrevcmp`.
  `installed_programs::Entry::version` calls a version "a label, never an identity".
- The manifest note checks its own format version at decode (`capability_manifest_note::VERSION`,
  refused as `UnknownVersion`). That is the note's encoding, not any contract's.
- §235 (the OS is built and updated from packages)'s ABI revision field is ruled into the note and
  not built.
- `capability_plan::Requirements::contract` is "not matched against anything: what makes a build
  substitutable is that it speaks the protocol, and a string it carries cannot prove that", in its
  own documentation. No contract has a version exchange on the wire.
- `capability_plan::dependents` names the direct dependents to warn before one contract is swapped,
  and §231 (a swap's warning to a dependent is advisory) makes that warning advisory. It orders
  nothing.
- One install is one package. `jig` takes one name per verb (`main` in `jig/src/main.rs`), and the
  progenitor's activation stages one package and commits one generation (`install` in
  `crates/system_initializer/src/activation.rs`). `installed_programs::with_entries` writes rows
  from any package into one generation; the limit is not there.
- Two versions of one program install side by side, and a project's version set selects among them
  (milestone 614; `crates/swish/src/jig_versions.rs`).
- Only `line_editor` swaps live (§308 (the installer asks the terminal to swap)). Every other member
  of an install takes effect at its next spawn or the next boot. Service providers are base
  programs built by `system_initializer`, and base is updated through the boot slot (§235), a full
  copy per slot.

## The rules

### 1. The contract carries the version

`filesystem` is at level 3. A package never names another package's version. calef: *"Contract
seems to make sense."*

A contract can have two providers (redoxfs today, another filesystem later), and they need one
number to agree on. A package version is the upstream's claim, which `installed_programs` already
declines to treat as an identity. Refused: Debian's package-version dependencies
(`needs redoxfs >= 0.4`).

A level is an unsigned integer per contract, starting at 1 and only rising. The word "level" and
the constant's name are provisional.

### 2. The provider declares a range, the consumer one level

A provider declares the levels it serves (`filesystem 2..3`), and a consumer the one level it was
built against (`filesystem 3`). calef: *"Yes."* This is Fuchsia's shape: a component names one
target API level, each platform release supports several, and Component Manager refuses a
component whose level is retired.

The side that keeps compatibility code is the side that knows what it serves, and nife writes both
ends of every contract. Retiring a level is one edit, at the provider. Refused: OSGi's importer
range and Debian's `>=`, where a consumer guesses a future upper bound.

### 3. The build stamps levels

Each contract crate exports its current level as a constant. A program's note records the level of
each contract crate it linked, as Fuchsia stamps a package's ABI revision. A provider's top level
is derived the same way. calef: *"Build."*

The floor, the oldest level a provider still serves, is written by hand in its manifest, because
nothing in a binary shows how much old protocol it answers. That is a recorded exception to rung 1,
and a test per retained level keeps it honest. A script or a foreign binary with no contract crate
states its levels by hand in its block or note, as fork A's block states its needs.

### 4. A runner keeps its upstream version, and a script states a range

The interpreter's package provides one version (`python 3.12`). A script's block says
`runner = python 3.10..3.11`, one bare word, so fork A's grammar is unchanged. Ordering is
`package_index::version`. calef: *"Yes."*

Python 3.12 removed `distutils` (PEP 632), `asynchat`, `asyncore` and `imp`, so it does not serve
every 3.8 script, and nife does not write Python. This is PEP 723's convention: a script's
`requires-python` field states "the Python version(s) with which the script is compatible".
Contracts and runners differ in which side carries the range for one reason, who owns the
compatibility. Python 3.11 and 3.12 install side by side under milestone 614. Each script runs on
the newest installed runner in its range, and removing a runner some script still needs is refused.

### 5. The progenitor decides, at every generation commit

The progenitor checks the whole generation when it commits one: install, remove, rollback, and the
boot-slot update of §235, which runs it over the installed table before it commits a slot. It reads
levels from the notes of the members it places, so the package header needs no new field. `jig`
runs the same function earlier, from the index copy, to refuse before downloading and to order a
plan. The function lives in one crate both link (rule 7 of the codebase rules). calef:
*"progenitor."*

§270 (a package manager holds an installer endpoint, not the spawn endpoint) gave the client the
right to ask, not to grant. Refused: the decision in `jig` alone.

### 6. The progenitor checks again at spawn

When the progenitor endows a program with a contract, it compares the note's level with the
provider it built, and refuses with the contract and both levels named. calef: *"Yes."*

This covers the one path that skips a commit check: the boot falling back to the other slot after a
failed update, which §235 keeps so a bad base cannot brick a machine. Refused: a version exchange on
each contract's wire. A provider's word on the wire proves no more than its note, and an opcode in
every contract is the irreversible kind. A live provider swap, when one exists, is where it would be
reopened.

### 7. One generation per package, provider first

A provider's new release must still serve the level its last release served, as Fuchsia keeps a
level runnable through its sunset phase. A gate refuses a release that drops a level with no
release between. `jig install a b` stays one command, and `jig` orders the installs by level.
calef: *"Yes."*

Every intermediate generation passes rule 5's check, so rolling back to any of them is consistent
by construction. Runners never couple, because old and new interpreters install side by side.
Refused: a set install committing several packages in one generation. It is the one path that
admits a break with no overlap, which forces quiescing both ends and swapping both at once; this
rule makes that case unrepresentable.

So swap order follows without a rule of its own. When a provider swaps live, it goes first and
serves both levels while consumers follow, and `capability_plan::dependents` names whom to warn.

### 8. Additive first, and a breaking change puts the level in the badge

A new level that only adds operations, leaving the old ones unchanged, is served by one dispatcher
for every level. That is the first rule for changing a contract. For a breaking change, the
progenitor puts the client's level, read from its note, in the badge of the endpoint copy it hands
over. calef: *"Yes."*

The kernel delivers the badge on every receive (§230 (badged endpoint capabilities)), and a badged
copy cannot be re-badged, so a client cannot claim another level. Milestone 709 (a graphical
terminal session on the no-keyboard arm holds only the raw half of the boot discipline)'s
`RAW_ONLY_BADGE` and `spawnproto::INSTALLER_BADGE` narrow a surface the same way. It costs no
kernel change and no opcode. What it does cost:

- The level takes a fixed field of the 64-bit badge, bits 48 to 55 provisionally, and every
  contract's own badge constants stay clear of it (`system_log`'s per-writer badges, the socket
  stack's bit 63). A test in one shared crate checks that.
- `line_editor` treats any non-zero badge as raw-only. That rule becomes a test of its own bit
  before any terminal copy carries a level.
- A server that mints derived capabilities, as the socket stack does per socket, copies the level
  field from the badge it was reached through.
- A multi-level contract's copy is handed over already badged, so a client holding `GRANT` cannot
  mint its own level.
- One `BADGE` call per endowment, which `system_log`'s writers already pay.

Refused: an endpoint per level, since a receive here takes one endpoint with at most a bound
notification beside it (`abi::notification::BIND`). Refused: a breaking change as a new contract
name, which moves the dispatch problem without solving it.

### 9. A level stays served until no package in basalt needs it

basalt's gate refuses a provider release that raises its floor past a level some package in its
index still needs. On a machine, rule 5's check refuses an install that would strand a locally
installed consumer, so a file installed by path is covered too. calef: *"Yes."*

This is derived from the record, so it cannot be wrong about who still needs a level. Under §297
(software reaches nife as a package `jig` installs from basalt), every supported program is in
basalt's index. Refused: a number of releases and a period of time, guesses that are early for a
slow consumer and late for a dead one.

A third-party package gets its levels from nife's `std` port when basalt builds it (calef,
2026-10-19 UTC). Retiring a level therefore usually means a basalt rebuild, not an upstream patch.
A recipe that stops building keeps its level alive until a person fixes it or drops it.

## Prior art, and what each gave

| System | Read | Taken | Refused |
|---|---|---|---|
| Fuchsia API levels | [api_levels](https://fuchsia.dev/fuchsia-src/concepts/versioning/api_levels): one target level per component; supported, sunset and retired phases; ABI revision stamped at packaging; Component Manager refuses a retired level at start | the level on the contract, the provider's range, the build-time stamp, the sunset overlap | nothing material |
| Debian | [policy ch. 7](https://www.debian.org/doc/debian-policy/ch-relationships.html): section 7.5: versioned `Provides` takes only `=`; section 7.3: `Breaks`; section 7.2: unpack together for mutual exact versions | `verrevcmp`, already in the tree, for runner versions | the range on the consumer for contracts; unpack-together |
| OSGi | [core 8, module layer](https://docs.osgi.org/specification/osgi.core/8.0.0/framework.module.html): importer range, exporter single version, resolved before any code runs | resolution before run, which rule 5 is | the importer's range, for rule 2's reason |
| Nix | [`nix-env --upgrade`](https://nix.dev/manual/nix/2.24/command-ref/nix-env/upgrade): an upgrade of several packages is one new generation | generations, which §208 (installing a package is granting it) already has | the set as the only unit; rule 7 keeps one package per generation |
| PEP 723 | [script metadata](https://peps.python.org/pep-0723/): `requires-python` | the consumer's range for a runner | none |

## What it builds

Milestone 849's item 5:

- a level constant in each contract crate;
- a level per need and a floor per provided contract in the note, which takes the descriptor to
  version 2 beside §235's ABI revision;
- the shared check crate, and the runner range in fork A's block;
- basalt's floor gate (rule 9) and the release-overlap gate (rule 7);
- the level badge field, with `line_editor`'s bit test (rule 8).

Milestone 809 (the package client becomes a program)'s item 9: the progenitor runs the check at
every generation commit and at spawn (rules 5 and 6), and `jig install` takes several names and
orders them by level (rule 7).

## BUGS

- The floor of a provider's range is hand-written (rule 3), so it can claim a level whose code was
  deleted. Only a test per retained level catches that.
- A contract still names a kind of service, not an instance (849's own BUGS), so two providers at
  different levels leave the progenitor choosing. The spawn check refuses rather than picks.
- OSGi's specification was read only in its first 100,000 characters, which cover sections 3.2 to
  3.7.
