---
status: PARTIAL
raised: 2026-10-12
milestone_dependencies: 809, 892, 910
decision_dependencies: 208, 252, 270, 297
machine_requirements: none
specific_machine: none
needs_person: no
---
# 911. Scripted installs grant only what the owner's approval file allows

**PARTIAL, 2026-10-28 (UTC).** Items 1 to 4 and 6 to 7 of the work are built; 1, 2, 3, 6 and 7
pass their host tests. The gate gates the Done-when rows in the approvals topic, all three
architectures ([row by row](0911-scripted-installs-grant-only-what-an-approval-file-allows/the-gate.md)).
Item 5's one-run form is built and gated with `jig` as the holder; the standing form waits on 910.

*(Minted 2026-10-12 (UTC) by lane `maintainer/mint-scripted-installs` at calef's request. The
number is provisional until the merge queue lands it; the title, the slug and every name below
(`--approved`, `jig approve`, `jig revoke`, `--json`, the file's keys, "approval file", "grant
record") are drafts. calef names them.)*

Appendices, in [this block's directory](0911-scripted-installs-grant-only-what-an-approval-file-allows/README.md):
[an example approval file](0911-scripted-installs-grant-only-what-an-approval-file-allows/an-example-file.md),
[the prior art](0911-scripted-installs-grant-only-what-an-approval-file-allows/prior-art.md), [the
gate](0911-scripted-installs-grant-only-what-an-approval-file-allows/the-gate.md).

calef, 2026-10-12 (UTC): *"If the owner does approvals, how will we script installs?"* The
maintainer proposed the design below, and he answered: *"Yes, write it up as a milestone."*

**In brief.** The owner writes an approval file once, naming the packages it may install and the
exact grants each may receive. `jig install --approved <file> <pkg>` installs with no prompt
when the manifest asks for no more than the file allows, and fails closed, naming the extra grant,
when it asks for more. A script that installs on its own holds an installer endpoint badged to one
approval file, so the progenitor refuses it anything outside that file.

## Why a plain `--yes` is refused

Under §208 (installing a package is granting it, and the installed programs are versioned), `jig
install` shows everything the install grants, and one confirmation covers it. Milestone 910 (a
package declares a service: install starts it, boot starts it, update restarts it) adds "runs as a
service, now and at every boot" to that list. Milestone 892 (a std program at the prompt holds the
network and a name resolver)'s item 3 (ruled "c" on #2011) adds the manifest's resolver zones,
asked for and granted at install. Each granted zone is one badge, under §252 (a resolver grant is
one zone per client badge). A root-zone ask is flagged apart.

A `--yes` answers that confirmation without reading it. It would approve a root-zone ask blindly.
On the next update it would approve whatever the new version asks, so a package that gained a
zone, a service or a directory would get it with nobody having seen the request. That is the one
thing the confirmation exists to prevent. The approval file moves the owner's judgment earlier:
the owner reads the grants once, when writing the file, and a script can only replay them.

## What exists, and who else touches this

Checked 2026-10-12 (UTC) on `main` and in #2011, #2022 and #2023, since landed. Nothing owns
pre-approved or unattended installs; each neighbor owns a piece this milestone reads.

- Milestone 809 (the package client becomes a program, `jig`), PARTIAL, owns `install`, `remove`
  and `rollback`; this adds a flag and two verbs. §270 (a package manager holds an installer endpoint, not the spawn endpoint) gives `jig` a copy of the progenitor's
  spawn endpoint, badged per job and narrowed to `WRITE`; only the owner's console grants it.
  Delegation here is a second badge on that same endpoint, not a new object.
- Milestone 910 owns the service grant and milestone 892 the zone grant; this milestone records
  and compares.
- §297 (software reaches a nife system as a package `jig` installs from basalt's index, the kernel
  included) makes every program an install, every one of which passes a confirmation, which is why
  scripting it cannot wait.
- Milestone 909 (basalt composes the bootable image from its packages) composes an image with
  nobody at a prompt; it reads this file, and a package asking more than it allows fails the
  build. Milestone 858 (lab machines update themselves through packages, and only a new kernel
  reboots them) ruled in its Fork 6 that automatic updates are an owner setting; this file bounds
  them, and 858 depends on this milestone.

909 and 858 are not dependencies of this block, though both use it: 858 waits on silicon and a
person, and the edge here would hold scripted installs behind a board.

## The approval file

The file is TOML, like the manifests of milestone 611 (every program and crate belongs to a
package, and every package has a home), ruled by calef on 2026-10-12 (UTC) after he asked for
"something more standard like JSON or yaml". It must hold:

- Its name, so a delegation and an error can say which file refused. Per package, by name:
  - the resolver zones it may receive, each one a separate entry (§252's one badge per zone), with
    `root` spelled out rather than implied;
  - whether it may run as a service (910);
  - the directories it may hold, each with its rights;
  - any other capability a manifest can ask for (the network, and the installer endpoint itself),
    by the same names `jig install`'s confirmation prints;
  - whether an update may be installed (yes or no), and whether the package may be removed.
- Nothing else. No wildcards over packages, no "any zone", and no version ranges. Which version
  installs is the question of §195 (a reviewed recipe vouches for a package, and the machine's owner
  may overrule it), not of this file.

[An example file](0911-scripted-installs-grant-only-what-an-approval-file-allows/an-example-file.md),
named `lab-runner`, approves two packages with provisional keys.

The rule is a subset test. An install or update passes when the manifest's asks are a subset of the
file's entry for that package, and the grants given are exactly the asks. When the asks exceed the
file, the install fails before anything is fetched past the manifest, and the error names each extra
grant, for example `refused: monit 5.36 asks for zone example.net, which lab-runner does not
allow`. It never grants the intersection: a program that silently got less than it asked would fail
at run time, far from the cause. A removal passes when the entry says `remove = true`.

Only `jig` parses the file. It compiles it on the owner's console into canonical grant records:
one per package, sorted, in the encoding the installed-programs table already stores
(`crates/installed_programs`, §208 (installing a package is granting it, and the installed
programs are versioned)). A human-readable file is for the owner. The progenitor compares records,
and never parses the file. The subset test runs in the progenitor, against the store's records the
trailer binds: a scripted install sends an `answered` trailer naming the approval and its digest,
which the progenitor could not otherwise know. `jig` pre-flights the package's entry before
anything is fetched past the manifest.

## The grant record, which every path writes

The grant record is the part of an installed-programs row that says what the owner granted that
package: its zones, its service, its directories and its other capabilities. One canonical
encoding, no timestamp, no trace of how the answer was given. Three paths write it:

1. An interactive `jig install` at the owner's console: the owner reads the list and says yes, and
   the record is the asks.
2. basalt's image composition (909), on the host: each package passes the same test against the
   image's approval file, and the image's first installed-programs table holds the records.

For the same grants, the records from both paths are byte-identical. How the answer was given is
recorded beside the record, not inside it, in a column like §270's manager column: `console`, or
the approval's name and digest. That keeps the audit question ("who allowed this?") answerable
without making two equal grants look different to anything that compares them.

## The authority is a capability

`--approved` at the owner's console needs nothing new: the owner already holds the authority, and
the file only replaces the prompt with a check that fails closed.

At the owner's console, `jig approve <file>` prints every grant the file contains, flags any
root-zone entry the way an interactive install does, and asks once. On yes, `jig` sends the
compiled records to the progenitor as the new activation request `Approve`, which only an owner's
(undelegated) badge may send. The progenitor stores the approval by name and digest. Approving a
name again shows only the grants changed since the stored digest, and the replaced records reach
every delegation already running from its next request.

The owner then starts the job that will install, with the grant "installer, within approval X".
The progenitor places an installer endpoint whose badge carries X, so no holder can forge or widen
it (§230 (badged endpoint capabilities: how a server learns which client's frame a request is
in)). Two forms exist. One run: the console's `--within <name>` on a job, ending at the reap.
Standing: a service package's manifest ask (910), granted at install like any other ask, which is
how the updater (858) holds it across reboots.

The progenitor checks every delegated request against X's stored records before anything is
written, because a delegated holder is trusted to be nothing more than a holder of that badge. A
holder under X cannot send `Approve` or `Vouch`. A `Rollback` under X passes when every row it
restores lies within X's records, and fails naming the row when one does not (ruled 2026-10-12,
read 2026-10-28 UTC): returning to what the file allowed grants nothing new. A row the live
generation already holds is not something the rollback grants, so the check reads the target
generation's rows the live one does not: a rollback that only removes passes, and one that would
bring back a package the file does not bound is refused. The owner's vouch rows lie outside every
approval, so the check skips them; it still restores them.

Approvals are not generations: the progenitor stores them beside the installed-programs table's
generations, never as rows in them, so `jig rollback` restores packages and leaves every approval
as it was (ruled 2026-10-12). Changing an approval stays `jig approve` or `jig revoke`, and `jig
revoke X` answers every later request under the badge `Revoked` with nothing read past its first
message. Editing the file changes nothing until the owner runs `jig approve` again (ruled
2026-10-12). DECISIONS §270 carries these methods' semantics as amended on 2026-10-28 (UTC), per
§10's rule for a new method.

## `jig` stays on one machine

calef, 2026-10-12 (UTC): *"maybe we are overloading jig. It should handle installs to the local
system. So adding repositories, removing repositories, installing, upgrading, removing packages,
updating its local index. There are other layers handling the automation of groups of servers,
etc."* And: *"I just want to ensure that a fleet options is possible. I don't want to define it
now."*

So `jig` covers one machine: repositories (add, remove), its local index, install, upgrade, remove,
rollback and approvals. Managing a fleet (role files, schedules, signing, fleet status) is a
separate layer, out of scope and not defined. Two hooks keep it possible: `Approve` leaves room for
a later signed form, and results and refusals come in a machine-readable form.

## Prior art

Read 2026-10-12 (UTC) and kept [in an appendix](0911-scripted-installs-grant-only-what-an-approval-file-allows/prior-art.md).
apt's `-y` and Android's auto-grant policy approve everything, including what an update newly asks.
NixOS declares which packages run but not what they may do. Fuchsia's capability allowlists,
checked by the granting authority, are the nearest model; this file keeps its list per package.

## The work

1. The grant record: one canonical encoding in `crates/installed_programs`, written by interactive
   install, and the "how it was answered" column beside it. Host tests that two paths agree.
2. The approval file, in TOML: `jig` parses it and compiles it to grant records. Host tests for
   the subset test and its error, which names every extra grant.
3. `jig install --approved <file>`, and the same flag on update and `remove`.
4. `Approve` and `Revoke` on the installer endpoint, and the badge that carries an approval, as
   §270's semantics amended in `design/decisions/` (per §10 (process model: capability-based,
   microkernel; untyped memory deferred)). Re-approval as ruled, and room in `Approve` for the
   signed form. Approvals live beside the generations, never as rows in them, and a `Rollback`
   under a badge is checked against that badge's approval, skipping the owner's vouch rows.
5. The two delegation forms: a one-run grant at spawn from the owner's console, and the standing ask
   in a service manifest beside 910's declaration.
6. A host-side check basalt's composition can call, sharing item 1's crate, so 909 can adopt it.
7. A machine-readable form of `jig`'s results and refusals (`--json`), with documented, stable
   exit codes. A refusal names each extra grant as data.

## Built 2026-10-28 (UTC)

Lane `milestone/911-scripted-installs-approval-file` (#2853). All seven items are built
([what was built, per item](0911-scripted-installs-grant-only-what-an-approval-file-allows/the-gate.md)):
the grant record and its `answered` column (`crates/installed_programs`). The approval file's
grammar, its compile and its host-side check (`crates/approval_file`). `--approved` on install and
remove, with the subset test in the progenitor, against the store. `Approve` and `Revoke` on the
installer endpoint, with the delegated badge, the store, and §270 amended in place. The one-run
delegation (`--within <name>`). The host-side check for basalt's composition. `--json` with the
documented exit codes. The host tests cover the two-paths-agree property, the subset test's
extras, the store's grammar and the record's reads. The seed writes the fixtures and three
approval files.

## Done when

Under QEMU on aarch64, riscv64 and x86_64, in `script/swish-check`, with fixture packages:
1. A scripted install with a matching approval file succeeds with no prompt, and the package runs
   with exactly its asked grants.
2. An update whose manifest adds a zone fails, names the zone, and leaves the previous version
   installed and running.
3. A delegated installer endpoint cannot install a package outside its file, nor a listed package
   asking more than its entry, nor send `Approve`. After `jig revoke`, it can install nothing.
4. The grant record from an interactive yes is byte-identical to the scripted one for the same
   grants.
5. An edited file changes nothing until `jig approve` runs again, which shows only the changed
   grants; a delegation already running then installs within the new records, with no restart and
   no new endpoint.
6. A refused scripted install exits with its documented code, and `--json` names each extra
   grant.
7. A delegated holder rolls back to a generation within its approval, and is refused a rollback
   that would restore a row outside it, which the refusal names.
8. A rollback leaves approvals unchanged: after `jig revoke X`, rolling back to a generation from
   before the revoke leaves X revoked, and an approval made after the restored generation stays.

## Ruled 2026-10-12 (UTC)

calef answered "Yes, record them" to the three open questions, recorded on #2030 and #2031, and
the body carries each: the file's digest binding (a), the delegated rollback within the approval
(b), and approvals beside the generations (a). The file is TOML. `jig` keeps to one machine.

Reuse: §270's badge and activation request, `crates/installed_programs`, and 910's and 892's grant
encodings, extended rather than duplicated. The design takes Fuchsia's checked-by-the-granter
allowlist and Android's per-permission grant state; debconf preseeding was the shape that
pre-answers questions without bounding them. The service ask is note version 3, one byte after the
zones; a manifest that asks for zones and no service stays version 2, so no digest changes.

## BUGS

- **910 is unbuilt, so the service ask is exercised by fixtures only** (2026-10-28 UTC). The
  version 3 note carries the ask, the record the grant, the confirmation the line and the subset
  test the bound; nothing starts a recorded service. A real service package in the wild waits on
  910, as does the standing delegation form, whose ask ("installer, within approval X") rides
  910's declaration. The gate proves the delegation with `jig` as the holder, the only program
  that can hold the endpoint, because a note cannot spell the installer ask (§270's rule,
  unamended on that point).
- A note cannot spell the installer ask, and no manifest asks for a directory at install, so the
  `installer` and `dirs` allow keys and the `Granted::Installer` and `Granted::Directory` records
  are exercised by host tests alone until 910's declaration exists.
- The delegated holder in the gate is a fresh `jig` per line, so "a delegation already running"
  is the mechanism's property (the check reads the store per request, shown by the
  refused-then-allowed sequence), not a long-lived holder's. A holder that lives across requests
  waits on 910's standing form.
- A store write is not staged and renamed; a torn `approvals/<name>` file fails to parse, which
  refuses every delegated request under it until `jig approve` runs again. Fail-closed, the same
  exposure a generation's own write has before its commit.
- The approval ids are per-boot: a badge from before a reboot carries an id the new boot's table
  answers `Revoked` to, until the standing form's re-placement (910's) or a fresh delegation.
- The store is the progenitor's to read, and a std holder's namespace root is the directory it was
  granted, so the diff travels in the `Approve` reply; on a boot with no disk the store write
  fails and says so. The `update` answer is enforced where the approval is stored (delegated
  installs of installed packages): a console `--approved` install of an installed package is the
  owner acting, and the answer does not bind it; the file's asks still do.

## Follow-on

- **Outstanding.** The standing delegation form: the ask in a service manifest beside 910's
  declaration, so a lab machine's updater (858) holds it across reboots. Checked 2026-10-28: 910
  is NOT-STARTED, and no manifest can spell the ask.
- **Outstanding.** A real service package in the wild (858's updater, monit): same dependency.
- **Milestone 909.** The image composition reads an approval file and calls
  `approval_file::check` per package. Checked 2026-10-28: the check is host-callable; 909 has not adopted it.

## Index row

Scripted installs without a blind yes. The owner approves packages and their exact grants once, in a
file. `jig install --approved` and a delegated, badged installer endpoint install only within it and
fail closed, naming the extra grant; basalt's image and lab updates use the same file.
