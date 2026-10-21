# std, C library and packages

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  0849/versioning.md | provider range floor is hand-written; only a test per retained level catches a deleted level | A test per retained contract level | S  |
|  c_library/README.md | getrusage answers zeros and the CPU-time clock is the monotonic one; no path from SURVEY's thread CPU time to C | C library: getrusage and CPU-time clocks from thread CPU time | M  |
|  c_library/README.md | syslog drops messages; no route to system_log | C library: syslog writes to the system log service | M  |
|  c_library/README.md | sys/user.h does not compile (relibc arch user.h not seeded) | C library: seed relibc's arch user.h | S  |
|  c_library/README.md | no dup or readdir (README says readdir is what a consumer wants first; std::fs::read_dir can carry it) | C library: readdir and dup | M  |
|  c_library/README.md | static C destructors never run (exit does not walk .fini_array) | C library: exit runs .fini_array | S  |
|  crates/swish/src/jig_versions.rs (2nd bullet) | The dev tool that installs the version a set names is not built (ruling 4's follow-on), so a set naming an uninstalled version only yields the divergence notice. | A tool that installs what a version set asks for | M  |
|  design/decisions/0084-how-we-port.md (cap-primitives bullet) | Nobody has read `cap-primitives`, so whether a third backend fits is unknown. | Read cap-primitives and say whether a nife backend fits | S  |
|  design/decisions/0084-how-we-port.md (patch burden, tiers) | No tool measures patch burden against upstream; no corpus has been put in tiers 1 to 4. | Measure patch burden and classify a program corpus by tier | M  |
|  design/roadmap/0121-ripgrep-and-enumeration.md | `ignore` gitignore semantics untested; the fixture holds no ignore files | Walker fixture with ignore files | S  |
|  design/roadmap/0184-std-x86-64.md | A zeroed timebase page reads as 1 GHz, so a std program built by `build_child_space` scales `Instant` wrongly | Real timebase page for userspace-built std children | S  |
|  design/roadmap/0310-miri-leak-free.md | `crates/paging/tests/mapping.rs` still has opt-in `TableGuard` (21 call sites) nothing forces to be bound | `mapping.rs` tests bind their table guard by construction | S  |
|  notes/crates-io-on-nife.md | No crates.io crate has ever been booted under a stated endowment; only compile and link evidence. Milestone 64 (BUILT) names this as its own acceptance criterion; needs a §46 (Thin primitives or whole subsystems; we write everything in between) ruling first | Boot one crates.io crate under a stated endowment | M  |
|  notes/packages.md | a slow package source makes the prompt wait | Bound the prompt's wait on a slow source | S  |
|  notes/packages.md | a manifest note may not declare files, directories, inputs, options or std runtime (see 0597's BUGS) | Image requests carry files and inputs | M  |
|  notes/packages/jig.md | refusals name no generation; `remove pkg@version` it cannot pick names no candidates | jig diagnostics name generation and candidates | S  |
|  notes/packages/jig.md | `update` keeps no record of which address answered | jig remembers the answering address | S  |
|  notes/packages/jig.md | `list -u` compares versions only, a changed digest at the same version is not reported | list -u compares digests | S  |
|  notes/packages/over-the-internet.md | owner's pinned mirror (`Index::sources`) has no owner setting, proved by host tests only | Owner setting for the pinned mirror | M  |
|  notes/packages/running-unvouched.md | a session that sets the promised-word bit without the capability hangs the progenitor (every promised message has this exposure) | Bound the progenitor's wait on promised words | M  |
|  notes/packages/running-unvouched.md | refused D2 path has no permanent gate; the capability census counts caps not mappings | Permanent gate for the D2 refusal and a mapping census | S  |
|  notes/std/std-aborts.md (stale-cache bullets; also notes/std.md last bullet) | `std-aborts` never checks scanned paths are under `farm_dir()`, so a stale or foreign build reports false source defects; stale cache pinning plain nightly std undiagnosed | `std-aborts` asserts its paths are under `farm_dir()`; diagnose the pinning | S  |
|  notes/std/threads.md (also notes/std.md first bullet) | Spawned thread stack has no guard page (overflow writes into heap); detached thread stacks are never freed | Guard page and reclamation for `std` thread stacks | M  |
|  patches/std-nife/.../futex/nife.rs | timed waits spin on `yield` until deadline (kernel `WAIT` has no timeout; Timer from 106 exists to serve one) (also notes/futex.md BUGS) | A timeout on futex `WAIT` | M  |
|  patches/std-nife/.../paths/nife.rs | `tempfile` crate still fails ("operation not supported"): no nife arm in its platform ladder | Give `tempfile` a nife arm | M  |
|  patches/std-nife/.../thread/nife.rs | a detached thread's stack is never freed, and each spawn spends one TCB page of the region for good (two entries merged) | Free a detached thread's stack and reclaim its TCB page | M  |
|  patches/std-nife/.../thread/nife.rs | spawned thread's stack has no guard page (plan in `notes/std/threads.md`; 812 BUILT did not do it) | Guard page under a spawned thread's stack | M  |
|  roadmap 0809 | `jig` refusals name no generation or candidate versions; `list -u` ignores a digest change at the same version | jig names candidates and reports digest drift | S  |
|  roadmap 0812 | no futex timeout; no guard page on a spawned stack | Futex timeouts and a thread stack guard page | M  |
|  roadmap 0816 (also 0817, 0818, 0819 `.dts` bullet) | licenses of device-tree fixtures unchecked before `cargo package` | Record fixture licenses before publishing | S  |
|  roadmap 0851 | `realloc` always copies (1.5 N peak); commit page tables stay until exit | In-place realloc and table reclaim | M  |
|  roadmap 0918 | `FAQ.md` raw HTML headings render as text; check only reports | Renderer handles raw HTML headings | S  |
|  roadmap 0919 | one 4,096-byte generation holds ~two dozen programs; 13 used by two packages | Generation grows past one page | M  |
|  roadmap 0921 | installed `rmle` cannot be started (no raw terminal wiring at prompt) | A raw terminal launch path for rmle | M  |
