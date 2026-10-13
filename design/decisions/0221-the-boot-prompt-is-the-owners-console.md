---
status: AMENDED
raised: 2026-09-26
decided: 2026-09-26
ratified_by: calef
---

# 221. The boot prompt is the owner's console

The question and its analysis are in
[notes/who-may-write-the-activation-set.md](../../notes/who-may-write-the-activation-set.md).

**Amended 2026-10-06 (UTC) by calef: there is no standing unauthenticated owner console.** Every
console shows a greeter, and an unauthenticated owner shell exists only in a recovery boot chosen at
boot. See "The amendment" below. The original text is kept unedited, with each passage the
amendment overturns marked where it stands.

## The ruling

*Clause 1 is amended: the console is no longer the owner's without a login. See "The amendment".*

calef, 2026-09-26 (UTC), *"Yes"* to both of the maintainer's questions:

1. The boot prompt is the owner's root console. The shell on the console before any login is
   the machine owner's, and whoever holds it is the owner, as with single-user or recovery mode
   elsewhere. It keeps §219's gate D2: the grant `crates/system_initializer` made provisionally on
   2026-09-26 is the rule. It may write the activation set, including vouching for bytes no source
   carries (§195 (a reviewed recipe vouches for a package) clause 3).
2. A `login` session gets D2 only for an identity on an owner-written list, empty by default.
   Until this ruling every session got it. This is calef's multi-user consequence in §219 made a
   mechanism: a machine can have users who may not run new native code.

## What it answers

*Amended: the hole stays closed, but by authentication rather than by definition, and the recorded
cost below no longer holds outside a recovery boot. See "The amendment".*

The note's first question: is the boot prompt allowed to vouch its own bytes? Yes, so the note's
option A holds. The note recorded a hole: the prompt can write `activation/` directly, because it
holds the file service's root endpoint and the server cannot tell its clients apart. That hole is
closed by definition rather than by a filter. The session that can write the table is the owner,
and the owner may. Options B (a root caretaker in front of the shell) and C (the server reserves the name)
are not built, and the note's second question lapses.

The recorded cost. On a machine whose console strangers can reach, anyone at the console is the
owner. That is the same trade single-user mode makes, and it is the reason a machine that must
defend its console needs the console itself defended, not a filter behind it.

## What was built on it

Lane `milestone/198-owner-console`, 2026-09-26. Names provisional.

- The list. `login_protocol::RUN_UNVOUCHED_LIST`, a file named `may-run-unvouched` at the root
  of the file service, one identity per line, `#` comments allowed. `login` reads it on every
  login, after authentication, and delegates D2 only to a listed identity. No file, an unreadable
  one, or one over a page lists nobody. The root is where this tree keeps what it knows about an
  identity (§117 (a principal's subtree is named by its identity)). It is also the one place a
  session cannot name, because a session is confined to its own subtree. The owner edits it at the boot
  prompt with `echo alice >> may-run-unvouched`.
- `vouch <path>`. The boot prompt asks the progenitor to record the file's digest in a new
  activation generation, under the path's last component, with `owner` in the package column
  (`activation_set::OWNER`). The bytes then run vouched with the installed manifest until a
  manifest travels in the executable (§197 (a package is one archive file), M2). A vouch is a
  generation, so `package rollback` undoes it. The proposal it was promoted from,
  `vouch-for-a-local-build`, recommended exactly that shape.

## What else was considered for the list, and why each lost

- An attribute in the credential store. It holds secrets and is sealed at boot
  (`components/src/credentialer.rs`): nothing can write it afterwards, which is its whole security
  argument. An owner's edit would need a reboot and a provisioning path to carry it.
- A file in each identity's own subtree. The session can write its own subtree, so it would
  grant itself the capability.
- A list the progenitor reads at boot and hands `login`. It works, and an owner's edit then
  waits for a reboot. Reading at login costs one open and one read of a page per login, and `login`
  already holds the root endpoint.
- Every session, as before. Refused by the ruling.

## What stays open

*Amended: "the boot prompt alone" below now means an authenticated owner's shell or the recovery
shell. See "The amendment".*

- Every activation verb is the spawn endpoint's. `install`, `remove`, `rollback` and `vouch` go
  to whoever holds it, which is the boot prompt alone, so today that is the owner. A session given a
  spawn endpoint would be the owner too. Before one is, the verbs need a presentation of their own,
  the way D2's `RUN_UNVOUCHED_BIT` has one. Recorded in `grant_plan::spawnproto`'s BUGS.
- Until the manifest note lands, a vouched build holds `uptime`'s manifest
  (`grant_plan::INSTALLED_MANIFEST_OF`): the output and nothing else. So vouching a build today
  narrows it (an unvouched one also gets the clock and configuration pages). The grant is the
  manifest's; the manifest is a stand-in.

## The amendment, 2026-10-06: the owner logs in too

Raised by the maintainer while calef ruled on the concurrent login sessions proposal (#1769). Its
Fork 3 chose virtual terminals, and with them this section as written makes console 1 an always-open
root shell beside the login prompts. The maintainer offered four options:

- A. Keep this section: console 1 never asks.
- B. Every console shows a greeter. An unauthenticated owner shell exists only in a deliberate
  recovery boot, as with Linux's single-user mode.
- C. Console 1 is the unauthenticated owner console only until an owner credential exists, then a
  greeter like the rest, with B's recovery boot.
- D. Only the serial line is the owner's unauthenticated console.

calef, 2026-10-06 (UTC), in order:

> "C with B. Can we make the owner set a password at first log in?"
>
> "Yes, installer asks, first-boot prompt as fallback. Is it the first-boot prompt or any boot where
> there is no password set?"
>
> "Yes, record it"

The last accepted the maintainer's answer, which is clause 3 below.

1. There is no standing unauthenticated owner console. Every console, console 1 included, shows a
   greeter. The owner is an identity holding owner authority, granted from the owner-written lists
   at the file-service root that #1769's Fork 2 chose. The only unauthenticated owner shell is a
   recovery boot chosen deliberately at boot. Physical access is the control there, as in every
   operating system's recovery mode.
2. The owner sets a password before any shell exists. `system_installer` (`installer` until #1773
   lands its rename) asks for it when it installs to disk.
3. Otherwise the prompt appears on any boot where no owner credential exists, not only the first.
   The condition is state, not an event, so it cannot drift (the ladder's first rung: the wrong
   state is unrepresentable). That covers a stick-only boot whose writes do not persist, which asks
   on every boot; a lost credential store; and a revoked or deleted owner credential (§108 (disabling a user's login credentials kills their durable session)).
4. If the credential store cannot be read, which is an error and not the same as holding no owner
   credential, the prompt refuses and points to the recovery boot. It never lets someone set a
   password over an owner who may exist.

What becomes of the activation-set hole. This section closed it by definition: the session that
holds the file service's root endpoint and the spawn endpoint is the owner, so its writes are the
owner's. That still holds, but the session now becomes the owner by authenticating as an identity
on the owner's list (or by a recovery boot), not by sitting at console 1, so the server still need
not tell clients apart. Clause 1 of the original ruling and its recorded cost apply only to the
recovery boot. Clause 2 (D2 only for a listed identity) stands, and §219's gate D2 is untouched.

Known costs, recorded and not ruled:

- Test images (swish-check, boot-check, the QEMU suites) need an owner credential preset at build
  time, close to milestone 49 (users, login, and attribution)'s generated first-boot password.
- Bench scripts (board-console, the soak tests) must supply that credential on radon and xenon.
- None of this is built yet. #1769's lane builds the greeter, the prompt and the recovery boot on it.
