# Leaving the central program table: every row, classified

[The proposal](../leaving-the-central-program-table.md) holds the end state and the order. This
appendix classifies each row of the `programs!` table in `crates/grant_plan/src/program/catalog.rs`.
Every fact here was read from the tree at the commit this branch was cut from. A file and a symbol
stand in for line numbers, which §277 (cite a heading or a symbol) forbids.

## The count

The table has 28 rows, wire ids 0 to 27 with no hole, so `PROGRAM_COUNT` is 28 and equals the
number of programs today. `Program::manifest` in `program/manifests.rs` has 27 match arms, not 28,
because one arm covers two variants: `Program::NullSyscall | Program::SinkProducer` both answer
`bench_row_manifest()`. `Program::SinkConsumer` reuses `wc_manifest()`. So 26 manifest functions
serve 28 rows. The two counts differ by that one shared arm and nothing else.

The test `the_wire_ids_already_shipped_never_move` in `program/tests.rs` pins 13 ids, 0 to 12. It
allows a pinned name to disappear and refuses only reuse of its id by another name. Ids 13 to 27
have no pin.

## The classes

Four classes, plus one row whose class depends on a ruling.

- **Pinned:** a grant no installed program may ever hold. The note encoder refuses these fields at
  compile time (`capability_manifest_note::encode`), and `grant_plan::image_can_carry` refuses them
  at spawn.
- Tool: a program a person types at the prompt for its function, owned by a package under
  `packages/`.
- Fixture: a program that exists to exercise or measure the system. Each lives in `fixtures/` or
  `std_exerciser/` and belongs to the `fixtures` package (kind `test`).
- Bootstrap: a program that must run before any package can. None of the 28 qualifies, as the
  finding below explains.

| id | name | class | package | what its manifest holds beyond output |
|---|---|---|---|---|
| 0 | `least_authority_demo` | tool (a demonstration) | `demos` (optional) | an argument; answers in words |
| 1 | `memory_grant_depleter` | fixture | `fixtures` | `--mem` 1 to 64 pages |
| 2 | `interrupt_heeder` | fixture | `fixtures` | interruptible, silent |
| 3 | `interrupt_ignorer` | fixture | `fixtures` | interruptible, silent |
| 4 | `date` | tool | `core-tools` | clock, second stream |
| 5 | `rm` | tool | `core-tools` | directory with `-r` subtree, options `rfv` |
| 6 | `wc` | tool | `core-tools` | input |
| 7 | `mdr` | tool | `mdr` | input; `std` |
| 8 | `ps` | tool | `process-tools` | domain, second stream |
| 9 | `pgrep` | tool | `process-tools` | domain, second stream |
| 10 | `uptime` | tool | `process-tools` | nothing |
| 11 | `printenv` | tool | `core-tools` | configuration page |
| 12 | `uuid` | tool | `core-tools` | entropy, second stream |
| 13 | `top` | tool | `process-tools` | domain, machine page, second stream |
| 14 | `network_echo_client` | fixture | `fixtures` | network, `--mem` 4 to 16 |
| 15 | `unreachable_network_witness` | fixture | `fixtures` | nothing (it carries a note of its own) |
| 16 | `std_exerciser` | fixture | `fixtures` | words; clock, configuration, entropy; `std` |
| 17 | `free` | tool | `process-tools` | machine page, share view, second stream |
| 18 | `vmstat` | tool | `process-tools` | machine page, second stream |
| 19 | `slabtop` | tool | `process-tools` | share view, second stream |
| 20 | `graphical_terminal` | unclear (below) | `display` (optional) | words; the progenitor builds a display session for it |
| 21 | `reboot` | pinned | `init` | `reboot`, `sync`, second stream |
| 22 | `jig` | pinned | `jig` | words; clock, entropy, network, installer, catalog, indexes, activation directory, console input; a verb table; the root resolver zone |
| 23 | `job_sleeper` | fixture | `fixtures` | an argument, `--mem` 4 to 16 |
| 24 | `null_syscall` | fixture | `fixtures` | `--mem` 10 to 16 |
| 25 | `sink_producer` | fixture | `fixtures` | `--mem` 10 to 16 |
| 26 | `sink_consumer` | fixture | `fixtures` | input (the `wc` manifest) |
| 27 | `map_el0` | fixture | `fixtures` | `--mem` |

Where each package comes from: `core-tools.package.toml` lists `date printenv rm uuid wc`;
`process-tools.package.toml` lists `ps pgrep pmap top uptime free vmstat slabtop`; `mdr`, `jig` and
`demos` each list their one program; `init.package.toml` lists `reboot` among the boot programs;
`display.package.toml` lists `graphical_terminal`. The ten fixtures above are `[[bin]]` blocks in
`fixtures/Cargo.toml`, and `fixtures.package.toml` owns the `fixtures` and `std_exerciser` crates.

### The count by class

| class | rows | names |
|---|---|---|
| pinned | 2 | `reboot`, `jig` |
| tool | 14 | five in `core-tools`, seven in `process-tools`, `mdr`, `least_authority_demo` |
| fixture | 11 | ids 1, 2, 3, 14, 15, 16, 23 to 27 |
| unclear | 1 | `graphical_terminal` |
| bootstrap | 0 | |

### Why no row is bootstrap

The progenitor starts every boot program by archive name, not through this table: the console, the
shell, the file servers and the drivers appear in `packages/base-image.toml` and in no row here.
`measure_spawnable` in `crates/system_initializer/src/boot_steps.rs` loads the table's programs only
to serve the shell's later requests. So nothing in the table runs before the shell asks for it. The
nearest case is `jig`: without it no package can be installed, so an image must carry it. That is a
reason to keep it in the base image, and its pinned grants already keep its row.

### Why `graphical_terminal` is unclear

Its manifest declares no field a note cannot carry, so `every_compiled_in_manifest_round_trips` in
`crates/capability_manifest_note/src/tests.rs` encodes it. Its authority comes from its identity
instead. The shell sets `spawnproto::Wiring::graphics` when `e.program ==
Program::GraphicalTerminal` (`components/src/swish/running.rs`, the request builder). The
progenitor then builds `gpu_driver`, `display_terminal` and the keyboard path and wires them to it
(`crates/system_initializer/src/spawn_loop.rs`, the checks on `Program::GraphicalTerminal`). Holding
the screen and the keyboard is the same kind of authority as `console_input`, which only the
owner's console grants. Whether that may become an ask an owner grants at install, like the
network, or stays pinned is a ruling; [decision 4](decisions.md) asks it. Until it is ruled, the
proposal treats the row as pinned.

### What the fixtures need, and a conflict about the bench rows

The bench rows (ids 24 to 27) are milestone 961 (the kernel's special boots become commands typed
at the production shell)'s work, and that block says a production shell runs them. But
`packages/base-image.toml` says no release image carries the `fixtures` package. The two agree
only because `cargo xtask initial-ramdisk-*` packs every `[[bin]]` (§158 (a program is declared
once), item 2), so the images that tests and benches boot carry fixtures anyway. A composed release
image would answer the bench rows with `SPAWN_NOT_IN_IMAGE`. Whether the bench rows belong in a
release image settles their class; it is not this proposal's question, and the
[unverified appendix](unverified.md) records it.

## Who spawns each row

| spawner | which rows | how, read from code |
|---|---|---|
| the shell | all 28 | `Program::from_name` in `grant_plan::plan_stage`, then `spawnproto::request` with `Program::id` in word 0 |
| the progenitor | all 28, only on the shell's request | `Program::from_id(spawnproto::prog_id(w0))` in `spawn_loop.rs`'s request reader, then the ELF `measure_spawnable` loaded at boot |
| the timetable | any row a timetable document names | `components/src/timetable.rs`'s `resolve` reads `e.program.name()` from its archive, and `fire_row` endows `registered.program.manifest()` |
| the kernel | none | the kernel links `grant_plan` for `spawnproto::RUN_UNVOUCHED_SLOT` and `grant_plan::argv` only (`kernel/src/user/login_service.rs`, `kernel/src/user/fs_service/std_spawn.rs`) |
| tests | `script/swish-check` types every row; `system_tests` checks every row's measurement | `xtask/src/swish_check/script/*.rs`; `measured_boot_tests.rs` sweeps `0..PROGRAM_COUNT` |

The timetable is a fourth spawner the question did not name. It builds and fires jobs itself from
an archive of the programs its plan names, and in store mode it reads installed programs' notes
instead (`resolve_installed`). So it already holds both paths.

## Two programs outside the table

`pmap` is in `process-tools` and in the base image, and no row names it. §158 (a program is
declared once) records under its BUGS that every archive carries `pmap` and nothing spawns it. The
disk tools `disk_surveyor` and `disk_partitioner` are also rowless; only kernel tests reach them
(`system_tests/src/user/disk_tests.rs`). Both facts show that membership in a package and a row in
this table are already independent.
