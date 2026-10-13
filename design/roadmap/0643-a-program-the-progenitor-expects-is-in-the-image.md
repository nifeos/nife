---
status: NOT-STARTED
raised: 2026-09-24
promoted_from: a-program-the-progenitor-expects-is-in-the-image
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 643. A program the progenitor expects is in the image, or the build says so

*Title and slug are drafts.*
This was raised by the rename of `audit_sink` to `login_audit_receiver` (#1228). The file's name is provisional;
`design/naming.md` is the rule.

Either option below is a host-side check or a transcript assertion. Neither touches
the syscall surface, a wire format or a dependency.

## The finding

`crates/system_initializer` finds each program it starts by a string:
`measured(&fs, table, "login_audit_receiver")`. Twelve names are looked up this way today:
- `console`, `credentialer`, `entropy`, `fs_subtree_caretaker`, `identity_provisioner`, `input`
- `job_undertaker`, `line_editor`, `login`, `login_audit_receiver`, `swish`,
  `terminal_sink_caretaker`

`measured_boot::verdict` treats an absent name as `elf: None, unvouched: false`. That is the same
answer as "this boot has no filesystem". For the login stack, that answer is deliberate:
`have_login_stack` makes the whole stack optional, so a boot that cannot find `login` still reaches
a prompt.

As a result, a rename that misses the one string in `system_initializer` does not fail anything. The
build passes, because the string is not code the compiler checks. The boot passes and the prompt
comes up. The suites pass too, because the kernel's `login_tests` load programs through their own
`program("login")` lookups, not through the progenitor. The only visible difference is that the
interactive boot stops printing `progenitor: login credentials provisioned`, and nothing reads that
line.

#1228 got this right because the rename checklist in `design/naming.md` says to grep the quoted name. The lane then confirmed it by booting `cargo xtask shell` under a time limit and reading the line by eye. That is rung four of the ladder: a habit, not a mechanism. An unvouched program is reported, and an unvouched name is a different failure from an absent one.

## Options

1. **A host check that every name `system_initializer` looks up is a declared program.** Read the
   string literals passed to `measured(...)` and to `fs.read(...)`, and check that each is a
   `[[bin]]` name in `components/Cargo.toml` or `fixtures/Cargo.toml`. Those manifests are what the
   packing table has been generated from since milestone 150 (adding a program should not need eight
   hand-maintained lists). This is cheap, runs in `script/lint` and needs no emulator, and it would
   have failed #1228 had the string been missed. Its weakness is that it pattern-matches one call
   shape, so a lookup written another way escapes it. That is the `--exclude`-goes-stale shape
   `design/naming/rename-where-names-hide.md` records.
2. **A transcript assertion in `cargo xtask swish-check`.** The check boots with an RNG device and a
   filesystem, which is exactly the configuration in which the login stack must come up. So it can
   require the `login credentials provisioned` line. That catches every cause of a silent skip, not
   only a rename: a program missing from the image, a manifest mismatch, an entropy failure. Its
   weakness is that it costs nothing to run but says less about why the line is missing.
3. Make an absent program loud in `system_initializer` itself. It would print a line naming each
   looked-up program that is absent, alongside the existing unvouched report. That is cheap and
   helps a person at the console, but no gate reads the console, so on its own it is rung three.

Option 1 plus option 2 is the natural pair: one fails fast at the name, the other covers every cause
at boot. This is a recommendation for a reversible fork, and either option alone is an improvement.

## What it would have cost to be wrong

A family boot with no way to log in, discovered by whoever next tried to. No existing gate would
have told anyone before then.

## Index row

The system initializer finds twelve programs by string, and an absent name looks the same as a boot that did not run it. Proposed: a host-side check or a transcript assertion that every program it expects is in the image.
