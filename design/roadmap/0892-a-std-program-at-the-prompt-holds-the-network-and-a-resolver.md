---
status: BUILT
raised: 2026-10-09
built: 2026-10-19
promoted_from: a-std-program-at-the-prompt-holds-the-network-and-a-resolver
milestone_dependencies: 384, 595
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 892. A std program at the prompt holds the network and a name resolver

**BUILT, 2026-10-19 (UTC).** An installed `std` program started at the prompt holds the network
and a resolver badge per zone, because the owner granted them at `jig install`. A package the owner
does not approve is not installed, and bytes vouched for without a grant are refused the network by
name. `script/swish-check` proves both halves on aarch64, riscv64 and
x86_64; the kernel harness proves a name resolved inside a zone, which calef accepted as the proof
(ruling B on #2011, 2026-10-12 UTC). His rulings are under "Item 3, ruled", "The network at
install, ruled" and "Rulings of 2026-10-19".

*(The title and slug are provisional.)*

Milestone 801 (packages over the internet) built a std program that resolves a name through
`std::net` and fetches the package index over TLS.

## The pieces this joins

- The name resolver, which the progenitor starts once the lease arrives (item 1). Milestone 384
  (in a capability system the resolver is a grant)'s BUGS gave that to 801, and 801 to this.
- The network for a `std` program: the stack at slot 2 and the socket frames' budget at slot 3
  (item 2), which milestone 595 (the shell runs a `std` program)'s BUGS left unminted.
- A resolver badge per granted zone, at `std_runtime_contract::RESOLVER_SLOTS` (item 3).

`jig` (milestone 809 (the package client becomes a program)) needs all three to reach
`basalt.nifeos.org` by name, so rung 3c's exit criterion on xenon rests on this.

## The work

1. The progenitor starts `domain_name_resolver` once the lease arrives, told the lease's name server
   (`socket_contract::lease`'s second word), as the test harness does with the runners' peer.
2. The progenitor mints a socket frames' budget for a std program whose manifest declares the
   network, and places it at slot 3. 595's `a_std_program_declares_only_what_the_std_layout_can_hold`
   then admits the network.
3. A manifest names the zone it may resolve; the progenitor grants a fresh badge that zone and
   places it at slot 9. Whether the zone is the manifest's word or the owner's is a fork: a manifest
   can only ask, and §252 (a resolver grant is one zone per client badge) says a spawner grants.
   For `jig` it is ruled: the root zone, recorded where it is granted (calef's Q5 on #1884,
   2026-10-10 (UTC)). For every other program it is ruled too: see "Item 3, ruled".
4. A `script/swish-check` line: an in-tree std program run at the prompt resolves a name in its zone
   and is refused one outside it, on all three architectures.

Reuse: everything named above is in the tree; this is wiring.

## Built 2026-10-12 (UTC), and a premise that was wrong

Lane `milestone/892-a-std-program-at-the-prompt-holds-the-network-and-a-resolver`. "Reuse: this
is wiring" was false. The resolver had eight windows for its life and no way to withdraw a grant
(`domain_name_resolution_contract`'s BUGS), and the progenitor outlives every job it spawns, so wiring
alone would have let the ninth `jig` of a boot be refused.

- The forget verb. `OPERATION_FORGET` (provisional) frees a badge's window; the resolver
  unmaps and deletes the page that badge attached, through its own address space at slot 4, as
  `net_stack` gives up a closed socket's page. Kernel test
  `a_forgotten_badge_is_denied_and_its_window_serves_the_next_client` fills every other window,
  forgets one, and requires the next badge to resolve there; without the `UNMAP` it answers
  `NOT_ATTACHED` (falsification patch in `system_tests/falsifications/`).
- Item 1. The progenitor starts `domain_name_resolver` once the lease arrives, on either stack,
  told the lease's name server over UDP, and says so: `progenitor: name resolver up; asks
  10.0.2.3, the lease's name server`. `script/swish-check` requires that sentence on every boot
  with a network. The resolver's endpoint is one more capability the progenitor holds for life.
- Item 2 was built for `jig` by milestone 809 (the package client becomes a program).
- Item 3, for `jig`. `grant_plan::Program::resolver_zone` (provisional) names `jig`'s zone in the
  program table, not in `Manifest`. Installed bytes ask for zones in their note and hold each on an
  owner's `grant` line (below). §270 (a package manager holds an installer endpoint, not the spawn
  endpoint) extends that to a replacement package manager's root zone. Each `jig` job whose
  verb holds the network gets a fresh badge granted the root zone (Q5), placed at slot 9, and
  forgotten at its reap. `caps jig install greeting@0.1.0` prints the row.
- Item 4. The swish script now starts nine `jig install`s in one boot. With the
  forget at reap removed, the ninth answered "could not spawn" on aarch64 (2026-10-12): the
  lifecycle is gated at the prompt. A name resolved at the prompt is not. Under slirp the lease's
  name server is always slirp's own proxy to the host's resolver, and QEMU refuses a `guestfwd`
  on its DNS address ("Conflicting/invalid host:port", measured 2026-10-12), so the runners' peer
  cannot answer there. calef ruled on #2011 (2026-10-12 UTC, "B") that the kernel harness's
  `a_std_program_resolves_its_granted_zone_and_nothing_without_a_grant`, green on all three
  architectures, is the proof that a program at the prompt resolves a name. No prompt line
  through the host's DNS and no name-server setting are owed.

## Item 3, ruled 2026-10-12 (UTC)

calef, on #2011: "yes, record c". For every program other than `jig`, the manifest asks for its
resolver zones. The owner grants them at `jig install`, in the same confirmation as the program's
other grants (§208 (installing a package is granting it, and the installed programs are
versioned)). The owner can narrow them later. The details he accepted:

- A manifest may list several zones. Each granted zone is its own badge, per §252 (a resolver
  grant is one zone per client badge).
- A root-zone ask (any name, as a browser needs) is flagged prominently at install, not listed like
  the others.
- A program that asks for nothing gets no resolver.

All four pieces are built: the zones in the note, the granted zones in the table, a badge per
zone, and the line at install. "Built 2026-10-19" has each.

## The network at install, ruled 2026-10-12 (UTC)

The ruling above left a gap: an installed `std` program could not hold the network, so a zone did
nothing for one. calef was asked whether a manifest declares network use beside its zones, shown at
install before the owner approves (a), or whether the network stays out of manifests and is granted
by hand after install (b). He answered "a". The reason he accepted: what a program may reach
differs by program, since a weather app that calls one API is not a browser. The network is the
same kind of grant as a zone, so it belongs in the same declaration and the same confirmation.

The question as put was loose on two points. The ruling reads as follows with both fixed:

- The zones are resolver zones, the names a program may look up. They are not filesystem zones;
  directories are a separate grant under §208 (installing a package is granting it, and the
  installed programs are versioned), and milestone 911 (scripted installs grant only what the owner's
  approval file allows) lists them apart.
- A manifest can already declare the network. `Manifest::network` is one flag (byte 34 of
  `capability_manifest_note`), and a program in the image table gets it today. What is missing is the install
  path, built 2026-10-19 under "Built 2026-10-19".

So "network use" means the existing grant: a client view of the network stack carrying `WRITE`,
at `NETWORK_SLOT` (slot 10) for a native program and slot 2 for a `std` one. A `std` program also
gets the socket frames' budget at slot 3, as item 2 mints it for `jig`. It connects out and cannot
listen (`socket_contract::NO_LISTEN_GRANT`).

All three are built with item 3's: the network is endowed only on a recorded grant, recorded in
the same grant lines as the zones, and listed in the one confirmation.

The grant stays one flag, ruled 2026-10-12 (UTC). Once the network grant was found to bound no
addresses, calef was asked whether it should be limited per destination now (b), or kept as one flag
with per-destination limits recorded as a gap (a). He answered "a". The reasons he accepted: Flatpak's
`--share=network` and Android's `INTERNET` permission are all or nothing too (recalled, not read
today), and a per-destination limit comes from a filter layer such as Little Snitch on macOS. Zones
already bound which names a program can look up, and a filter in the network stack is a milestone of
its own. (a) can be undone; (b) would have committed to building that filter now.

## Built 2026-10-19 (UTC)

Lane `milestone/892-installed-programs-hold-the-network-and-zones` (#2405).

- **The ask.** `capability_manifest_note` version 2: version 1's 56 bytes, then one to eight zones,
  each a length byte and dotted ASCII. A note with no zone stays version 1, so no existing digest
  changes. Zones are spelled as the resolver spells them
  (`domain_name_resolution_contract::is_canonical_zone`), sorted, each once; the root zone is empty.
  A program carries them with `carry!(MANIFEST, zones = &["nife.test"])`.
- **The confirmation.** `jig install` lists each program's network and zone asks, the root zone
  apart in capitals, and asks once; only `y` or `yes` sends the package. `jig` reads the answer as
  standard input: the boot terminal badged `line_editor::proto::STDIN_BADGE`, at
  `std_runtime_contract::STDIN_SLOT` (29), served `OPERATION_READ_LINE_BYTES` (eight bytes of the
  typed line a reply) and nothing else. It is placed only for `Manifest::console_input`, which
  rides with the installer authority in `jig`'s verb table. No note can spell it yet; §270 lets an
  owner's `grant` line give it to installed bytes, and that half is unbuilt. The spawn
  service holds that copy while it builds the job, which is the progenitor's capability peak:
  `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED`, measured with and without it.

  ```text
  $ jig install ./reach.nifepkg
    reach asks for:
      reaches the network (connects out)
      looks up names in nife.test
    grant these and install? [y/N] y
    installed; generation … is live
  ```

- **The record.** The installer writes `grant <digest> network` and `grant <digest> zone <name>`
  lines after the pointers, exactly each program's asks; the root zone is `.` (provisional).
  `installed_programs` reads them strictly: a grant for no row, out of order, repeated or misspelled
  makes the table unreadable. A removal drops its rows' grants.
- **The endowment.** `grant_plan::image_manifest` takes a required `Vouch`. Vouched bytes whose note
  asks for the network get it only on a `grant` line; without one they are refused by name
  (`ImageRefusal::NetworkNotGranted`), native or `std`, and an owner's `vouch`, which records no
  grant, is refused the same way. Unvouched bytes still run granted nothing (§219 (how the shell
  names an installed program to the spawner)). A granted image gets one resolver badge per zone its
  grants give and its note asks, at `std_runtime_contract::RESOLVER_SLOTS` (9, then 22 to 28),
  forgotten at the reap. std's `lookup_host` asks them in order and moves on only on `DENIED`.
- **The proof.** `script/swish-check` answers `n` for `dialer` (nothing is installed) and `y` for
  `reach`, a `std` program (`std_exerciser/src/bin/std_reach.rs`, provisional), which then echoes
  through the runner's peer and reads `PermissionDenied` for `example.com`, outside its zone. Kernel
  tests: `a_std_program_asks_each_badge_in_order_until_one_holds_the_name` and
  `a_copy_badged_as_stdin_reads_the_typed_line_and_nothing_else`. Host tests in
  `capability_manifest_note`, `installed_programs` and `grant_plan` cover the encodings.

## Rulings of 2026-10-19 (UTC), on #2405

calef took the recommendation each time.

- Zones in the note ("approve version 2?" "Yes"): descriptor version 2.
- The grant record ("a"): `grant` lines keyed on the digest. Approval is all or nothing; the
  grants are exactly the asks.
- Several zones ("(a). First zone at slot 9, the rest at fixed slots above 12, probed in order."):
  slots 22 to 28, the first seven free in a row, since `jig` holds 15 to 18 and the shell's
  configuration is 21.
- The owner's yes ("a"): a byte-source contract gives a std program stdin, granted to `jig` at
  the owner's console, with no `--yes` and no second verb.
- Its shape ("a"): the cooked read on a badged terminal copy at slot 29. The options are in
  [the appendix](./0892-a-std-program-at-the-prompt-holds-the-network-and-a-resolver/stdin-contract.md).
- The maintainer's sequencing call: 892 builds the install confirmation, and milestone 910 adds
  its service line to it.

## BUGS

- Every badge is one of the resolver's eight windows, shared by every job on the machine, so one
  program granted eight zones leaves none for anyone else while it runs.
- A program whose note asks for zones but not the network runs without a resolver; nothing refuses
  it by name.
- The shell hashes a file whose note asks for the network before each run, to name the missing
  grant, so such a run reads the file twice.
- No verb narrows a grant after install. The format allows it (a generation without the line), and
  milestone 911 (scripted installs grant only what the owner's approval file allows) owns revoking.
- A stdin read that a terminal quiesce interrupts reads end of input, and the rest of a line a
  program stopped reading stays for the next reader of the badge.
- No program at the prompt resolves a name yet. `jig` holds the badge and fetches by a fixed
  address until milestone 809's item 6.
- A screen-narrowed job gets no resolver. The shell, not `job_undertaker`, reaps it, so the
  spawn service would never hear to forget its badge.
- The spawn service waits on the resolver. A grant or a forget is a `SEND` the resolver takes
  between requests, so a resolve nobody answers (three 15-second waits) delays the next spawn.
- At most eight resolving jobs at once; a ninth live one is refused rather than queued.
- The network grant is one flag and bounds no addresses. A program holding it can connect to any
  address it knows; its zones bound which names it can turn into addresses, not where it connects.
  A weather app and a browser differ in their zones, not in their network grant. This is the gap
  calef ruled to keep for now; a per-destination filter is unowned.

## Follow-on

- **Milestone 911.** Its approval file's "the network's socket budget" is this grant, and its grant
  record is this block's `grant` lines.
- **Milestone 910.** Milestone 910 (a package declares a service) adds its service line to
  `jig install`'s confirmation.
- **Milestone 809.** `jig` resolves the index's host through its badge at item 6.

## Index row

A `std` program installed with `jig` and started at the prompt holds the network and a name
resolver, because the owner granted them at install; one not granted is refused by name.
