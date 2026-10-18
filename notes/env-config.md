# The inert-configuration page: `TZ`, `LANG`, `TERM`, and what still waits

Milestone 47's environment-variable fork. Built 2026-08-23. The contract is `environment_contract` (a
provisional name; naming crates is an architect's call, per AGENTS.md); the decision is DECISIONS
§111 (inert configuration is a read-only page).

## What this is

Milestone 47 splits what Unix puts in one string-to-string environment map into three parts. They
are inert configuration (`TZ`, `LANG`, `TERM`, genuinely just data, no authority in it) and names
(`PATH`, `HOME`, directory capabilities wearing a string costume). Then secrets (authority badly
encoded as a bearer string). Only the first third is built here. Names wait on `bind` (milestone
154's two-directory endowment, not yet built); secrets are answered elsewhere, by an endpoint
(§41), and are never meant to arrive as a string on this table at all.

The wire encoding is a read-only page, the same rights-ladder shape the clock page uses. No
capability, or a `PageFrame` with `READ`. A config value is never something a person designates on a
command line. There is nothing to propose or set from inside the process that holds it, so an
endpoint (as the clock's propose half uses) would be the wrong shape here.

Each declared key is validated against a closed domain before it is ever written to the page.
`TZ` must be a real IANA-style timezone identifier, `LANG` a real locale identifier, `TERM` a real
terminal type. A value that does not parse as a member of its key's domain is refused when the page
is assembled, not carried through disguised as configuration. This closes the gap DECISIONS §111
identifies: a capability governs reach, not meaning, so once a value is bytes on a page nothing
about the capability model can tell a password from a timezone. `environment_contract::domain`'s three lists
are curated and real, not the full IANA/glibc/terminfo databases; see the crate's own `BUGS`
section for why growing them on demand is deliberate rather than a gap.

## Why no seqlock, unlike the clock page

The clock page is published to repeatedly by a running service while readers hold a stale mapping,
so a reader can catch a writer mid-publish and must retry rather than blend. The config page has
exactly one writer, and it finishes before the page has a second reader: it is assembled in a
buffer nothing else can see, and only *then* mapped read-only into the process that will read it.
There is nothing to race, so `environment_contract::ConfigPage` is a plain length-prefixed byte layout with no
atomics at all.

## What's built end to end

- `crates/environment_contract`: the page layout, the three validated domains, `PageBuilder` (assemble,
  validated) and `ConfigPage` (read). Host-tested: round trips, refusals, an unrecognized magic
  or a zeroed frame reading as "no configuration" (DECISIONS §42's rule, applied here), every
  domain member fitting its field's byte cap.
- `system_tests/src/user/std_service.rs`: assembles a page (`TZ=UTC`, `LANG=C`, `TERM=dumb`, the
  conservative universal defaults). They are chosen because nothing holds a *different* default to
  pass explicitly: the shell exists, but the "a shell holds its own default" mechanism does not
  (this note's own second BUGS entry). The page is mapped read-only into a std program at
  `CONFIG_PAGE_STD` (`0x1300_0000`), granting a `PageFrame` with `READ` at `CONFIG_SLOT` (7), the same
  shape as the
  clock's slot 5.
- `patches/std-nife/overlay/std/src/sys/pal/nife/rt.rs`: `CONFIG_SLOT`/`CONFIG_PAGE` constants,
  the std PAL's twin of the kernel-side ones.
- `patches/std-nife/overlay/std/src/sys/pal/nife/mod.rs`: the `envproto` module (generated
  verbatim from `crates/environment_contract` by `cargo xtask std-src`, the same mechanism `clockproto` and
  `fsproto` use) and the call into `sys::env::seed()` from `pal::nife::init`, before `main` runs.
- `patches/std-nife/overlay/std/src/sys/env/nife.rs`: `seed()`, which probes `CONFIG_SLOT` the
  same way `sys/time/nife.rs` probes `CLOCK_SLOT` (an invocation with a method number no object
  defines, so an empty slot and an occupied one answer differently), and pushes whatever keys the
  page carries into `std::env`'s table.
- `std_exerciser`: asserts `TZ`/`LANG`/`TERM` read back exactly what the kernel wrote, before
  `std::env::vars().count()` is asserted (now 3, not 0: the count changed from "always empty" to
  "exactly what was granted"). `kernel::user::std_tests::a_whole_std_program_runs_on_the_native_abi`
  pins the whole transcript on both ISAs.

## What this does not do, and why

- Corrected 2026-09-26: a shipped program does declare this page. This bullet said no
  `Manifest` field existed; `grant_plan::Manifest::configuration` and `printenv` landed 2026-08-26 and
  the real progenitor endows the page to any child that declares it (milestone 47 (navigation and naming)'s
  `design/roadmap/0047-navigation-and-naming/environment.md`). `date` still does not read `TZ`, for the
  reason `notes/calendar.md` gives.
- The `caps` preview extension DECISIONS §111 (inert configuration is a validated page) asks for is built (2026-09-26). The progenitor
  places the frame it endows children with in the boot shell too, `READ` without `GRANT`, at
  `grant_plan::SHELL_CONFIGURATION_SLOT` (21) and maps it at `grant_plan::SHELL_CONFIGURATION_VIRTUAL_ADDRESS` (both names
  ratified, calef, 2026-10-18). `caps printenv` then prints the three values from that frame, so the preview is
  what the child will read rather than a copy of the defaults that could drift:

  ```text
  $ caps printenv
    printenv would grant the new process, and nothing else:
      ...
      cap 1  frame     config   read-only, the page this shell reads too:
                                TZ=UTC
                                LANG=C
                                TERM=dumb
  ```

  Bare `caps` lists the shell's own view as one more `READ only, NOT delegable` row.
- `PATH` and `HOME` are not seeded here. Both are namespace questions, directory capabilities
  rather than variable ones. The roadmap's own text is explicit that they wait on `bind`.

## Tests

`crates/environment_contract` host suite: 9 unit tests plus 2 doctests. They cover the round trip,
every domain check, and an undeclared key reading as absent rather than empty. Then a zeroed frame
and an unrecognized
magic both reading as "no configuration," and the layout offsets not overlapping.
`kernel::user::std_tests::a_whole_std_program_runs_on_the_native_abi`, both ISAs: the real
`std_exerciser` binary, granted a real config page assembled by the real kernel wiring, reads
`TZ`/`LANG`/`TERM` back through the real std PAL and the real `std::env` API.

## BUGS

- No wire announcement on the shell's spawn protocol, and none is needed: the progenitor
  reads `Manifest::configuration` from the program's own declaration, as it reads `clock`.
- The shell previews the progenitor's one default; it cannot choose another. The roadmap's
  "inheritance with visibility" shape has two halves. The visible half is built: `caps` shows the
  values a child will read. The chosen half is not: a shell cannot hand a child a different set
  (`TZ=Europe/Paris printenv`), because the page is the progenitor's and one frame serves every
  child. Building it means a per-spawn page and a way for the shell to say what goes on it, a
  change to `spawnproto` that two programs agree on, and nothing needs it yet.
- A `login` session's shell cannot see the values. `login` holds no configuration page, so the
  shells it builds find `SHELL_CONFIGURATION_SLOT` empty and `caps printenv` says it "cannot show their
  values". Only the boot prompt's shell holds the view. Children spawned from a session are still
  endowed normally; only the preview is blind.
- `caps <path>` for an unvouched image prints the config row without values
  (`swish::write_image_capabilities`, milestone 198 (a package manager)'s), because that renderer does not take the page. A
  small follow-up in 198's file once its lane has landed.
- The domains are curated, not exhaustive. See `environment_contract`'s own `BUGS` section.
