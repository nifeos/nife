# Leaving the central program table: how other systems separate privileged programs

[The proposal](../leaving-the-central-program-table.md) holds the end state. **Everything in this
appendix comes from memory and was not re-checked against a source** for this proposal. Read each claim at its source before quoting it.

## Five systems

| system | where a program's requests live | who decides | how the privileged few are kept apart |
|---|---|---|---|
| Android | each app's `AndroidManifest.xml`, inside the signed package | the installer, by each permission's protection level; the user, for "dangerous" ones | a `signature` or `privileged` permission goes only to an app signed with the platform key, or to an app under `/system/priv-app` that an allowlist file in `/etc/permissions` names |
| Apple platforms | entitlements, inside the code signature | the kernel and its daemons, at launch, against the signature | restricted entitlements (the `com.apple.private` family) work only in binaries Apple signed, or under a provisioning profile that grants them |
| Linux file capabilities | an extended attribute (`security.capability`) on the executable | whoever may set it: `setcap` needs `CAP_SETFCAP` | the attribute is not in the file's bytes; a copy of the binary loses it, and a write clears it |
| Fuchsia | a component manifest per component (`.cml`), compiled into the package | the parent realm, which routes each capability; `use` alone grants nothing | a security policy allowlist in the component manager's configuration names the monikers that may hold certain capabilities |
| seL4 with CAmkES, and Genode | the system's assembly or `init` configuration, outside the program | the system integrator, statically | everything is privileged or not by what the configuration routes to it; the program states nothing |

## Where nife sits

nife's note is closest to Android's manifest and Apple's entitlements: a claim inside the bytes the
vouch covers (§197 (a package is one archive file), option M2). It differs from both in what a
vouch is. Android and Apple trust a signer's identity at install and at launch. nife pins a digest:
§220 (signed builds: a vendor signs, a developer self-signs) checks a signature at install and then pins the digest,
and never asks who signed at spawn. So nife's analogue of Android's platform allowlist cannot key on
a signer. It keys on the measured archive, by name, which is what the pinned table in
[the end state](end-state.md) does.

nife differs from Fuchsia in who routes. A Fuchsia parent routes each capability explicitly. In
nife the command line routes what a person designates (a file, a directory, a stream), and the
progenitor endows what nobody designates (the clock, the domain, entropy) from the manifest. The
note plays the part of Fuchsia's `use`, and the vouch with the owner's grant lines plays the part
of the parent's `offer`.

nife differs from seL4 and Genode in that the program speaks at all. A static configuration is the
strongest form, and today's table is that form for 28 programs. It does not survive packages
installed after the build, which is why §219 (how the shell names an installed program to
the spawner) chose a note.

Linux file capabilities show the cost nife avoided. The grant lives outside the bytes, so it can
drift from them, and every copy tool must know to carry it. A registry of manifests held in the
installed-programs table, shape (b) in [the end state](end-state.md), would repeat that.

## One concrete example

Android's `REBOOT` permission has protection level `signature|privileged`. A downloaded app that
lists it in its manifest installs normally and never receives it. A system app receives it only if
it sits in the privileged partition and the allowlist names it.

nife's `reboot` maps onto that one for one. Any package may build a program; none may declare
`reboot` in a note, because the encoder refuses the field at compile time. The progenitor endows the
reboot object only to the pinned row named `reboot`, whose bytes the measured archive vouches for.
The difference is that Android lets a privileged app ask in its manifest and decides by allowlist,
while nife refuses the ask itself and keeps the whole grant in the pinned table. That is the
stricter of the two shapes, and loosening it later stays possible.
