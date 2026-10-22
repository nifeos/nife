---
status: NOT-STARTED
raised: 2026-10-06
promoted_from: a-package-declares-what-it-needs-at-run-time
milestone_dependencies: 597, 611
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 849. A package declares what it needs at run time, apart from what it links

*(Minted 2026-10-09 (UTC) by lane/promote-proposals from its proposal; number provisional until the merge queue lands it, title and slug drafts.)*


calef asked on 2026-10-06 (UTC) how a nife package says what it needs at run time, as distinct from
what it links. Written by lane `package-runtime-requirements`, which built nothing but this file.

Reuse: the per-program half reuses `grant_plan::Manifest` and `crates/capability_manifest_note` as they
stand, and the gate extends `helpers/packages.py`. No outside tool fits, for the reasons in
[the appendix](0849-a-package-declares-what-it-needs-at-run-time/prior-art.md).

## The gap

A package has one relationship to another today: linking.

- `depends` is checked only for link edges across a package boundary. `helpers/packages.py:410`
  allows a link if the target crate is an `interface` or its package is in `depends`, and nothing
  else reads `depends` except the kind rules below it. `script/lint:3298-3313` runs it.
- So a client of a service never declares the service. `system_log`'s clients link
  `system_log_contract`, which `contracts` exports as an interface, so calef's ruling on #1796 (fork
  1, 2026-10-07 UTC) added no `depends` edge to `system-log` anywhere. That is right for linking. It
  also means no record anywhere says "this program needs a log service running".

What a program needs at run time is declared, but per program and in code, never per package:

- `grant_plan::Manifest` (`crates/grant_plan/src/lib.rs:1731`) has a field per service a program may
  be endowed with: `clock`, `domain`, `config`, `entropy` (`:1836`), `network` (`:1857`), `machine`
  and `share`. Its `runtime` field (`:1888`) says whether capabilities land in native order or at
  `std`'s fixed slots. An archive program's manifest is a `match` arm. A foreign program's travels
  as an ELF note (milestone 597 (a program carries its manifest in an ELF note), `crates/capability_manifest_note`).
- Who provides each service is a string at a call site. `crates/system_initializer/src/lib.rs:1168`
  looks up `"system_log"`, `:1190` `"entropy"`, `:1194` `"net_stack"` and `:1244` `"redoxfs_server"`.
  Each is optional by design: a missing provider costs a feature, and a declaring child gets an empty
  slot (`:3550`, `:3559`) and says so.
- The log service has no client side at all yet. `:1728` records that nothing registers a log writer,
  so there is no manifest field for it either.
- `capability_plan::Requirements` (`crates/capability_plan/src/lib.rs:206`) is the closest thing to
  a provider declaration in the tree: a `contract`, capabilities tagged `Serve` or `Use` (`:216`),
  and `depends_on` (`:396`) naming contracts. Only milestone 23 (a capability-routed component OS
  with live replacement)'s swappable components use it. The boot's own services are built from
  literals in `system_initializer`.

Nothing joins these. No gate knows that `uuid` (in `core-tools`) declares `entropy`, that the
`entropy` package provides it, or that an image without the second still ships the first.

### Ripgrep, verified

`rg` is unmodified ripgrep 14.1.1 from crates.io. It is in no manifest and no recipe.

- Built by `helpers/build-ripgrep.sh`, which nothing in `script/test` or CI runs, for all three
  targets with `-Zbuild-std`, into `target/ripgrep/<triple>/rg`.
- Packed by `xtask` iff that file exists: `xtask/src/farm.rs:71` names the path, and
  `xtask/src/archive.rs:653` (aarch64), `:354` (riscv64) and `:525` (x86_64) push it as `rg`. It is
  one of six names in a hand list, `BUILT_ELSEWHERE` (`:236`).
- No package file can name it: `helpers/packages.py:325` admits only in-tree binaries.
- It carries no manifest note (`helpers/build-ripgrep.sh:67-69`).

What it is handed, and by whom: the only thing that runs it is the kernel's test harness, not
the progenitor. `system_tests/src/user/ripgrep_tests.rs:71` calls `fs_service::start_std_full`, which
starts a RedoxFS server if none is running and spawns `rg` with three capabilities
(`fs_service/std_start.rs`):

| Slot | What | Why `rg` needs it |
|---|---|---|
| 0 | a memory region, its heap | `std`'s allocator grows from it |
| 1 | `WRITE` on a rendezvous | stdout and stderr |
| 4 | the file server's endpoint, as a directory | `std::env::current_dir()` and every `std::fs` call |

Without slot 4 the same binary prints `failed to get current working directory`. These needs are
written down only in that harness call.

From the prompt it would be worse. With no note, `grant_plan::image_manifest` (`:1477`) falls back
to `NO_NOTE_MANIFEST` (`:1397`), `uptime`'s native layout, so a `std` binary would take its output
endpoint for an allocator (read from the code, not run). The note `rg` needs is three fields: `runtime = Std`,
`arg = Words(ReadOnly)` and `output = Bytes`. `UNVOUCHED_STD_MANIFEST` (`:1574`) is already that
shape, and a word such as `.` then designates the directory, as §170 (how a foreign program is told what to do) rules.

## What a runtime need is here

A need is a capability, not a process: a program needs a `WRITE` endpoint that answers
`entropy_contract`, not "the entropy package". And absence is supported (every provider in
`system_initializer` is optional), so most needs are wants in systemd's sense, not requirements.

## Options and prior art

Options A to F are in the
[appendix](0849-a-package-declares-what-it-needs-at-run-time/prior-art.md):

- B duplicates the manifest, so a gate can prove two hand lists agree and never that they agree with
  the code.
- A rots the first time a program gains a field. C does not apply: ordering is readiness, a
  provider answering `READY` before its endpoint is handed out.
- F fixes the image at build time, and §235 (the OS is built and updated from packages) and `jig`
  install onto a running system.
- D and E fit together. nife has Fuchsia's `use` (the manifest) and an implicit `offer` (the
  progenitor endows each service it built to each child that declared it). What is missing is
  `expose`, and a check that a route exists.

## Recommendation

Per fork 1, needs live only in program manifests and the build derives the rest.

1. Use: `grant_plan::Manifest`, carried as in fork A. Add `Manifest::contracts()`, mapping each
   service field to a contract identifier. Write it as a destructure with no `..`, so a new field is
   a compile error until it is mapped (rung 1). A manifest also names its runner (fork B).
2. Expose: a `provides` table in a package file, covering contracts and runners (fork 2).
   `packages.py` checks each named program is a member, the `subtree_grants` precedent
   (`helpers/packages.py:449-458`).
3. Route check: `xtask` already links `grant_plan` and checks every archive it packs
   (`check_declared_programs`, `xtask/src/archive.rs:265`). It emits each package's derived needs,
   and `packages.py` applies the kind rules `depends` already has (fork 3).
4. Install: the derived needs go into the package metadata and the index, per fork 1.
5. Versions: levels, the gates and the badge field, as the versioning appendix lists.

`depends` stays link-only. Counted cost: nine fields to map, a `provides` table in about six
packages, one function in `grant_plan`, one emitter in `xtask`, one rule in `packages.py`. B loses on
drift, not effort.

The gate would fire on day one: `rm` (`base`) needs a file server, and `redoxfs` is `optional`.

System log: `system-log` provides the contract `system_log`. Its first client adds a manifest field
(`log`) and links only `contracts`.

Ripgrep: its build links a note declaring `runtime = Std`, `arg = Words(ReadOnly)` and
`output = Bytes`, by the mechanism #1319 measured (`-Clink-arg=note.o`). The words grant derives the
`filesystem` contract, slot 4 above. Fork 4 puts that build in basalt.

## Every kind of language

calef, 2026-10-07 (UTC): *"What about dynamic languages, non-C languages, go, java? I'm trying to
make certain that we can package software comprehensively."* Here, only where each kind's manifest
lives and who runs it. Porting each runtime is per-workload milestone work.

| Kind | Examples | What the kernel runs | Where the manifest lives | In the tree today |
|---|---|---|---|---|
| Native compiled | Rust, C, C++, Zig, Swift, Go | the program's own ELF | an ELF note | Rust `std` (`patches/std-nife`); C behind a Rust shim |
| Interpreted | Python, JavaScript, Ruby, Lua, shell | the interpreter | a comment block after `#!` (fork A) | none |
| Bytecode VM | Java, .NET, WebAssembly | the VM | a custom section, or an entry under `META-INF/` (fork A) | none |

Native compiled. Each language needs its runtime ported, and the shape differs. C runs today only
as §31 (the foreign-language seam) builds it: `fixtures/c/c_seam.c` is linked into `c_shim`, a Rust
shell that holds every capability, and the C makes no syscalls. Full POSIX, §31's tier three, is
unbuilt. C++, Zig and Swift need a libc or their own runtime ported; nothing has started. Go is
different: its runtime makes system calls itself per `GOOS` rather than through a libc (recalled),
so Go needs a `GOOS=nife` port. `git grep GOOS` finds nothing. §83 (take the Rust one) still
applies, so a C path is for software with no Rust equivalent.

Interpreted. The interpreter is the ELF the progenitor builds, shared by every script. Its note
cannot carry a script's needs, because it would have to be the union of every script's, which is
ambient authority. So the manifest belongs to the script, and the grants attach to the
interpreter-plus-script pair: the interpreter's own needs plus what the script declares. That closes
§219 (how the shell names an installed program to the spawner)'s first recorded limitation, that a
vouched interpreter runs any script with all of its own authority (`design/decisions/0219-naming-an-installed-program-to-the-spawner.md`, "Recorded limitations").
§219 already names the shape: `interp build.nsh`, with the script as a read-only file grant (`:129`).

Bytecode VMs. The same as interpreters, one level down. The JVM and .NET assume threads, which
§105 (`std::thread::spawn` stays declined) declines for now, so they wait on that ruling as well.
WebAssembly is the cheap route. WASI's preopens hand a module its directories at start, which is
this system's directory capability already. §84 (how we port) records that alignment and is careful
that it is "of design shape, not of ABI". One WASI runtime ported as a nife program would run every
language that targets `wasm32-wasip1` behind one runner. That is a candidate milestone, not a
promise.

## Forks

Each is reversible while nothing outside the tree reads these files. A "no" leaves the gap as it is
and breaks nothing that runs.

1. Where a need is declared. Ruled by calef, 2026-10-07 (UTC): only in program manifests. The build
   computes them into package metadata (§197 (a package is one archive file)) and the index (§250 (an image names its
   distribution's package index)), where `jig` resolves them before download.

A. Where a manifest travels. Revised by calef, 2026-10-07 (UTC), on his question *"Could a script's
   manifest be part of the start of the script much like #! is used to identify the scripting
   language/executor?"*, answered *"Yes, that seems more elegant. Obviously jig would need to evolve
   as we expand language support."* The proposal now reads: a program carries its manifest inside
   itself, in its own format's metadata slot.

   | Format | Slot | Encoding |
   |---|---|---|
   | ELF | the note, milestone 597 (a program carries its manifest in an ELF note) | the note's binary descriptor |
   | Script | the leading comment lines, after an optional `#!` | the text form below |
   | WebAssembly | a custom section named `nife.manifest` | the note's binary descriptor |
   | Java jar | the entry `META-INF/nife/manifest` | the text form |
   | A format with no slot | a sidecar file, as the last resort | the text form |

   The marker. The block sits in the script's leading comment lines, after an optional `#!`, so a
   Python encoding line can keep line 1 or 2; the reader finds it by its marker. It opens with the
   script's comment prefix, a space, `/// nife`,
   and closes with the prefix, a space and `///`. Every line between starts with the same prefix.
   The reader takes the prefix from the opening line, so `#`, `//` and `--` all work. This is PEP
   723's block shape with its own type word, so a Python script can carry `# /// script` for its
   packages and `# /// nife` for its grants side by side:

   ```python
   #!/usr/bin/env python3
   # /// nife
   # runner = python
   # entropy = true
   # ///
   ```

   The encoding. A text form inside text formats, the binary descriptor inside binary ones, and both
   decode to one `grant_plan::Manifest`. The text form is a strict subset of TOML: one `key = value`
   per line, keys from `Manifest`'s fields, values bare words, integers or booleans, and no tables or
   quoted strings. That keeps the reader the progenitor needs small enough to fuzz, while any TOML
   parser still reads it. A round-trip test (text, `Manifest`, descriptor, `Manifest`) keeps the two
   encodings equal.

   The consequence. One digest covers code and manifest together, for a script as for an ELF, so
   the earlier draft's separate manifest file and its own entry in the installed-programs table go away. The
   cost. `jig` and the spawner each need one small reader per format, so `jig` grows as language
   support grows. That belongs to milestone 809 (the package client becomes a program).

   Prior art is in the appendix. Refused: a sidecar for every format, which separates the manifest
   from the bytes §197's option M2 hashes, and a stub ELF per script.

   Ruled by calef, 2026-10-07 (UTC), as written at c40c633c7, marker and encoding included: *"Yes"*.

B. How a program names what runs it. Options:
   - `#!` and a path the shell resolves, Unix's and Linux `binfmt_misc`'s way (recalled). It names
     a path, which is ambient, and the package system cannot see it.
   - Bundle the interpreter into each package. Every package then ships its own copy.
   - `runner` as a kind of need. A manifest names a runner (`elf`, `python`), and an interpreter's
     package provides it. Fuchsia does this: `program: { runner: "elf", binary: "bin/example" }`,
     the ELF runner built in, and a component able to serve a runner capability
     ([runners](https://fuchsia.dev/fuchsia-src/concepts/components/v2/capabilities/runner)).

   Ruled by calef, 2026-10-07 (UTC), as a trial: *"Lets try it. I think we need to use it to figure
   out the usability challenges."* `runner` is a hard need declared in the block, with `elf`
   implicit for native programs and provided by `init`. nife never reads `#!`, which stays for other
   systems. A script with `#!` and no block is refused. `jig` may offer to write a block, and never
   infers one silently.

2. How a provider is declared. Ruled by calef, 2026-10-07 (UTC): *"ratify #1797 Fork 2."* One
   `provides` key covers contracts and runners, each naming the member program that serves it, and
   a gate checks it:

   ```toml
   [provides]
   contracts = { entropy = "entropy" }
   runners = { python = "python3" }
   ```

   A contract is named by an identifier, and the Rust crate (`entropy_contract`) is one binding of
   it, so a Python or WASI client names the same contract.
3. How hard an unmet need fails. Ruled by calef, 2026-10-07 (UTC), reworded: the gate requires at
   least one `base` provider for every contract a `base` program needs, rather than naming a
   package. `jig` resolves or refuses a missing runner, and installs or warns on a missing
   contract. `redoxfs` becomes `base` as today's chosen provider of the filesystem contract, being
   the only writable one. His words: *"Yes begrudingly. I suspect btrfs is likely a better FS choice
   than redoxfs, but it is what we have today."*
4. Ripgrep. Ruled by calef, 2026-10-07 (UTC), amended: *"Should we have a milestone for packaging
   ripgrep and a milestone for a functional ripgrep, which seems like what the ripgrep milestone
   should be."* then *"Yes"*, and later *"To be clear, we should package ripgrep in basalt. That's
   what it is for."* So `ripgrep` is kind `optional`, packaged in basalt (§151 (the goal of the
   repository split is independent release and third-party programs)) from a recipe naming the
   crate, the version and a checksum, with its note written at build time. And: *"With a
   functional jig, ripgrep should be installed via jig and not part of the base image. It isn't
   base."* It is a customer-facing optional package that any customer installs with `jig install`;
   only baking it into the base image is excluded. It depends on milestone 809 for that. Milestone 810 (ripgrep is packaged in basalt) holds the
   work; milestone 121 (`ripgrep` on nife) depends on it only for `rg` at the prompt. The earlier
   draft's `[[foreign]]` key is withdrawn.
5. Versioning, runner versions included. Ruled by calef, 2026-10-19 (UTC), nine rules as
   [the versioning appendix](0849-a-package-declares-what-it-needs-at-run-time/versioning.md)
   states them. A contract carries a level. A provider declares the levels it serves, and a
   consumer the one the build stamped. A runner keeps its upstream version, and a script states a
   range. The progenitor checks every generation it commits and every spawn.

Every fork is ruled. Names still provisional: `level`, `contracts`, `runners`, `runner`,
`Manifest::contracts`, the manifest field `log` and contract identifiers.

## BUGS

- Needs between boot services, such as `system_log`'s `WRITE` on the console, stay literals in
  `system_initializer`. Only `capability_plan` components declare those.
- A contract names a kind of service, not an instance. Two file servers would both provide
  `filesystem`, and the progenitor still chooses.
- Prior art marked "recalled" was not reread for this file.
- An open usability risk, in calef's words: *"One of the powers of a scripting language is that it
  doesn't take a build step to get running and I worry we're creating just that."* The trial should
  test one distinction. An installed program comes through `jig`, needs its block, and its grant is
  reviewed. An ad hoc script typed at the shell (`python foo.py` in swish) could run with a subset
  of what the shell itself holds and need no block. Prior art for that path is in
  [the appendix](0849-a-package-declares-what-it-needs-at-run-time/prior-art.md). It needs a
  measurement once a first interpreter runs.
- Proposed milestone (candidate, not a fork): that ad hoc path, measured against the installed path
  on the first interpreter.

## Index row

A program's manifest declares what it needs at run time, a package declares what it provides, and a gate proves a route from each need to a provider. Every fork is ruled, versioning included.
