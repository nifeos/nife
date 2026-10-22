# The nine forks as calef read them

An appendix of [lab machines update themselves](../0858-lab-machines-update-themselves.md). These are the
options and recommendations as presented on #1805 on 2026-10-07 (UTC), kept as the argument. Every
fork is ruled; the rulings, with his words, are the main body's table, and where a ruling recast a
fork (5, 6 and 9) the ruling wins over the text here.

## Fork 1: what replaces the floor when a package breaks its own rollback (ruled 1a, a trial)

- 1a. Generations get tries, like slots. The progenitor marks a new generation on trial, and if the
  boot's required set (`console`, `input`, `line_editor`, `swish`, `job_undertaker` today) fails
  or the boot is not confirmed, the next boot starts the previous generation. systemd's boot
  counting and greenboot do this for boot entries; this applies it one layer up.
- 1b. A small recovery set stays in the slot: a recovery shell and `net_stack`, used only when the
  generation fails. §241 (a threadbare base) noted this and did not rule it. It reopens the floor the model removed.
- 1c. A person with a stick or network boot. True today, and not unattended.

Recommend 1a, with the chooser's own image as the last resort it already is. It keeps the slot to
what nothing can restart, and it is the mechanism §208 (installing is granting) already has, with tries added.

## Fork 2: how the progenitor accepts a base package update (ruled 2a, a trial)

calef's model needs the progenitor to accept a newer base program than its slot names.

- 2a. A trusted key vouches (§220 (signed builds), built by milestone 666 (a signed build installs up to its key's ceiling)), and the progenitor checks the package's
  ABI revision against the running kernel's. A package needing a newer ABI is staged, and activates
  on the boot that confirms the new slot.
- 2b. A signed index the progenitor verifies (809's option I3). Narrower trust, but every channel's
  index needs a key the progenitor holds.
- 2c. The slot's catalog stays the only vouch. Every base update is then a slot, which is the model
  calef refused.
- 2d. The owner's `vouch`. §229 (how a bare name reaches an installed program) refused letting a vouch claim a bare name, so a vouched base program
  could run only by path.

Recommend 2a. The ABI check is what makes model point 3 a rule rather than a hope. §229's refusal
narrows to the slot's programs in the same change.

## Fork 3: how new bytes take effect

- 3a. Immediately, by kind. A per-use program at its next spawn. A swappable component by live
  swap. A long-lived service by a restart in `capability_plan`'s dependency order, the way NixOS
  restarts changed units. A service with state it cannot hand over is restarted and its clients
  reconnect.
- 3b. At the next boot, as Fuchsia does for its base. Simple, and the reboot calef asked to remove.
- 3c. `needrestart`'s way: install, then report what still runs old bytes, and the owner or the lab
  run triggers restarts.

Recommend 3a for the lab, because testing the restart is the point, with 3c's report printed either
way. The supervisor that restarts is the progenitor for the services it starts, until a real boot
runs `root_supervisor`.

## Fork 4: what marks an update good

- For a package: the restarted service reports ready, and a probe it answers passes, the way
  milestone 23 (a capability-routed component OS with live replacement)'s swap reports `PROBE_SURVIVED`. Fail, and `jig rollback` restores the generation and
  restarts the old bytes.
- For a slot: the shell answers, the network is up, and the lab index is fetched and its digest
  checked. Milestone 554 (a good upgrade sticks)'s file-server-ready is too early; the whole run passing would roll back a
  healthy kernel for an unrelated red test.

Recommend both, with the run's own result recorded and not gating. Fuchsia's committer and Android's
`update_verifier` confirm what the next update needs, and so does this.

## Fork 5: one step or two for a kernel update

- 5a. Two steps. The slot is confirmed, then the generation moves.
- 5b. One step. A generation records the slot build it needs; it activates on the boot that confirms
  that slot, and a slot rollback restores the previous generation.

Recommend 5b. Model point 3 makes them depend on each other, and 2a's ABI check is what reads it.

## Fork 6: who decides when to update

- 6a. The machine polls its channel at boot and when idle, like `update_engine`, Mender and Fuchsia.
  Needs nothing inbound.
- 6b. The CI run pushes, LAVA's model. nife has no inbound shell, no workflow reaches the lab, and
  calef chose manual power for radon (milestone 224 (nothing can power-cycle radon)).
- 6c. The boot server picks, for netbooted machines. A netbooted machine installs nothing and
  spends no try, so it tests none of this.

Recommend 6a. Network boot stays the recovery and bench path. Which mode a machine is in is calef's
switch, since it changes his lab workflow.

## Fork 7: how radon and argon choose a slot

- 7a. `uefi_loader`'s chooser through U-Boot's `bootefi`. The loader builds for riscv64 and
  aarch64 already (`notes/boot-stick.md`), so all three machines share one chooser and one format.
  Whether radon's U-Boot 2021.10 has `bootefi`, and whether its block I/O writes: unverified.
- 7b. U-Boot's `bootcount`, kept in radon's QSPI environment, written on every trial boot.
- 7c. A tries file on the card's FAT, written by a U-Boot script with `fatwrite` (unverified).

Recommend 7a, with 7c as the fallback. 7b writes the flash radon cannot boot without. Under the
model, the slot changes only with the kernel, so this matters less than it did.

## Fork 8: where the lab channel lives and when it advances

basalt pins nife by commit and bumps the pin daily at 06:17 UTC. It publishes nothing a machine can
fetch: its gate's builds are 14-day workflow artifacts. No channel exists.

- 8a. A `lab` index at `basalt.nifeos.org`. Waits on the index path (§250 (an image names its
  distribution's package index)), TLS (milestone 801 (packages over the internet)), and calef publishing.
- 8b. A `lab` index on patagonia over the LAN, served by `helpers/package-http-peer`, the same
  format as 8a.
- 8c. Package bytes as GitHub Releases on basalt, the index on patagonia.

Recommend 8b now and 8a later. The channel advances on each basalt gate that passes, with the pin
bump triggered per nife merge. The runner minutes that costs are not measured.

## Fork 9: the update index is a TUF repository

Fork 2's trial signs packages. A signature alone does not stop an attacker, or a stale mirror,
serving an older signed package, freezing the index at a vulnerable moment, or mixing packages from
two releases. Nor does it say how to survive a stolen key. The Update Framework answers those with
separate roles (root, targets, snapshot, timestamp), a threshold of root keys, root rotation,
version numbers that only grow, and a timestamp that expires. That expiry is the freshness §196
(nife carries TLS) named as missing: "an expiry in signed metadata (the Update Framework's timestamp
role ...), which the format fork's metadata rows do not yet carry".

- 9a. basalt publishes the index as a TUF repository. `jig` and the progenitor verify the metadata
  before trusting a package. 809's I3 (a signed index) and 666's signed builds take TUF's shape:
  a publisher's key is a delegated targets role, and 666's per-key ceiling rides in the
  delegation's custom fields.
- 9b. Fork 2's per-package signatures plus an expiring signed index of nife's own design. Fewer
  parts, and it rebuilds TUF's roles one incident at a time.
- 9c. Per-package signatures only. Leaves rollback and freeze attacks open.

Prior art: Fuchsia's package resolver and Bottlerocket's `updog` both verify TUF, PyPI accepted it
(PEP 458), Sigstore delivers its trust root through it, and Uptane extends it for vehicle fleets.

Neither Rust client builds for nife's targets today, and both fail the same way the tree's TLS
probes did: their C crypto. rust-tuf (Fuchsia's) is runtime-agnostic and needs no thread. Its
crypto is `ring`, used in 11 files, and its last crates.io release (2022) no longer compiles on our
nightly. tough (Bottlerocket's) is actively released, but needs `aws-lc-rs` and tokio's file system,
which runs on a thread pool, and our `std` is single-threaded. The measurements are in
[the TUF appendix](tuf.md).

Recommend 9a, built on rust-tuf. Swap its `ring` calls for the RustCrypto verifiers
`cryptography_provider` already builds, behind a feature offered upstream. Offer the Kani proofs of
the metadata checks upstream too, per calef's standing direction on #1806. Writing our own client
loses to both on every count except the size of the patch. A key in an image is what §195 (a reviewed recipe vouches for a package) called
irreversible; TUF's root rotation is the mechanism that makes it less so.
