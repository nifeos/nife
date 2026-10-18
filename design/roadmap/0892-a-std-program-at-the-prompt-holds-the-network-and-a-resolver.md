---
status: PARTIAL
raised: 2026-10-09
promoted_from: a-std-program-at-the-prompt-holds-the-network-and-a-resolver
milestone_dependencies: 384, 595
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 892. A std program at the prompt holds the network and a name resolver

**PARTIAL, 2026-10-12 (UTC).** Items 1 and 3 are built for `jig`, and a forget verb the block did
not foresee is built under them. Item 4 is met: its positive and refusal halves are proved in the
kernel harness, which calef accepted as the proof (ruling B on #2011, 2026-10-12 UTC). He approved
the forget verb as written (Q2, "Approve", the same day), and ruled item 3's fork, also the same
day ("yes, record c"). Then he ruled that a manifest declares the network beside its zones and
the owner grants both at install ("a", the same day). What is left is building both for installed
programs, under "Item 3, ruled" and "The network at install, ruled" below.

*(The title and slug are provisional.)*

Milestone 801 (packages over the internet) built a std program that resolves a name through
`std::net` and fetches the package index over TLS. Only the kernel test harness can start it.

## The gap

Three pieces exist and are not joined at the prompt.

- The name resolver runs only when a test starts it. Milestone 384 (in a capability system the
  resolver is a grant)'s BUGS gave starting it from the lease to 801. 801 built the client half
  and left this, because nothing at the prompt could use it.
- A std program cannot hold the network at the prompt. Milestone 595 (the shell runs a `std`
  program)'s BUGS: the progenitor does not mint the socket frames' budget slot 3 needs, so
  `grant_plan` refuses a std manifest that declares the network.
- A std program's resolver is a badge at `std_runtime_contract::RESOLVER_SLOT` (slot 9,
  provisional), placed today only by `system_tests`' `std_service`.

`jig` (milestone 809 (the package client becomes a program)) needs all three to reach
`basalt.nifeos.org` by name, so rung 3c's exit criterion on xenon waits on this.

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
  with a network. One more resting capability slot: `CAPABILITY_TABLE_PEAK_MEASURED` is 34.
- Item 2 was built for `jig` by milestone 809 (the package client becomes a program).
- Item 3, for `jig`. `grant_plan::Program::resolver_zone` (provisional) names the zone in the
  program table, not in `Manifest`, so installed bytes can never carry one. Each `jig` job whose
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

What that leaves to build:

1. The zones in a program's manifest note, the ask. `capability_manifest_note` is a format two programs agree
   on, so the encoding is decided once.
2. The granted zones in the installed-programs table's entry, which is where an owner's narrowing lands too.
3. The progenitor grants one badge per granted zone at spawn and forgets each at the reap. The std
   PAL holds one resolver slot today, so a program with several zones needs a slot per badge, or
   a PAL that picks the badge whose zone holds the name.
4. The line at install: each zone listed, and a root-zone ask flagged apart. One confirmation
   covers it with the other grants. No milestone yet builds `jig install`'s listing of what it
   grants. The services milestone proposed on #2022 (a package declares a service) adds its
   "runs as a service" line to the same listing, and milestone 809 (the package client becomes a program)
   owns `jig install`. The listing belongs in one of them, and this item waits on it.

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
  path: `grant_plan::image_can_carry` refuses installed bytes whose note sets it, and nothing at
  install lists it.

So "network use" means the existing grant: a client view of the network stack carrying `WRITE`,
at `NETWORK_SLOT` (slot 10) for a native program and slot 2 for a `std` one. A `std` program also
gets the socket frames' budget at slot 3, as item 2 mints it for `jig`. It connects out and cannot
listen (`socket_contract::NO_LISTEN_GRANT`).

What that leaves to build, beside item 3's four pieces and in the same places:

1. The progenitor endows the network to an installed program only when its installed-programs entry
   records the owner's grant. `image_can_carry` keeps refusing bytes with no such entry.
2. The granted network in the installed-programs table's entry, in the same grant record as the zones, so an
   owner's narrowing and 911's approval file reach it the same way.
3. The line at install, "reaches the network (connects out)", in the one confirmation with the
   zones. Milestone 910 (a package declares a service) builds that confirmation for every grant a
   manifest asks for, so this is a line in it rather than a listing of its own.

The grant stays one flag, ruled 2026-10-12 (UTC). Once the network grant was found to bound no
addresses, calef was asked whether it should be limited per destination now (b), or kept as one flag
with per-destination limits recorded as a gap (a). He answered "a". The reasons he accepted: Flatpak's
`--share=network` and Android's `INTERNET` permission are all or nothing too (recalled, not read
today), and a per-destination limit comes from a filter layer such as Little Snitch on macOS. Zones
already bound which names a program can look up, and a filter in the network stack is a milestone of
its own. (a) can be undone; (b) would have committed to building that filter now.

## BUGS

- Item 3 is built for `jig` only; "Item 3, ruled" says what is left.
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

- **Outstanding.** Item 3 for installed programs: the four pieces under "Item 3, ruled".
- **Outstanding.** The network at install: the three pieces under "The network at install, ruled".
- **Milestone 911.** Its approval file's "the network's socket budget" is this grant, and now cites
  this block.
- **Milestone 809.** `jig` resolves the index's host through this badge at item 6.

## Index row

A `std` program started at the prompt can hold the network and a name resolver. Today the pieces
exist but join only when the kernel test harness starts them, so a package client cannot run from
the shell.
