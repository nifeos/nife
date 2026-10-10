# Package boundaries: every crate and program in a package, every path with a home

*Milestone 611 (every program and crate belongs to a package, and every package has a home). The
file name, the key names of both TOML files, every package name and every home below are
provisional, minted by that lane on 2026-09-27 (UTC); the four kinds were ratified at 07:23Z that
day. calef ratifies the table; the questions are at the end.* What a package is as a file on a target is [notes/packages.md](packages.md). This note is
about the tree: which crate, program and path belongs to which package, and where each is going.

## What calef ruled

On 2026-09-27, on pull request #1389 (the OS is built and updated from packages; its decision is not yet on main):

- Fork 3: "R1 for now is right with package boundaries drawn and enforced inside it by lint
  check." The end state is everything moving out, and "I want to force decisions on homes versus
  there being a default of sticking around."
- Composition: "borrow the composition of packages from linux distros." A package is what releases
  together and may hold several programs. Authority stays per program: milestone 597 (a
  program carries its manifest in an ELF note) and §208 (installing a package is granting it).
- Scope: "Everything in the tree may not be in a package, but it may be in a repo." So a path has
  a home whether or not it ships.

## The two declarations

`packages/<name>.package.toml` declares a package. `packages/homes.toml` gives a home to every
tracked path that is in no package. Both are TOML, as the recipes are: calef ruled on 2026-09-27
that declarations and recipes move to TOML, refusing JSON for having no comments, and YAML. The
extensions and key names are provisional. An unknown key is refused, so a misspelt one cannot read
as absent. The field list, in full, is the header of `helpers/packages.py`. The required fields
have no default:

| field | required | what it says |
|---|---|---|
| `name` | yes | equals the file's stem |
| `kind` | yes | `base`, `optional`, `sdk` or `test` |
| `home` | yes | `{ repo, status = "provisional" }`, `{ repo, status = "ratified", date }`, or `{ status = "undecided", reason }` |
| `crates`, `interfaces` | one member at least | Cargo packages; an interface is one other packages may link |
| `programs` | | binary targets of a crate whose programs are split across packages |
| `paths` | | anything else: the std overlay, a recipe, a `#[path]` module |
| `depends` | | declared dependencies; those packages' crates may be linked |
| `[[exception]]` | | `date`, `member`, `crate`, `reason`: one dated link the rules refuse |

A package's kind says where it ends up. calef ratified that rule and the four kinds at
2026-09-27T07:23Z (UTC). `base` ends up in every image and `optional` on a running nife that
installs it. `sdk` ends up on a developer's machine: the contracts, the runtime and the host tools.
`test` ends up only in a test image, never in a release. A kind is not what a package does or who
runs it, so a question about a new package's kind is a question about its destination.

This is the in-tree form the base image list (P1 in #1389's decision) and the recipes are to be generated from.
A recipe needs a name, a version, an architecture and members; this file has the name and members,
and the version and architecture belong to a build. Nothing generates either one yet.

## The gate

`script/lint` runs `python3 helpers/packages.py --check`, after a selftest that plants one violation
per rule and fails unless each is caught. The rules:

1. Every tracked path has exactly one home. The longest claimed prefix wins, across every package
   member and every `packages/homes.toml` entry. A path nobody claims fails, and so does a prefix claimed
   twice or a claim that holds nothing.
2. Every Cargo package in the tree is in exactly one package. A crate's binaries go with it,
   unless any of them is placed elsewhere: then every binary needs a `program` line. That is
   `components` today, so a new program there has to be placed on purpose.
3. A link across a boundary goes to an `interface` crate, to a package the linker `depends` on, or
   through a dated exception. An exception whose link has gone fails, so the list only shrinks.
4. Nothing but a `test` package depends on a `test` package. A `base` package does not depend on an
   `optional` one.

For `components`, whose manifest links the union of fifty programs, rule 3 is checked per program.
It reads which crates each program's source names, comments stripped, among the crates the manifest
declares. The check was proved on the tree as well as in the selftest: a planted `use timetable`
in `components/src/rm.rs` failed with "coreutils: rm links timetable, an internal crate of
timetable" (the package was named `coreutils` then), and a planted untracked-then-added file failed as a path with no home.

## The table

`*` marks an interface. This table is generated, and `script/lint` fails when it is stale.

<!-- package table: helpers/packages.py --table writes this -->
| package | kind | members | allowed dependencies | home |
|---|---|---|---|---|
| `boot` | base | crates: `bitmap_font*`, `board_console`, `screen_console*`, `sealed_pair*`, `uefi_loader`; programs: `uefi_loader` | interfaces only; 1 dated exception(s) | `boot` (provisional) |
| `core-tools` | base | programs: `date`, `printenv`, `rm`, `uuid`, `wc` | interfaces only | `core-tools` (provisional) |
| `disk-tools` | base | programs: `disk_partitioner`, `disk_surveyor` | interfaces only | `disk-tools` (provisional) |
| `drivers` | base | crates: `designware_ethernet*`, `designware_mobile_storage*`, `e1000e*`, `jh7110_entropy`, `non_volatile_memory_express`, `virtio`; programs: `block_driver`, `designware_mobile_storage`, `jh7110_entropy`, `non_volatile_memory_express`, `serial_driver` | interfaces only | `drivers` (provisional) |
| `entropy` | base | programs: `entropy` | interfaces only | `entropy` (provisional) |
| `filesystem` | base | crates: `subtree_scope*`; programs: `fs_file_caretaker`, `fs_nameset_caretaker`, `fs_subtree_caretaker` | interfaces only | `filesystem` (provisional) |
| `init` | base | crates: `components`, `system_initializer`, `system_log`; programs: `broker`, `job_undertaker`, `progenitor`, `reboot`, `root_supervisor`, `spawner`, `sub_server_supervisor`, `swapper`, `system_log` | `timetable`; 2 dated exception(s) | `init` (provisional) |
| `kernel` | base | crates: `address_space_identifier`, `capability`, `cpu_set`, `direct_memory_access_validator`, `firmware_configuration`, `generational_table`, `inter_process_communication`, `intrusive_fifo`, `jh7110_clock_and_reset`, `kernel`, `memory_corruption_canary_gate`, `memory_regions`, `page_frames`, `paging`, `pci`, `thread_wake_handshake`, `work_steal_slot`; programs: `kernel` | interfaces only; 7 dated exception(s) | `kernel` (provisional) |
| `login` | base | crates: `credentialer`; programs: `credentialer`, `identity_provisioner`, `login`, `login_audit_receiver`, `user_timetable_keeper` | `timetable` | `login` (provisional) |
| `mdr` | base | programs: `mdr` | interfaces only | `mdr` (provisional) |
| `network` | base | crates: `domain_name_system`, `http_response`, `name_resolution_protocol`; programs: `name_resolver`, `net_stack`; paths: `components/src/virtio_net_transport.rs`, `components/src/socket_test_client.rs` | interfaces only | `network` (provisional) |
| `process-tools` | base | crates: `free`, `pgrep`, `pmap`, `ps`, `slabtop`, `top`, `uptime`, `vmstat`; programs: `free`, `pgrep`, `pmap`, `ps`, `slabtop`, `top`, `uptime`, `vmstat`; paths: `packages/uptime.recipe.toml`, `packages/uptime-riscv64.recipe.toml`, `packages/uptime-x86_64.recipe.toml` | interfaces only | `process-tools` (provisional) |
| `swish` | base | crates: `swish`; programs: `swish` | interfaces only | `swish` (provisional) |
| `terminal` | base | crates: `line_editor*`; programs: `console`, `input`, `line_editor`, `terminal_sink_caretaker`, `terminal_supervisor` | interfaces only | `terminal` (provisional) |
| `time` | base | crates: `network_time_protocol`; programs: `clock`, `network_time_client` | interfaces only | `time` (provisional) |
| `timetable` | base | crates: `schedule_store`, `timetable`; programs: `timetable`; paths: `components/timetable.conf` | interfaces only | `timetable` (provisional) |
| `demos` | optional | programs: `least_authority_demo` | interfaces only | `demos` (provisional) |
| `display` | optional | crates: `compositor*`, `extensible_host_controller_interface`, `usb`, `video_terminal`; programs: `compositor`, `display_terminal`, `framebuffer_driver`, `gpu_driver`, `graphical_terminal`, `keyboard_driver`, `usb_keyboard_driver` | interfaces only | `display` (provisional) |
| `redoxfs` | optional | crates: `redoxfs`, `redoxfs_host`, `redoxfs_server`; programs: `mkfs`, `redoxfs`, `redoxfs-ar`, `redoxfs-clone`, `redoxfs-mkfs`, `redoxfs-resize`, `redoxfs_host`, `redoxfs_server`, `second_mount`; paths: `vendor/redoxfs.divergence.patch`, `vendor/redoxfs.pin` | interfaces only | undecided: the server and host tool are ours and the library is Redox's; whether the port goes upstream is open |
| `rmle` | optional | programs: `rmle` | interfaces only | `rmle` (provisional) |
| `system_installer` | optional | programs: `system_installer` | interfaces only | `system_installer` (provisional) |
| `c-library` | sdk | crates: `c_library`, `c_library_errno`; paths: `vendor/relibc/` | interfaces only | `c-library` (provisional) |
| `contracts` | sdk | crates: `abi*`, `activation_set*`, `address_space_map*`, `argument_protocol*`, `block_roster*`, `boot_ladder*`, `boot_slot*`, `byte_sink_protocol*`, `c_program_fixture*`, `capability_witness_protocol*`, `clock_protocol*`, `component_plan*`, `confined_fuzz_protocol*`, `counter_frequency_protocol*`, `credential_protocol*`, `current_cpu_protocol*`, `device_tree_blob*`, `documentation*`, `elf*`, `entropy_protocol*`, `environment_protocol*`, `file_allocation_table*`, `filesystem_protocol*`, `glob*`, `globally_unique_identifier_partition_table*`, `grant_plan*`, `graphics_protocol*`, `login_protocol*`, `machine_discovery*`, `machine_statistics_protocol*`, `manifest_note*`, `measured_boot*`, `nifefs*`, `package_archive*`, `package_index*`, `socket_protocol*`, `std_runtime_protocol*`, `supervision_protocol*`, `swap_protocol*`, `system_log_protocol*`, `test_times*`, `universally_unique_identifier*` | interfaces only | `contracts` (provisional) |
| `cryptography` | sdk | crates: `cryptography_provider*`, `pinned_tls_client*` | `network` | `cryptography` (provisional) |
| `host-tools` | sdk | crates: `portable_executable`, `stick_maker`, `walk_pricing`, `xtask`; programs: `stick_maker`, `xtask` | `boot`, `display` | `host-tools` (provisional) |
| `runtime` | sdk | crates: `calendar*`, `entropy_backend*`, `user_mode_heap*`, `user_mode_runtime*`; paths: `patches/`, `targets/` | interfaces only | `runtime` (provisional) |
| `fixtures` | test | crates: `c_seam`, `coremark`, `cryptography_exerciser`, `fixtures`, `fuzz`, `job_mix`, `loaded_image_check`, `pinned_tls_exerciser`, `soak_page`, `std_exerciser`; programs: 66, too many to list here; paths: `packages/greeting.recipe.toml`, `packages/greeting-riscv64.recipe.toml`, `packages/greeting-x86_64.recipe.toml`, `packages/greeting-0.2.0.recipe.toml`, `packages/greeting-0.2.0-riscv64.recipe.toml`, `packages/greeting-0.2.0-x86_64.recipe.toml`, `packages/noteless.recipe.toml`, `packages/noteless-riscv64.recipe.toml`, `packages/noteless-x86_64.recipe.toml` | `init`, `time`, `timetable`, `network`, `host-tools`, `redoxfs` | `fixtures` (provisional) |
| `system-tests` | test | crates: `system_tests`; programs: `system_tests` | `kernel` | `system_tests` (provisional) |
<!-- end of package table -->

27 packages: 16 base, 5 optional, 4 sdk and 2 test by the table's count, `system-tests` having
joined from milestone 609 (the system tests leave the kernel crate). One has an undecided home; the
other 26 carry a provisional one. On 2026-09-27, 1,718 of 2,678 tracked paths had an undecided
home and 960 a provisional one; the weekly metrics page has the current count.

## How the packages were drawn

The start was #1389's seven divisions, `cargo metadata` over the eight workspaces, and one read of
which crates each component program names.

Where Linux has the tool, the distros' grouping was taken. `process-tools` holds `ps`, `pgrep`,
`pmap`, `top` and `uptime`, Debian's `procps` set, after milestone 126 (the `procps` package).
`core-tools` holds `date`, `printenv`, `rm`, `wc` and `uuid` (uuidgen). `disk-tools` holds
`disk_surveyor` (lsblk) and `disk_partitioner` (fdisk). Each shell and each editor is its own
package in Debian, so `swish` and `rmle` are too. `timetable` is the `cron` slot, and `mdr` the
`man-db` one.

Grouping is borrowed and names are not. On 2026-10-06 (UTC) calef ruled three package names, each
recorded in its manifest. The lane's `util-linux` became `disk-tools` ("util-linux is a horrible
package name for a nife package"). `coreutils` became `core-tools` and `procps` became
`process-tools`, so package names follow one rule, spelled out, and give up the GNU and Linux terms
of art on purpose. He also moved `uuid` out of the disk tools, against Debian's grouping
(util-linux, or uuid-runtime): "uuid can be used for lots of applications. Putting it in disk-utils
doesn't seem right."

Where Linux has only a role, nife groups by role and the table's header comment names the Debian
package it stands in for. These are `kernel`, `boot`, `init`, `drivers`, `terminal`, `display`,
`filesystem`, `network`, `time`, `entropy`, `login` and `system_installer`.

The contracts are one `-dev` split, Debian's `linux-libc-dev` shape: the ABI, every `*_protocol`
and the formats two programs agree on. A split per service, the `libfoo-dev` shape, is the
alternative. It waits until a contract changes on its own schedule, which #1389 measured as not yet
(76% of contracts commits touch another division).

The judgment calls, each a place the table could reasonably differ:

- `calendar` and `entropy_backend` are in `runtime`, since they are the time and randomness a libc
  provides. `glob`, `documentation`, `boot_ladder`, `block_roster`, `device_tree_blob` and
  `machine_discovery` are in `contracts`. Each is a pattern language, a store, a byte sequence, a
  page or a machine description that two programs read the same way.
- `line_editor` is an interface of `terminal`, and `compositor` of `display`. Each crate is the
  contract its package serves and the engine behind it at once.
- The `components` crate itself sits in `init`. It is a build container that the moves dissolve.
- `socket_test_client.rs` and `virtio_net_transport.rs` are `path` members of `network`, because
  `net_stack` includes both through `#[path]` and neither is a binary.
- `redoxfs` holds the vendored library, the server and the host tool. Its home is the one undecided
  package home: whether the port goes upstream is open.
- Documentation is in no package yet. calef (2026-09-27): "I'm thinking of building a website for
  much of our documentation. It may also ship in packages." Until that is settled every note has an
  undecided home, and [the proposal](../design/roadmap/0684-a-documentation-site.md) holds the
  question. A README inside a crate goes with its crate.

## What the gate found

Nothing was moved and no rule was weakened. The first run failed on 21 links across a boundary.

Eight were real dependencies, now declared in seven `depends` lines. `init`'s `session_reviver`
re-derived timetable entries, so it linked the timetable crates (retired 2026-09-27; `init`'s
`components` crate still links `timetable`). `fixtures` links internals of `init`, `time`, `timetable` and `network`,
which a test package is for. `host-tools` builds images from `boot` and `display`.

Thirteen were recorded as exceptions dated 2026-09-27, in the package file of the linker. Pull
request #1392 (the system tests leave the kernel) then made four of the kernel's links
dev-dependencies, and the gate failed their exceptions as stale, so nine remain:

| linker | links | why it is refused, and what fixes it |
|---|---|---|
| `kernel` | `non_volatile_memory_express`, `jh7110_entropy` | a handoff record and a device discovery living in driver crates |
| `kernel` | `video_terminal` | display service wiring in `kernel/src/user/` |
| `kernel` | `coremark`, `job_mix`, `soak_page` | benchmarks and probes built into the kernel; a release kernel should not link a fixture |
| `boot` | `job_mix` | `board_console` runs the job mix to prove a board boots |
| `init` | `http_response`, `video_terminal` | `system_initializer` builds the whole interactive image; it is integration, not init |

The driver row wants a lane of its own, and so do `ps::Row` and `pmap::Row`, which the kernel
now reaches only from its tests: each layout moves into `contracts`. That is
[contracts leave implementation crates](../design/roadmap/0689-contracts-leave-implementation-crates.md).
The fixture rows and the integration row are limitations, recorded below.

## How the moves will be done

Not in this milestone. One package per pull request, at a quiet moment in the queue, as unchanged
file moves (`git mv` and nothing else in the commit), so `git log --follow` and review both see a
rename. The package's `crates`, `programs` and `paths` are the list of what moves. The workspace
root and the gates follow in a second commit. The order is leaves first, contracts before the
programs that link them. [The proposal](../design/roadmap/0691-packages-move-out-one-per-pull-request.md)
holds the plan.

## Questions for calef

1. The format is TOML (ruled 2026-09-27). Are the fields above the shape, and are
   `packages/<name>.package.toml`, `packages/homes.toml` and `.recipe.toml` the names?
2. The kinds: answered, ratified at 07:23Z with the rule that a kind says where a package ends up.
3. The homes. The proposal is one repository per package, named after it, which is Debian's
   source-package shape. Grouping by #1389's seven divisions is the alternative. Which?
4. The project records: `design/`, `briefs/`, `script/`, `helpers/` and `.github/`. Does this
   repository become their home, or do they leave too?
5. The names: all 27 packages, `helpers/packages.py`, this note and the `homes` metric.

## BUGS

- Dev-dependencies are not checked, since they never ship. A test that links another package's
  internals is therefore invisible here, and it will break at the move.
- The per-program check reads source text. A crate reached only through a macro, or a `use` of a
  renamed dependency, is missed. The manifest-level check covers every crate that is not split.
- A `depends` line admits every crate of that package, internals included. It is a reviewed
  declaration, not a proof, so review has to ask whether each one is true.
- Four fixtures are linked by base packages: `coremark`, `job_mix` and `soak_page` by the kernel,
  and `job_mix` by `board_console`. They are exceptions until a release build can leave them out.
- `system_initializer` sits in `init` but builds the whole image. It links `network` and `display`
  internals under exception until it has its own integration package.
