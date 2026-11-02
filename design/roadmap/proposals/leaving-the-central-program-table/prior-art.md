# Leaving the central program table: how other systems give privilege to a program

[The proposal](../leaving-the-central-program-table.md) holds the end state. **Everything in this
appendix comes from memory and was not re-checked against a source** for this proposal. Read each
claim at its source before quoting it.

## Where a program's requests live, and who grants them

| system | where a program's requests live | who decides | how the privileged few are kept apart |
|---|---|---|---|
| Android | each app's `AndroidManifest.xml`, inside the signed package | the installer, by each permission's protection level; the user, for "dangerous" ones | a `signature` or `privileged` permission goes only to an app signed with the platform key, or to an app under `/system/priv-app` that an allowlist file in `/etc/permissions` names |
| Apple platforms | entitlements, inside the code signature | the kernel and its daemons, at launch, against the signature | restricted entitlements (the `com.apple.private` family) work only in binaries Apple signed, or under a provisioning profile that grants them |
| Linux capabilities | an extended attribute (`security.capability`) on the executable, or the process's inherited sets | whoever may set it: `setcap` needs `CAP_SETFCAP` | `CAP_SYS_BOOT` alone lets `reboot(2)` run; any binary the administrator marks may hold it, and nothing ties it to the name `reboot` |
| Fuchsia | a component manifest per component (`.cml`), compiled into the package | the parent realm, which routes each capability; `use` alone grants nothing | a security policy allowlist in the component manager's configuration names the monikers that may hold certain capabilities |
| Nix | the store path, by its hash | the daemon's configuration: `trusted-users` may add substituters and import unsigned paths | trust attaches to a user, and through that user to whatever paths they import; nothing attaches to a program's name |
| seL4 with CAmkES, and Genode | the system's assembly or `init` configuration, outside the program | the system integrator, statically | everything is privileged or not by what the configuration routes to it; the program states nothing |

## Roles and alternatives: who answers a function's name

| system | how a name or function finds a program | who chooses | several providers |
|---|---|---|---|
| Android roles (`RoleManager`) | a role such as the default dialer, SMS app, home screen or browser; the system routes the function's intents to the holder | the user, in settings or on a system prompt the app requests | several apps may qualify; exactly one holds an exclusive role, and holding it brings the permissions the role carries |
| Debian `update-alternatives` | a generic name (`/usr/bin/editor`, `x-terminal-emulator`) is a symbolic link through `/etc/alternatives/` to one provider | the administrator, or automatic mode by each provider's priority | every installed provider registers; one wins; removing it falls back to the next |
| Fuchsia capability routing | a protocol name (`fuchsia.hardware.power.statecontrol.Admin`) is offered by one component and routed to its users | the product's assembly | one provider per route; another provider is another route |

## Where nife sits

nife's note is closest to Android's manifest and Apple's entitlements: a claim inside the bytes the
vouch covers (§197 (a package is one archive file), option M2). It differs from both in what a
vouch is. Android and Apple trust a signer's identity at install and at launch. nife pins a digest:
§220 (signed builds: a vendor signs, a developer self-signs) checks a signature at install and then
pins the digest, and never asks who signed at spawn.

The owner's grant of 2026-11-02 puts nife between Android and Linux. Like Android's runtime
permissions, the owner answers an ask the program states, at install, and the answer attaches to the
installed bytes. Unlike Android's `signature|privileged` level, no platform key and no allowlist of
names decides the high-authority grants: the owner does. Linux `CAP_SYS_BOOT` on a file is the
nearest shape for `reboot`. It is one capability, it is the whole authority, any binary may hold it,
and the administrator's act on that binary is what grants it. nife's grant line is that act,
recorded beside the digest instead of on the file, so a copy of the bytes keeps the same identity
and a changed byte loses the grant.

The role table in [the command-name appendix](command-names.md) borrows from Debian and Android in
one way each. From `update-alternatives` it takes a generic word bound to one provider, with the
built-in as the fallback. From Android roles it takes the owner, not the package, as the one who
chooses. It refuses Android's coupling of role and permission: holding the word `reboot` grants
nothing in nife, and the grant line alone endows the object.

nife differs from Fuchsia in who routes. A Fuchsia parent routes each capability explicitly. In nife
the command line routes what a person designates (a file, a directory, a stream), and the progenitor
endows what nobody designates (the clock, the domain, entropy, and now `reboot`) from the manifest,
under the vouch. The note plays the part of Fuchsia's `use`, and the vouch with the owner's grant
lines plays the part of the parent's `offer`.

Nix's `trusted-users` is the caution. Trust given to an identity that can then make more things
trusted spreads further than the owner sees. An `installer` grant has that shape, which is why
[the security appendix](security.md) keeps it the strictest of the classes.

## One concrete example

Android's `REBOOT` permission has protection level `signature|privileged`. A downloaded app that
lists it installs normally and never receives it.

Under the ruling, nife's answer differs. A package from a repository the owner added declares
`reboot` in its note. `jig install` shows the ask in capitals beside what it means, and the owner
answers yes. The installer writes `grant <digest> reboot`. When the owner runs that program, the
progenitor finds the line, endows the reboot object at slot 13, and the program may restart the
machine. Without the line, the same bytes run and hold nothing, exactly as a downloaded Android app
that asks for `REBOOT` holds nothing.
