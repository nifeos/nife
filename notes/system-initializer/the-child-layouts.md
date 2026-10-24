# The child layouts: where a spawned child's endowment goes

The appendix of [`notes/system-initializer.md`](../system-initializer.md) for a child's table.
How each grant is decided is in [`the-spawn-service.md`](the-spawn-service.md).

## The native layout

A native child's table is filled from slot 0, and slot 0 is the output. It is the shared result
endpoint unless the shell delegated a sink, for `>` or the left of a `|` (milestone 50 (pipes and
redirection)). The child never learns which. When the shell narrowed this tail to the screen (§106
(an unredirected tail stage's output goes to the screen)) the terminal's sink takes slot 0. The shell
asked by delegating a completion endpoint, so slot 0 is as opaque to the child as a redirected one.
Behind a directory grant the narrowed
endpoint takes slot 0 and the output moves to slot 1. That is the contract `components/src/rm.rs`
documents and the kernel's `start_granted_dir` wires, so a program means one thing in a guest test
and at the prompt. The grant goes first because it is the authority the line named.

Then come the clock, the configuration page, the input source with `READ` only, and the `--mem`
budget. The clock and configuration page come before the source and budget, so `date`'s clock and
`printenv`'s configuration are slot 1. Slot 1 is unambiguous only because no manifest declares a
clock or configuration with an input, which is the ordered-slot debt in `notes/pipes.md`'s BUGS. The
array holds six, what the fields allow rather than what a shipped manifest uses. A directory grant
takes two, and the clock, configuration, source and budget one each.

The budget is narrowed to `SPEND_ONLY`. The child may spend it, but may lend neither it nor anything
retyped from it (milestone 824 (a retype mints no right its budget lacks)). A child holding the
network gets every right, because it lends the stack a page per socket out of this budget.

The named slots sit high and explicit. How many low slots a child gets depends on what else the line
granted, and a program that probes for a slot by number cannot have it move:

| slot | what | right | why that right |
|---|---|---|---|
| the manifest's diagnostics slot | the second stream: a `2>` endpoint, else the terminal's sink (§67 (a program's second stream is a declaration)) | `WRITE` | it prints and nothing else |
| `DOMAIN_SLOT` | `deaths`, this shell's jobs' supervision endpoint | `ENUMERATE` | `READ` would also give `RECEIVE` and `REAP` |
| `ENTROPY_SLOT` | the entropy service (milestone 111 (endow a child with entropy)) | `WRITE` | `CALL` only, never another client's request |
| `NETWORK_SLOT` | the network stack (milestone 590 (the booted system starts its network stack)) | `WRITE` | as entropy, and no `GRANT` to a grandchild |
| `MACHINE_SLOT` | the machine statistics page, also mapped | `READ` | |
| `SHARE_SLOT` | the job pool | `ENUMERATE` | answers `USAGE` and nothing that spends |
| `REBOOT_SLOT` | the reboot object (§251 (restarting the machine is a kernel object)) | `WRITE` | invoke, never hand on |
| `SYNC_SLOT` | the sync-only file endpoint (milestone 805 (`reboot` at the prompt)) | `WRITE` | `CALL` the file server |

The second stream defaults to the terminal's own sink when the line had no `2>`, because the shell
delegates an endpoint only when it has a file to back. That also keeps a redirected `date`'s complaint
off the redirection. A child with no second stream says what it has to say in-band.
`unreachable_network_witness` at the prompt checks that the network slot is filled only for a
declaring child.

The domain is a view over the supervision domain the child is about to be born into. `deaths`
supervises every job the progenitor spawns for this shell, so a viewer sees this shell's jobs and
itself. The progenitor, the shell, the terminal and the file server are outside it. That is the confinement claim `kernel::user::survey_tests` checks. The right is
`ENUMERATE` because a domain names its members and does not act on them (calef, 2026-08-17), and
`notes/process-view.md` carries the argument. The share view is the region this child was split from,
so "yours" includes the program asking, the way `ps` lists itself.

A native child maps, each only when asked for: the directory grant's page, the clock, the
configuration page and the machine statistics page. Each is a page the progenitor holds and only
maps, so the child's reclaim leaves them alone. The file contract's page is one page for the program,
the caretaker and the file server. That is sound because every request on both hops is a blocking
`CALL`, so the program is parked inside its own call while the caretaker uses the page.

A program behind a directory grant is started with the grant's own three words, which the
progenitor forwards unread. That is `rm`'s shape: a spec of options and two words of name
(`filesystem_contract::grant`). Any other native program gets the line's integer. A `std` program is started with zeros:
nife's `_start` ignores all three, and its words arrive on the argument page (§170 (how a foreign
program is told what to do)).

## The `std` layout

`std_layout::StdLayout` places a `std` child's endowment where nife's `std` reads it, one slot per
authority (`crates/std_runtime_contract`, `grant_plan`), from the same decisions the native layout
reads. A native child's output slot is a `std` child's heap budget, so a `std` program built natively
dies on its first `println!`.

| slot | what | right | page |
|---|---|---|---|
| 0 | the job's own region, the heap's budget | `SPEND_ONLY`, every right for an installer holder | |
| 1 | the output, the capability a native child gets at slot 0 | | |
| 2 | the network stack | `WRITE` | |
| 3 | the job's region again, for socket pages | every right | |
| 4 | the directory grant, or the index store | `WRITE` | `FS_PAGE` |
| 5 | the clock | `READ` | `CLOCK_PAGE` |
| 6 | the entropy service | `WRITE` | |
| 7 | the configuration page | `READ` | `CONFIG_PAGE` |
| 8 | the child's own copy of the argv (§170) | `READ` | `ARGS_PAGE` |
| 9, 22 to 28 | a resolver badge per granted zone | `WRITE` | |
| 10, 11, 12 | its own process, first thread and space | `BIND`, `WRITE`, `READ` | |
| 15, 16 | the installer endpoint and its reply endpoint | `WRITE`, `READ` | |
| 17 | the catalog | `READ` | `CATALOG_PAGE` |
| 18 | the image's index sources | `READ` | `INDEX_SOURCES_PAGE` |
| 29 | the boot terminal as standard input | `WRITE` | |
| 30 | the line's input, a streamed file or a pipe's read end | `READ` | |

Pages lent out of a budget carry that budget's rights (milestone 824), so `GRANT` must be on the
region wherever the child lends. Slot 3 carries it for the network. Slot 0 carries it for an
installer holder, which stages a package in pages retyped from its heap and lends them to the
progenitor. Every `std`
program is built as a process of its own and may add threads within its region's budget (milestone
812 (`std::thread::spawn` runs real threads in one address space)). None of the three thread
capabilities reaches anything outside the program.

*Name: provisional. calef names files.*
