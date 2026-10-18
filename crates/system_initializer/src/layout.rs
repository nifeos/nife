//! The address-space layout and page budgets: every address a child or the progenitor maps at, each
//! stack's budget, and the constants each stack's program must agree on. Kept in one file because
//! its correctness is that entries do not collide, and that is checked by reading them side by
//! side. Moved out of `lib.rs` unchanged but for visibility by milestone 843 (the system-
//! initializer crate root is split along its seams), under §279 (the system-initializer crate root
//! splits into private modules).
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

use super::*;

/// Where the kernel maps the initial ramdisk archive, read-only. Must match
/// `kernel::user::INITIAL_RAMDISK_VA`.
pub(super) const INITIAL_RAMDISK_VA: u64 = address_space_map::runtime_window(0x2000_0000);

/// Stack pages every child the progenitor builds gets, mapped down from `supervision_protocol::CHILD_STACK_VA`.
///
/// **Twelve since DECISIONS §67**, and every step of that number was measured rather than chosen.
/// Four overflowed at the first `ls > out.txt`; eight held until `2>` put a **second** `FileOut` on
/// `run_pipeline`'s frame, each carrying a 256-byte staging buffer by value, and the scripted wiring
/// faulted twenty-four bytes below its lowest page. Four extra rather than one, because every
/// previous instance bought exactly enough and the next change found the wall again. The cost is
/// 48 KiB of address space per child, which is nothing next to a page table.
///
/// `kernel::user::pipeline_service`'s `SHELL_EXTRA_STACK` must stay level with this: a test wiring
/// with less headroom than the boot wiring finds faults the boot does not have
/// (notes/pipes/the-boot.md).
/// The kernel cannot depend on this crate (it would drag `user_mode_runtime`'s EL0 syscall stubs in), so that
/// one is still a number in two places, and this is the one it follows.
pub const CHILD_STACK_PAGES: u64 = 12;

/// Where a child that declares a clock maps it, read-only. Must match `components/src/date.rs`'s
/// `CLOCK_VA` and `kernel/src/user/clock_service.rs`.
pub(super) const CHILD_CLOCK_VA: u64 = address_space_map::pair_page(0x00c0_0000);

/// Where a child that declares the inert-configuration page maps it, read-only (DECISIONS §111).
/// Must match `components/src/printenv.rs`'s `CONFIG_VA`. A different address from `std_service.rs`'s
/// `CONFIG_PAGE_STD`: that std program is spawned by a different wiring entirely (the `-Zbuild-std`
/// farm's own harness), in its own address space, so there is no collision to avoid, only two
/// numbers that happen not to need to agree.
pub(super) const CHILD_CONFIG_VA: u64 = address_space_map::pair_page(0x00e0_0000);

/// **Whether the boot prompt's session is handed the machine statistics page** (milestone 126,
/// DECISIONS §225 part 2). §225 ruled the page granted to every login by default and withholdable
/// by the machine owner, and this is the owner's switch, in the one file the owner's other
/// boot-time policy (the run-unvouched capability) already lives in. `false` and the shell never
/// holds the page, so every `free` prints that it cannot see the machine and `vmstat` refuses,
/// rather than either printing zeroes.
///
/// **An exception worth marking**: the page already travels with the session
/// (`spawnproto::MACHINE_BIT`), so a per-login policy is `login` handing each session the page or
/// not. Nothing hands it to `login` yet; this constant decides for the boot prompt alone.
///
/// Name: ratified 2026-09-27 (calef, #1360's table).
pub const GRANT_MACHINE_PAGE: bool = true;

/// Where a supervised (interruptible) child maps its shared job frame (DECISIONS §24 (interrupting the foreground process)). A pair page
/// on the address-space map; must match `interrupt_heeder.rs` and
/// `interrupt_ignorer.rs`'s `JOB_PAGE_FRAME_VA`.
pub(super) const CHILD_JOB_PAGE_FRAME_VA: u64 = address_space_map::pair_page(0x0030_0000);

/// Pages of untyped split off our own budget and handed the shell (milestone 31), so the shell can
/// in turn endow the programs it spawns (`run --mem N`) out of a budget that is genuinely *its own*.
/// The shell shrinks this by N pages per grant; the pages a spawned child pins are not reclaimed in
/// phase 1, so this is a session budget, not a renewable one. The number is
/// [`spawnproto::SHELL_BUDGET_PAGES`], which the shell reads too.
pub(super) const SH_BUDGET_PAGES: u64 = spawnproto::SHELL_BUDGET_PAGES;

/// **What the progenitor keeps for itself after the boot servers are up** (milestone 22, the interactive
/// increment). It pays for one thing: the page tables reaching the loader's scratch window, which
/// are the progenitor's own mappings and must never come out of a child's region (tearing that region down
/// would free the progenitor's tables under a window it never unmaps). One L3 covers 512 scratch pages.
///
/// **Bounded since milestone 604 (provisional).** The loader's cursor used to climb forever, so this
/// budget was spent one table per 2 MiB of everything ever built, and ran dry after about 200 MiB of
/// builds past boot (the block has the arithmetic): about seventy-five `ripgrep`-sized spawns, a
/// little before the cursor would have reached the initial ramdisk window. Now the cursor wraps
/// inside `supervision_protocol::SCRATCH_WINDOW`, whose tables cost at most
/// `supervision_protocol::SCRATCH_TABLE_PAGES` (32) however long the machine runs. The assertion
/// below keeps that half of this budget spoken for; the other half pays for `job_undertaker`, which
/// is built out of this region (its image, twelve stack pages, its tables, its address space and
/// thread), and the tables under `ACTIVATION_FS_VA` and `IMAGE_STAGING_VA`.
pub const INIT_OWN_PAGES: u64 = 128;
const _: () = assert!(INIT_OWN_PAGES >= 2 * supervision_protocol::SCRATCH_TABLE_PAGES);

/// **One job's region**: everything a spawned program is made of, so a single reclaim frees all of
/// it. The biggest program the prompt can spawn is `date` at seven pages, plus
/// [`CHILD_STACK_PAGES`], a TCB, an address-space root, the intermediate tables for the four windows
/// a child touches, and the §13 mapping records. Forty was that with room to spare, and it is spent
/// per *live* job rather than per job ever run.
///
/// **Forty-one since 2026-09-26**, and the one is counted rather than chosen. The address-space map
/// of milestone 206 (a program image has under 896 KiB) put a child's image and stack in the second
/// gigabyte and left its pair pages in the first, so a child with any window mapped now walks two
/// gigabyte-level tables where it walked one. The stack's leaf table absorbed the current-CPU page's,
/// so that is the whole difference. The "room to spare" was not there: the first `x86_64`
/// `script/swish-check` after the map refused `mdr gate.txt`, whose debug image is 18 pages rather
/// than `date`'s seven, and `x86_64` also pays three tables for its timebase page.
///
/// **Forty-eight since milestone 126 (the `procps` package), and counted the same way.** `top`
/// gained the machine statistics page, a pair page of its own, and on `x86_64` its debug image is
/// nineteen pages; at forty-one the progenitor refused to build it ("could not spawn") in the
/// `x86_64` leg of `script/swish-check`, and at forty-eight that leg passes every line. The pool is
/// six of these, so the raise costs the job pool forty-two pages.
pub(super) const JOB_REGION_PAGES: u64 = 48;

/// **One directory-granted job's region**: the program *and* the `fs_subtree_caretaker` that carries
/// its grant, plus the two endpoints between them, all out of one carve.
///
/// One region rather than two is DECISIONS §92 (a caretaker is supervised by the client it serves)
/// read through the mechanism of §40 (a supervisor's death is its subtree's death). A caretaker's
/// serve loop never returns, so it never dies of its own accord; built in a region of its own, that region never
/// comes home and §16's LIFO rule then pins the region above it too, and six `rm`s would end the
/// prompt. Built out of the region it serves, it is inside the client's subtree, and the one reclaim
/// `job_undertaker` already performs ends both. **The two endpoints are retyped from this region too,
/// and that is load-bearing rather than tidy**: `sched::reap_region_objects` sweeps a region's
/// endpoints before it looks at its threads, and that sweep is what wakes a caretaker parked in
/// `RECEIVE` so it can be collected. An endpoint carved from the progenitor's own budget would leave it blocked on
/// something the teardown never touches, and a blocked thread never reaches the `schedule()` that
/// spends §16's kill.
///
/// **Ninety-six rather than eighty**, and the extra is not slack for its own sake: the caretaker is
/// a second address space with its own tables and its own stack, and the failure mode of getting it
/// wrong is `build_child` answering `Err(())` mid-boot-command, which reads at the prompt as "could
/// not spawn" with no way to tell a small region from an empty pool.
///
/// **Ninety-eight since 2026-09-26**: two address spaces, each one gigabyte-level table dearer under
/// the address-space map, for the reason [`JOB_REGION_PAGES`] gives.
pub(super) const DIR_JOB_REGION_PAGES: u64 = 98;

/// The stack a `fs_subtree_caretaker` gets, beyond the one page `build_child` maps for it.
///
/// Four rather than [`CHILD_STACK_PAGES`]'s twelve, and it is a measurement of the program rather
/// than a guess: it has no allocator, no recursion, and one frame that matters, a
/// `[Option<u64>; 16]` handle table plus a `grant::MAX_NAME` name buffer. Twelve pages per caretaker
/// would be 32 KiB of a region this crate is already sizing carefully.
pub(super) const CARETAKER_STACK_PAGES: u64 = 4;

/// Where a `fs_subtree_caretaker` and the program it serves both map the FS contract's shared page.
/// Must match `components/src/fs_subtree_caretaker.rs`'s `PAGE_VA` and `components/src/rm.rs`'s.
///
/// One address for both because they are two ends of one contract and neither is the other's parent:
/// a request travels caretaker-to-server and program-to-caretaker through the same frame, so a
/// second VA would only be a second name for the same page.
pub(super) const FS_CLIENT_PAGE_VA: u64 = address_space_map::pair_page(0x0060_0000);

/// Where `fs_nameset_caretaker` reads its name set, read-only (milestone 205). Must match that
/// program's `SET_VA`, as [`FS_CLIENT_PAGE_VA`] matches its `PAGE_VA`.
pub(super) const NAMESET_PAGE_VA: u64 = address_space_map::pair_page(0x0070_0000);

/// **One shell-boot second-directory caretaker's region** (milestone 154's "wiring a second
/// grant into the real boot"). Sized for one caretaker alone, the way [`CARETAKER_STACK_PAGES`]
/// already is: unlike [`DIR_JOB_REGION_PAGES`], nothing else is built out of this region, because
/// the confined program here is the shell itself, already built directly out of `ut` rather than
/// out of a region of its own. Conservative rather than tight (the same headroom
/// [`JOB_REGION_PAGES`] carries for one child), since this is spent once per boot rather than
/// once per command.
pub(super) const SECOND_DIR_CARETAKER_PAGES: u64 = JOB_REGION_PAGES;

/// **The job pool.** Six live jobs at once, which is far more than a prompt has ever needed and is
/// deliberately small: the whole claim of this increment is that a *bounded* budget is enough once
/// the regions come back, so a budget nobody could exhaust would prove nothing. `script/swish-check`
/// runs thirteen jobs through it, so widening this silently retires that gate.
///
/// **Plus one `std` program's region** (milestone 595 (provisional)), so a `std` job has room of its
/// own rather than needing every native job before it reclaimed first: it is nearly ten native
/// regions' worth ([`grant_plan::STD_REGION_PAGES`]), and a pool of 240 would fit it only when
/// empty. The ratchet above still holds at 630 pages: `script/swish-check` runs more than twenty
/// jobs, which is well past what the pool could hold without the regions coming back.
pub const JOBS_BUDGET_PAGES: u64 = JOB_REGION_PAGES * 6 + grant_plan::STD_REGION_PAGES;

/// **The image pool**: where a file run by its path, or a package being installed, is staged and
/// built (milestone 595 (the shell runs a `std` program), 2026-09-27). Provisional name.
///
/// Room for the largest image twice (this process's own copy, which §219 has it hash, and the
/// child's pages) and a `std` program's heap beside it: `grant_plan::image_region_pages` for a
/// `std` image of [`spawnproto::IMAGE_MAX_PAGES`], plus its staging. About 9.5 MiB of the
/// progenitor's 48.
///
/// **Apart from [`JOBS_BUDGET_PAGES`] on purpose.** That pool is small so that `script/swish-check`
/// runs more jobs through it than it could hold without the regions coming back; folding 2,400
/// pages into it would retire that ratchet without saying so. Named programs never carve from this
/// one, and an image never carves from that one.
///
/// # BUGS
///
/// - **The image is held three times while a child is built**: the shell's frames, this copy, and
///   the child's pages. Hashing each byte as it is copied into the child would drop this copy and
///   halve the pool, but it means building the child from the caller's still-mapped frames, and
///   the tree's one loader would have to learn to hash as it fills. That is more work than the copy,
///   and the copy is why this pool is twice the image rather than once: a choice made on effort.
/// - **Reserved, not borrowed.** These pages are set aside at boot whether or not an image ever
///   runs, as the shell's matching staging pages are.
pub const IMAGE_POOL_PAGES: u64 = grant_plan::image_region_pages(
    spawnproto::IMAGE_MAX_PAGES,
    grant_plan::StartupContract::Std,
    JOB_REGION_PAGES,
) + spawnproto::IMAGE_MAX_PAGES;

/// **Everything the job pool can have built at once fits in the loader's scratch window**, twice
/// over (milestone 604 (provisional)). Every page the progenitor builds for a job is mapped once in
/// its own scratch window and stays mapped until the job's region is destroyed, so the window must
/// hold the whole pool at once, plus an image request's peek pages, beside the boot servers' pages,
/// which are never given back. Half the window is left for those. They were not measured, and the
/// sum of every program in the archive is under 2,400 pages of file, so half is generous; a pool
/// grown past this fails the build here rather than a spawn at run time. The image pool
/// ([`IMAGE_POOL_PAGES`], milestone 595 (the shell runs a `std` program)) is built through the
/// same window, so it counts too.
const _: () = assert!(
    JOBS_BUDGET_PAGES + IMAGE_POOL_PAGES + spawnproto::IMAGE_MAX_PAGES
        <= supervision_protocol::SCRATCH_WINDOW_PAGES / 2
);

/// Where the progenitor maps the shell's output frame in **its own** address space, to print the one line it
/// ever prints (the dropped-authority negative control). Well clear of the progenitor's segments, its stack,
/// and the loader's scratch window at `0x1000_0000`.
pub(super) const INIT_OUT_VA: u64 = address_space_map::pair_page(0x0f00_0000);

/// Where the progenitor briefly maps the virtio-rng DMA page, in **its own** address space, to read
/// [`VIRTIO_DMA_PHYS_OFFSET`] back out before handing the same frame on to entropy. Distinct from
/// [`INIT_OUT_VA`]. Given up with `UNMAP` the moment the offset is read (milestone 95 (an unmap
/// primitive), §249 (a running address space stays nameable)); it was mapped for the life of the
/// boot until then.
pub(super) const RNG_DMA_PEEK_VA: u64 = address_space_map::pair_page(0x0f10_0000);

/// The DMA region's own physical base, written inside the page itself at its last eight bytes
/// (`abi::virtio::DMA_PHYS_OFFSET`, which carries the reasoning for exactly this offset). It was a
/// second copy of the kernel's number until milestone 600 (provisional), on a "no crate the two
/// could share" argument that `abi` already answered; it became a crate constant when `gpu_driver`
/// and `keyboard_driver` started reading it too, which made it a contract between peer programs.
pub(super) const VIRTIO_DMA_PHYS_OFFSET: u64 = abi::virtio::DMA_PHYS_OFFSET;

/// Where entropy maps its own DMA page. Must match `components/src/entropy.rs`'s `DMA_VA`.
pub(super) const RNG_DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

/// `entropy.rs`'s own spawn-argument convention (`components/src/entropy.rs`'s `MODE_VIRTIO`): the
/// kernel's (and now this crate's) shared understanding with the one program it spawns, not a wire
/// contract between two user programs, so rule 7 does not apply the way it does to `RNG_DMA_VA`.
pub(super) const RNG_MODE_VIRTIO: u64 = 0;

// -------------------------------------------------------------------------------------------
// The network stack (milestone 590 (provisional), the booted system starts its network stack).
// -------------------------------------------------------------------------------------------

/// Where this process briefly maps the network card's DMA page to read [`VIRTIO_DMA_PHYS_OFFSET`]
/// back out, [`RNG_DMA_PEEK_VA`]'s twin: given up with `UNMAP` once the offset is read (milestone 95
/// (an unmap primitive)). Until then this process kept a writable view of the NIC's rings for the
/// life of the boot.
pub(super) const NET_DMA_PEEK_VA: u64 = address_space_map::pair_page(0x0f30_0000);

/// Where `net_stack` maps its DMA page. Must match `components/src/virtio_net_transport.rs`'s `DMA_VA`, the
/// same kernel-and-spawner convention [`RNG_DMA_VA`] is.
pub(super) const NET_DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

/// `net_stack`'s heap budget, in pages: matches
/// `kernel::user::virtio_service::NET_SERVER_BUDGET_PAGES`,
/// which carries the measurement (the heap is capped at 96 pages by `net_stack`'s own `HEAP_MAX`,
/// and the rest is page tables for the heap and for clients' shared frames). Stated on this side for
/// [`CRED_BUDGET_PAGES`]'s reason: a spawner states the budget it hands, and there is no crate a
/// kernel constant and a user-mode spawner could share it through that would not be one number
/// with two readers who both need the comment.
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

/// `domain_name_resolver`'s stack, in pages, counted whole as [`NET_STACK_STACK_PAGES`] is: the kernel
/// harness's eight extra (measured: the debug build faulted below three on its first resolve,
/// 2026-10-06 UTC) and the one every process gets.
///
/// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
/// The kernel harness's `name_resolver_tests::RESOLVER_STACK_PAGES` is older; the pull request
/// records no other name considered.
pub(super) const RESOLVER_STACK_PAGES: u64 = 9;

/// `net_stack`'s own spawn-argument convention: entry role 0 is the server
/// (`components/src/net_stack.rs`'s
/// `_start`). [`RNG_MODE_VIRTIO`]'s reasoning.
pub(super) const NET_STACK_ROLE_SERVER: u64 = 0;

/// `line_editor.rs`'s own spawn-argument convention (`components/src/line_editor.rs`'s `MODE_CONSOLE`):
/// [`RNG_MODE_VIRTIO`]'s own reasoning, one program over. The pre-milestone-177 wiring: prints
/// through the console's bespoke two-endpoint protocol.
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
/// [`RNG_DMA_PEEK_VA`] and [`INIT_OUT_VA`].
pub(super) const PROVISION_SCRATCH_VA: u64 = address_space_map::pair_page(0x0f20_0000);

/// `credentialer`'s own construction budget, in pages: matches
/// `kernel::user::credential_service::CRED_BUDGET_PAGES` (6 MiB, sized from `credentialer::Cost::DEFAULT`'s
/// own scratch requirement, not guessed).
pub(super) const CRED_BUDGET_PAGES: u64 = 1536;
/// Extra stack pages `credentialer` needs, beyond the one page `build_child` maps: matches
/// `kernel::user::credential_service::CRED_STACK_PAGES`, itself a measurement (Argon2id's inner
/// loop overflows one page) rather than a guess.
pub(super) const CRED_STACK_PAGES: u64 = 16;
/// `login`'s own construction budget under a real boot, in pages. Sized for a handful of real
/// sessions across the boot's whole life (`OWN_UT_PAGES` 128 + `CHANNEL_UT_PAGES` 32, both
/// `components/src/login.rs`'s own one-time costs, plus `CARETAKER_REGION_PAGES` 64 + `CLIENT_BUDGET_PAGES`
/// 64 per session that never logs out), not the much larger figure
/// `kernel::user::login_tests::CONSTRUCTION_PAGES` carries for a whole guest-test suite's worth of
/// logins against one shared instance.
///
/// **Plus one durable session's budget** (milestone 152 (durable delegation)), which `login` splits
/// at start-up whenever it is handed `user_timetable_keeper` and `timetable`, before it serves anyone. Without it,
/// 768 pages less `login`'s 128 + 32 + the durable budget is less than nothing, and `login` stopped at
/// `fail(2)`. Derived from `login_contract::durable::BUDGET_PAGES` so the two cannot drift.
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

/// **The demo identity this boot provisions**, once per boot, with a freshly generated password
/// (see `have_login_stack`'s own block for the reasoning): a role name rather than a specific
/// family member's, deliberately, because the whole point of a boot-generated credential is that
/// nothing here has to decide *whose* account this is (`design/roadmap/0049-users-and-attribution.md`'s
/// own BUGS names this as the reason the generated shape was recommended over a baked-in one).
pub(super) const DEMO_IDENTITY: &[u8] = b"operator";
/// How many raw bytes of entropy the generated password draws. 12 bytes (96 bits) hex-encoded to
/// [`PASSWORD_HEX_LEN`] printable characters: comfortably past what an online guesser could work
/// through before the boot that generated it is long gone, and short enough to type at a keyboard.
pub(super) const PASSWORD_BYTES: usize = 12;
/// [`PASSWORD_BYTES`] hex-encoded, two characters per byte.
pub(super) const PASSWORD_HEX_LEN: usize = PASSWORD_BYTES * 2;
