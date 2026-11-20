---
status: BUILT
raised: 2026-08-23
built: 2026-08-23
---
# 155. A provisioning tool: create an identity and its home subtree together

`milestone/155-identity-provisioner`, 2026-08-23 (an agent lane; see that pull
request's `**Lane:**` line for the record CLAUDE.md asks for). `components/src/identity_provisioner.rs`
(provisional name, an architect's to ratify) `PUT`s an identity and secret into the credential store
and `MKDIR`s its home subtree, as one tool invocation, tested end to end against a real credential
service and a real filesystem (`kernel/src/user/identity_provisioning_tests.rs`,
`kernel/src/user/identity_provisioner_service.rs`). All three questions this block itself raised
were answered by precedent, without a design fork (it needed none: minted with Gate: NONE, and
that held); see "What it needs" below for each.

## What this is, in brief

Unix's `useradd`, one level down: a tool that authenticates as (or is trusted by) an operator,
takes an identity and a secret, and does two things as one act rather than two. `PUT`s them into
the credential store over `cred_proto`'s `PROVISION` endpoint, and creates that identity's home
subtree (named by the identity string itself, DECISIONS §117) under whatever directory capability
holds the principal tree.

## What it needs, and how each question was answered

- **A capability contract, and who may run this tool.** Answered by the tree's own standing model
  rather than invented. Authority here is a capability held, never a checked identity, so "who may
  run this tool" is answered completely by "whoever is handed its two capabilities at spawn," the
  same shape `credentialer_test_client.rs`'s provisioner role and `login.rs`'s own BUGS already
  carry for "not wired into the interactive boot" (see this milestone's own BUGS, below, for the
  same bound). The tool holds `WRITE` (not `GRANT`; it delegates neither capability onward) on the
  credential service's provision endpoint and on the directory capability holding the principal
  tree. It is unnarrowed in this slice's own wiring for the same reason `login.rs` names for its own fixed
  subtree grant. A real deployment's answer to how an operator comes to hold those two capabilities
  is the same open boot-wiring question `login.rs` already leaves open, not decided here.
- **What happens if the identity already exists.** Checked, not assumed: `credentialer::Store::put`'s own
  rule is "a duplicate identity is refused" (`Error::Identity`, which the credential service maps to
  `cred_proto::MALFORMED`, the same code a malformed request gets, because neither is an
  authentication outcome). This tool does not special-case it; it reports the raw code back
  (`RPT_CRED_FAILED`, word 1 carrying `cred_proto::MALFORMED`) so a caller can tell a duplicate from
  every other refusal without a second vocabulary for the same fact. Proved by
  `a_duplicate_identity_is_refused_without_disturbing_the_original`.
- **One transaction or two.** Two, honestly: this system has no cross-server commit protocol between
  the credential service and the file service, and building one would be a real architectural
  undertaking, not a detail of this tool. The subtree is created first. A failed `MKDIR` is a
  clean no-op (nothing written to the credential store yet). A failed credential `PUT` *after* a
  successful `MKDIR` leaves an orphaned, empty, inert subtree, which is the accepted failure mode.
  Nobody can authenticate as an identity with no stored credential, so the orphan is harmless and
  recoverable by re-running the tool. The other order risks the opposite (a live credential for an
  identity with no home), which is dangerous rather than merely untidy once a caller wires
  per-identity subtree lookup (DECISIONS §117's own follow-on). A login could authenticate and then
  have nowhere defined to go. A pre-existing subtree (`EEXIST`) is treated as recovery rather than
  refusal: the tool proceeds to the credential half instead of failing a retry that finds its own
  prior work. The full argument is in `identity_provisioner.rs`'s own module docs, where a reader
  meets the code it governs.

## What it does not decide

Deprovisioning (removing an identity and reclaiming its subtree) is not this milestone's scope.
`components/src/login.rs`'s own BUGS already names reclamation as an open bound and this tool's removal
half, if it gets one, should be sequenced against that rather than invented independently here.

## Prior art

Unix `useradd`/`adduser` (the shape, not the mechanism: no uid, no `/etc/passwd` line, a
capability-shaped grant instead). See DECISIONS §117 for why the subtree name itself needs no
separate lookup.

## BUGS

~~Not wired into the interactive boot.~~ Resolved, 2026-08-27, milestone 49's boot-wiring
update. `crates/system_initializer::boot` now spawns this tool once per real boot, on both ISAs.
It stages a boot-generated password into its request page and holds both of its capabilities
(`credentialer`'s own provision endpoint, before its seal, and the file service's root) itself.
It provisions a demo identity (`operator`) before `login` can serve anyone. See
`design/roadmap/0049-users-and-attribution.md`'s own account of that wiring for the full design. An
operator's real path to holding this tool's two capabilities *for an identity other than the one
boot-generated demo account* remains real work this slice does not attempt.

**No `SEAL`.** Deliberately: sealing ends provisioning for the whole store, forever, and is an
operator's decision made once after every identity for a boot is in, not a side effect of one
identity's own provisioning. This tool's caller seals when it is done, exactly as
`credentialer_test_client.rs`'s provisioner role already does in its own tests.

**The directory capability this slice wires the tool against is the file service's whole root,
unnarrowed.** The same bound `login.rs` names for its own fixed subtree grant. A real deployment
scopes this to a dedicated "principal tree" parent directory once one exists, rather than the
filesystem's whole root; that directory does not exist yet and is follow-on, not invented here.

**The two-step ordering is proved by example, not exhaustively.** The subtree-first argument in
`identity_provisioner.rs`'s own module docs is exercised by two guest tests (a fresh identity, and a
genuine duplicate against a subtree the first attempt already created). That is not a proof over every
interleaving a real deployment's retries could hit. The two servers this tool talks to are each
proved independently and at length elsewhere (`cred_proto`'s Kani harnesses, `fs_proto`'s); the
orchestration between them is this milestone's own and is tested rather than proved.

This milestone found and fixed a latent single-instance assumption in `credential_service.rs`
that predates it, because it is the first caller to need a second, independently wired instance
in the same boot. The module used to remember each shared frame in one bare global (`FRAMES`,
keyed only on a virtual address) and hand it back through `verify_frame()`/`provision_frame()`.
Every earlier caller happened to be correct only because `credential_tests::provisioned()`'s
instance was the *only* one ever wired for the life of the test binary, so "the global's current
value" and "this instance's value" were always the same fact. A second `start()` call broke that
silently. The global would hold whichever instance ran most recently. A caller still asking the
global for the *other* instance's frame (`login_service::start`, `credential_service::client`/
`provisioner`, `credential_service::peek`, and the two SMB-adapter guest tests that call it) would
read the wrong page. Fixed by removing the global and its two accessors entirely: `Wiring` now
carries `verify_frame`/`provision_frame` itself, and every one of those call sites was updated to
read the specific `Wiring` it already holds rather than ask a shared global. All of it was exercised
by the existing `credential_tests`/`login_tests`/`riscv_virtual_io_tests`/`tests`/`std_tests` suites
before this milestone's own tests were added, so this is a fix with the tree's own pre-existing
coverage behind it, not an untested change riding along.

## Follow-on

- **Milestone 49.** Wiring this tool into the interactive boot, which was this block's first BUGS
  entry. `crates/system_initializer::boot` now spawns it once per real boot on both ISAs, holding
  both of its capabilities itself and provisioning a demo `operator` identity before `login` can
  serve anyone.
- **Refused.** Sending `SEAL`. Sealing ends provisioning for the whole store forever, so it is an
  operator's decision made once after every identity for a boot is in, not a side effect of
  provisioning one identity. The caller seals, exactly as the provisioner role in
  `credentialer_test_client.rs` already does in its own tests.
- **Recorded.** `design/roadmap/0155-user-provisioning.md` BUGS: an operator's real path to holding
  this tool's two capabilities, for an identity other than the one boot-generated demo account, is
  untouched.
- **Recorded.** `design/roadmap/0155-user-provisioning.md` BUGS: the directory capability this slice
  wires the tool against is the file service's whole root, unnarrowed. A real deployment scopes it
  to a dedicated principal-tree parent directory, and that directory does not exist yet.
- **Recorded.** `components/src/login.rs` already names session reclamation as an open bound, and
  deprovisioning (removing an identity and reclaiming its subtree) should be sequenced against it
  rather than invented separately here. It is not in this milestone's scope.
- **Recorded.** `components/src/identity_provisioner.rs` module docs carry the argument: there is no
  cross-server commit protocol between the credential service and the file service, so this is two
  acts rather than one. Subtree-first makes the failure mode an orphaned empty subtree rather than a
  live credential with nowhere to go. It is exercised by two guest tests rather than proved over
  every interleaving a real deployment's retries could hit.
- **Recorded.** `components/src/identity_provisioner.rs` carries a provisional name block: the program's
  name has not been put to calef, and what was refused and why is written where the next proposer
  would read it.

## Index row

Surfaced by DECISIONS §117 (a principal's subtree is named by its identity string, created at
provisioning time), which means a tool has to exist to do that creating. Nothing outside test
harness code calls the credential service's `PROVISION` endpoint today. `useradd`'s shape without
a uid.
