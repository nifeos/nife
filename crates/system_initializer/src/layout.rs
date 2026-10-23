//! The address-space layout and page budgets: every address a child or the progenitor maps at, each
//! stack's budget, and the constants each stack's program must agree on. Kept in one file because
//! its correctness is that entries do not collide, and that is checked by reading them side by
//! side. How each budget was counted is in notes/system-initializer/budgets.md.
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

use super::*;

/// Where the kernel maps the initial ramdisk archive, read-only. Must match
/// `kernel::user::INITIAL_RAMDISK_VA`.
pub(super) const INITIAL_RAMDISK_VA: u64 = address_space_map::runtime_window(0x2000_0000);

/// Stack pages every child the progenitor builds gets, mapped down from
/// `supervision_protocol::CHILD_STACK_VA`; measured, with four pages of headroom.
///
/// `kernel::user::pipeline_service`'s `SHELL_EXTRA_STACK` must stay level with this, or a test
/// wiring finds faults the boot does not have (notes/pipes/the-boot.md). The kernel cannot depend
/// on this crate, so that one is a second copy, and it follows this. See
/// notes/system-initializer/budgets.md.
pub const CHILD_STACK_PAGES: u64 = 12;

/// Where a child that declares a clock maps it, read-only. Must match `components/src/date.rs`'s
/// `CLOCK_VA` and `kernel/src/user/clock_service.rs`.
pub(super) const CHILD_CLOCK_VA: u64 = address_space_map::pair_page(0x00c0_0000);

/// Where a child that declares the inert-configuration page maps it, read-only (DECISIONS §111
/// (inert configuration is a validated page)). Must match `components/src/printenv.rs`'s
/// `CONFIG_VA`. `std_service.rs`'s `CONFIG_PAGE_STD` is another wiring's, and need not agree.
pub(super) const CHILD_CONFIG_VA: u64 = address_space_map::pair_page(0x00e0_0000);

/// **Whether the boot prompt's session is handed the machine statistics page**: the owner's
/// switch for DECISIONS §225 (`free` sees the machine and your share) part 2, which grants the page
/// by default and lets the owner withhold it. `false` and every `free` says it cannot see the
/// machine and `vmstat` refuses, rather than printing zeroes.
///
/// # BUGS
///
/// It decides for the boot prompt alone. Nothing hands the page to `login`, so a per-login policy
/// does not exist yet.
///
/// Name: ratified 2026-09-27 (calef, #1360's table).
pub const GRANT_MACHINE_PAGE: bool = true;

/// Where a supervised (interruptible) child maps its shared job frame (DECISIONS §24
/// (interrupting the foreground process)). Must match `interrupt_heeder.rs` and
/// `interrupt_ignorer.rs`'s `JOB_PAGE_FRAME_VA`.
pub(super) const CHILD_JOB_PAGE_FRAME_VA: u64 = address_space_map::pair_page(0x0030_0000);

/// Pages of untyped split off our own budget and handed the shell, which endows the programs it
/// spawns (`run --mem N`) from it. A session budget, not a renewable one. The number is
/// [`spawnproto::SHELL_BUDGET_PAGES`], which the shell reads too.
pub(super) const SH_BUDGET_PAGES: u64 = spawnproto::SHELL_BUDGET_PAGES;

/// **What the progenitor keeps for itself after the boot servers are up**: the page tables
/// reaching the loader's scratch window, which must never come out of a child's region, and the
/// building of `job_undertaker`. The scratch window's tables cost at most
/// `supervision_protocol::SCRATCH_TABLE_PAGES`, and the assertion below keeps that half spoken for.
/// See notes/system-initializer/budgets.md.
///
/// Name: ratified 2026-10-18 (calef: "Progenitors numbers are ratified."), replacing `INIT_OWN_PAGES`.
pub const PROGENITOR_RESERVED_PAGES: u64 = 128;
const _: () = assert!(PROGENITOR_RESERVED_PAGES >= 2 * supervision_protocol::SCRATCH_TABLE_PAGES);

/// **One job's region**: everything a spawned program is made of, so a single reclaim frees all of
/// it. Spent per live job, not per job ever run. Counted from the largest debug image the prompt
/// spawns, [`CHILD_STACK_PAGES`], a TCB, an address-space root, the tables for the windows a child
/// touches and the mapping records of §13 (capability revocation and untyped reclamation); see
/// notes/system-initializer/budgets.md.
pub(super) const JOB_REGION_PAGES: u64 = 48;

/// **One directory-granted job's region**: the program *and* the `fs_subtree_caretaker` that carries
/// its grant, plus the two endpoints between them, all out of one carve, so the client's one reap
/// ends both (DECISIONS §92 (a caretaker is supervised by the client it serves), §40 (a
/// supervisor's death is its subtree's death)). The endpoints must come from this region: the reap
/// sweeps them, and that is what wakes a caretaker parked in `RECEIVE`. See
/// notes/system-initializer/children.md and notes/system-initializer/budgets.md.
pub(super) const DIR_JOB_REGION_PAGES: u64 = 98;

/// The stack a `fs_subtree_caretaker` gets, beyond the one page `build_child` maps for it: it has
/// no allocator, no recursion, and one sizable frame (a sixteen-entry handle table and a
/// `grant::MAX_NAME` name buffer).
pub(super) const CARETAKER_STACK_PAGES: u64 = 4;

/// Where a `fs_subtree_caretaker` and the program it serves both map the FS contract's shared page.
/// Must match `components/src/fs_subtree_caretaker.rs`'s `PAGE_VA` and `components/src/rm.rs`'s.
///
/// One address for both, because a request travels caretaker-to-server and program-to-caretaker
/// through the same frame.
pub(super) const FS_CLIENT_PAGE_VA: u64 = address_space_map::pair_page(0x0060_0000);

/// Where `fs_nameset_caretaker` reads its name set, read-only (milestone 205). Must match that
/// program's `SET_VA`, as [`FS_CLIENT_PAGE_VA`] matches its `PAGE_VA`.
pub(super) const NAMESET_PAGE_VA: u64 = address_space_map::pair_page(0x0070_0000);

/// **The job pool**: six live native jobs and one `std` program's region
/// ([`grant_plan::STD_REGION_PAGES`]). Small on purpose: `script/swish-check` runs more jobs
/// through it than it could hold without the regions coming back, so widening it silently retires
/// that gate. See notes/system-initializer/budgets.md.
pub const JOBS_BUDGET_PAGES: u64 = JOB_REGION_PAGES * 6 + grant_plan::STD_REGION_PAGES;

/// **The image pool** (a provisional name), apart from [`JOBS_BUDGET_PAGES`], where an image run by
/// path or installed is staged and built: the largest image twice (the child's pages, and the copy
/// §219 (how the shell names an installed program to the spawner) has us hash) and a `std` heap.
///
/// # BUGS
///
/// - **The image is held three times while a child is built**: the shell's frames, this copy, and
///   the child's pages. Hashing as the child is filled would halve the pool, but needs a loader
///   that hashes as it fills from the caller's still-mapped frames.
/// - **Reserved, not borrowed**, at boot, whether or not an image ever runs.
pub const IMAGE_POOL_PAGES: u64 = grant_plan::image_region_pages(
    spawnproto::IMAGE_MAX_PAGES,
    grant_plan::StartupContract::Std,
    JOB_REGION_PAGES,
) + spawnproto::IMAGE_MAX_PAGES;

/// **Everything both pools can have built at once, and an image request's views, fit in half the
/// loader's scratch window**: a job's pages stay mapped there until its region is destroyed. The
/// other half is the boot servers', unmeasured; see notes/system-initializer/budgets.md.
const _: () = assert!(
    JOBS_BUDGET_PAGES + IMAGE_POOL_PAGES + spawnproto::IMAGE_MAX_PAGES
        <= supervision_protocol::SCRATCH_WINDOW_PAGES / 2
);

/// Where the progenitor maps the shell's output frame in **its own** address space, to print the one line it
/// ever prints (the dropped-authority negative control). Well clear of the progenitor's segments, its stack,
/// and the loader's scratch window at `0x1000_0000`.
///
/// Name: ratified 2026-10-18 (calef: "Progenitors numbers are ratified."), replacing `INIT_OUT_VA`.
pub(super) const PROGENITOR_SHELL_OUTPUT_VIRTUAL_ADDRESS: u64 =
    address_space_map::pair_page(0x0f00_0000);

/// Where the progenitor briefly maps the virtio-rng DMA page, in **its own** address space, to read
/// [`VIRTIO_DMA_PHYS_OFFSET`] back out before handing the same frame on to entropy. Given up with
/// `UNMAP` the moment the offset is read (§249 (a running address space stays nameable)).
pub(super) const RNG_DMA_PEEK_VA: u64 = address_space_map::pair_page(0x0f10_0000);

/// The DMA region's own physical base, written inside the page itself at its last eight bytes
/// (`abi::virtio::DMA_PHYS_OFFSET`, which carries the reasoning for exactly this offset), shared
/// with the drivers that read it.
pub(super) const VIRTIO_DMA_PHYS_OFFSET: u64 = abi::virtio::DMA_PHYS_OFFSET;

/// Where entropy maps its own DMA page. Must match `components/src/entropy.rs`'s `DMA_VA`.
pub(super) const RNG_DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

/// `components/src/entropy.rs`'s `MODE_VIRTIO`: a spawner's understanding with the one program it
/// spawns, not a contract between two user programs, so rule 7 does not make it a crate.
pub(super) const RNG_MODE_VIRTIO: u64 = 0;

// -------------------------------------------------------------------------------------------
// The network stack (milestone 590 (the booted system starts its network stack)).
// -------------------------------------------------------------------------------------------

/// Where this process briefly maps the network card's DMA page to read [`VIRTIO_DMA_PHYS_OFFSET`]
/// back out, [`RNG_DMA_PEEK_VA`]'s twin: given up with `UNMAP` once the offset is read.
pub(super) const NET_DMA_PEEK_VA: u64 = address_space_map::pair_page(0x0f30_0000);

/// Where `net_stack` maps its DMA page. Must match `components/src/virtio_net_transport.rs`'s `DMA_VA`, the
/// same kernel-and-spawner convention [`RNG_DMA_VA`] is.
pub(super) const NET_DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

/// `net_stack`'s heap budget, in pages: matches
/// `kernel::user::virtio_service::NET_SERVER_BUDGET_PAGES`, which carries the measurement. A
/// spawner states the budget it hands; see notes/system-initializer/budgets.md.
pub(super) const NET_STACK_BUDGET_PAGES: u64 = 128;

/// `net_stack`'s stack, in pages: the kernel harness maps eight extra pages under the one every
/// process gets (`kernel::user::virtio_service::NET_SERVER_STACK_PAGES`, "smoltcp builds packets
/// on the stack"), and nine is those together, since `build_child` counts the whole stack.
pub(super) const NET_STACK_STACK_PAGES: u64 = 9;

/// **`domain_name_resolver`'s untyped budget** (milestone 892 (a std program at the prompt holds the
/// network and a resolver)): its socket page and the page tables for up to
/// `domain_name_resolution_contract::GRANTS_MAX` client pages. The kernel harness's
/// `name_resolver_tests::BUDGET_PAGES`, which every resolver test runs under.
///
/// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
/// The pull request records no other name considered.
pub(super) const RESOLVER_BUDGET_PAGES: u64 = 16;

/// `domain_name_resolver`'s stack, in pages, counted whole as [`NET_STACK_STACK_PAGES`] is: the
/// kernel harness's measured eight extra and the one every process gets.
///
/// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
/// The kernel harness's `name_resolver_tests::RESOLVER_STACK_PAGES` is older; the pull request
/// records no other name considered.
pub(super) const RESOLVER_STACK_PAGES: u64 = 9;

/// `net_stack`'s own spawn-argument convention: entry role 0 is the server
/// (`components/src/net_stack.rs`'s
/// `_start`). [`RNG_MODE_VIRTIO`]'s reasoning.
pub(super) const NET_STACK_ROLE_SERVER: u64 = 0;

/// `components/src/line_editor.rs`'s `MODE_CONSOLE`, for [`RNG_MODE_VIRTIO`]'s reasoning: prints
/// through the console's two-endpoint protocol.
pub(super) const LINE_EDITOR_MODE_CONSOLE: u64 = 0;
/// `line_editor.rs`'s own `MODE_DISPLAY`: the `graphical_terminal` session's wiring, prints through
/// `display_terminal`'s `OPERATION_WRITE`/one-`CALL` contract instead.
pub(super) const LINE_EDITOR_MODE_DISPLAY: u64 = 1;

// The VAs each program hardcodes; they must match console.rs / input.rs / line_editor.rs / swish.rs.
pub(super) const CON_SHARED_VA: u64 = address_space_map::pair_page(0x0060_0000); // console reads text here; line_editor writes it
/// Where the console maps the page `display_terminal` reads an `OPERATION_WRITE`'s bytes from, when a
/// screen was wired beside the UART. Must match `components/src/console.rs`'s `SCREEN_OUT_VA`.
pub(super) const CON_SCREEN_OUT_VA: u64 = address_space_map::pair_page(0x0068_0000);
/// `console.rs`'s own `MODE_SCREEN`: [`LINE_EDITOR_MODE_CONSOLE`]'s reasoning, one program over.
/// `0`, what every other boot passes, is the UART alone.
pub(super) const CONSOLE_MODE_SCREEN: u64 = 1;
/// **`system_log`'s `arg0` asking for the kernel's ring** (milestone 342). Must match
/// `components/src/system_log.rs`'s `MODE_KERNEL`.
pub(super) const LOG_MODE_KERNEL: u64 = 1;
/// Where `system_log` finds the kernel's ring, read-only. Must match its `RING_VA`.
pub(super) const LOG_RING_VA: u64 = address_space_map::pair_page(0x0070_0000);
/// Where `system_log` finds the kernel ring's cursor page. Must match its `CURSOR_VA`.
pub(super) const LOG_CURSOR_VA: u64 = address_space_map::pair_page(0x0080_0000);

// The two UART mappings are aarch64/riscv64 only: x86 holds COM1 as a port range, which has no page
// to map (milestone 299), so on that architecture these VAs name nothing and the console/input
// drivers hold the port capability instead.
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
pub(super) const CON_UART_VA: u64 = address_space_map::pair_page(0x0070_0000); // console's UART mapping
// line_editor's own pages (the shell's text in, completed lines out) are declared with it now, in
// `line_editor::component`, and `terminal_supervisor` maps them.
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
pub(super) const IN_UART_VA: u64 = address_space_map::pair_page(0x00a0_0000); // input driver's UART mapping
pub(super) const SH_OUT_VA: u64 = address_space_map::pair_page(0x00c0_0000); // the shell's view of the TERM_OUT frame (swish.rs OUT_VA)
pub(super) const LINE_VA: u64 = address_space_map::pair_page(0x00b0_0000); // the shell's view of the TERM_IN frame
pub(super) const SH_FS_VA: u64 = address_space_map::pair_page(0x0060_0000); // the shell's half of the FS contract (swish.rs FS_VA)

/// Where the **shell** maps its own read-only clock (milestone 86). Must match swish.rs's
/// `SH_CLOCK_VA`. A different address from [`CHILD_CLOCK_VA`], because that is where a *child* maps
/// its clock and the shell already maps the terminal's output frame there; two address spaces may
/// agree on an address, one may not.
pub(super) const SH_CLOCK_VA: u64 = address_space_map::pair_page(0x00d0_0000);

// -------------------------------------------------------------------------------------------
// The login stack of milestone 49 (users and attribution): credentialer, identity_provisioner,
// login, login_audit_receiver.
// -------------------------------------------------------------------------------------------

/// Where `credentialer` maps its own provision page. Must match `components/src/credentialer.rs`'s own
/// `PROV_VA`.
pub(super) const CRED_SVC_PROV_VA: u64 = address_space_map::pair_page(0x0000_0000_00e0_0000);
/// Where `credentialer` maps its own verify page. Must match the same file's `VERIFY_VA`.
pub(super) const CRED_SVC_VERIFY_VA: u64 = address_space_map::pair_page(0x0000_0000_00e1_0000);
/// Where `login` maps its relay of the verify page (the same physical frame as
/// [`CRED_SVC_VERIFY_VA`], mapped into a different address space). Must match `components/src/login.rs`'s
/// own `CRED_VA`.
pub(super) const LOGIN_CRED_VA: u64 = address_space_map::pair_page(0x0000_0000_00e3_0000);
/// Where `identity_provisioner` maps the identity/secret this boot stages for it. Must match
/// `components/src/identity_provisioner.rs`'s own `REQ_VA`.
pub(super) const IDP_REQ_VA: u64 = address_space_map::pair_page(0x0000_0000_00e4_0000);
/// Where `identity_provisioner` maps `credentialer`'s provision page (the same physical frame as
/// [`CRED_SVC_PROV_VA`]). Must match the same file's own `PROV_VA`.
pub(super) const IDP_PROV_VA: u64 = address_space_map::pair_page(0x0000_0000_00e0_0000);
/// Where `identity_provisioner` maps the file service's shared page. Must match the same file's own
/// `FS_VA`.
pub(super) const IDP_FS_VA: u64 = address_space_map::pair_page(0x0000_0000_00e5_0000);
/// Where this process briefly maps the page it stages `identity_provisioner`'s request into, in its
/// own address space, before delegating the same physical frame on. Distinct from every VA above
/// (those are addresses inside a *child's* address space); in the same scratch family as
/// [`RNG_DMA_PEEK_VA`] and [`PROGENITOR_SHELL_OUTPUT_VIRTUAL_ADDRESS`].
pub(super) const PROVISION_SCRATCH_VA: u64 = address_space_map::pair_page(0x0f20_0000);

/// `credentialer`'s own construction budget, in pages: matches
/// `kernel::user::credential_service::CRED_BUDGET_PAGES` (6 MiB, sized from `credentialer::Cost::DEFAULT`'s
/// own scratch requirement, not guessed).
pub(super) const CRED_BUDGET_PAGES: u64 = 1536;
/// Extra stack pages `credentialer` needs, beyond the one page `build_child` maps: matches
/// `kernel::user::credential_service::CRED_STACK_PAGES`, itself a measurement (Argon2id's inner
/// loop overflows one page) rather than a guess.
pub(super) const CRED_STACK_PAGES: u64 = 16;
/// `login`'s own construction budget under a real boot, in pages: its one-time costs and a handful
/// of sessions (`components/src/login.rs`'s `OWN_UT_PAGES`, `CHANNEL_UT_PAGES`,
/// `CARETAKER_REGION_PAGES` and `CLIENT_BUDGET_PAGES`), plus the durable session's budget it splits
/// at start-up, derived from `login_contract::durable::BUDGET_PAGES` so the two cannot drift. See
/// notes/system-initializer/budgets.md.
pub(super) const LOGIN_CONSTRUCTION_PAGES: u64 = 768 + login_contract::durable::BUDGET_PAGES;
/// Extra stack pages `login` needs, beyond the one page `build_child` maps: matches
/// `kernel::user::login_service::LOGIN_STACK_PAGES`, "sized against `credentialer.rs`'s own lesson"
/// (that file's own comment).
pub(super) const LOGIN_STACK_PAGES: u64 = 16;

/// `credentialer.rs`'s own readiness sentinel, duplicated here the same way its `PROV_VA`/`VERIFY_VA`
/// already are: a binary crate cannot be imported, so every wiring site that needs to recognize this
/// word states it again (`kernel::user::credential_service`'s own `RPT_READY` is the same
/// duplication one level over). Must match `components/src/credentialer.rs`'s own `RPT_READY`.
pub(super) const CRED_RPT_READY: u64 = 0x_c2ed_0000_0000_0001;
/// `identity_provisioner.rs`'s own success report code, duplicated for the same reason
/// (`kernel::user::identity_provisioner_service`'s own `RPT_OK` is the identical duplication).
/// Must match `components/src/identity_provisioner.rs`'s own `RPT_OK`.
pub(super) const IDP_RPT_OK: u64 = 1;

/// **The demo identity this boot provisions**, once per boot, with a freshly generated password: a
/// role name, so nothing here decides whose account it is
/// (`design/roadmap/0049-users-and-attribution.md`'s BUGS).
pub(super) const DEMO_IDENTITY: &[u8] = b"operator";
/// How many raw bytes of entropy the generated password draws. 12 bytes (96 bits) hex-encoded to
/// [`PASSWORD_HEX_LEN`] printable characters: comfortably past what an online guesser could work
/// through before the boot that generated it is long gone, and short enough to type at a keyboard.
pub(super) const PASSWORD_BYTES: usize = 12;
/// [`PASSWORD_BYTES`] hex-encoded, two characters per byte.
pub(super) const PASSWORD_HEX_LEN: usize = PASSWORD_BYTES * 2;
