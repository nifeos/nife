//! What the kernel and the JH7110 clock and reset server agree on: where the spawner maps the first
//! page of each window the server holds, and the kind each has on the startup page. The server
//! (`components/src/jh7110_clock_and_reset_server.rs`) reads the page to learn which windows it
//! holds; a machine whose tree names no always-on window, for one, gets a server that serves no
//! always-on device. Its clients speak `jh7110_clock_and_reset_contract`, and
//! `jh7110_clock_and_reset_devices` holds what each badge brings up. See
//! notes/jh7110-clock-and-reset.md.
//!
//! Name: provisional.

use address_space_map::pair_page;
use startup_page_contract::WindowKind;

/// Where the spawner maps the system top group (STG) clock and reset window's first page,
/// device-typed.
///
/// Name: provisional.
pub const SYSTEM_TOP_GROUP_VIRTUAL_ADDRESS: u64 = pair_page(0x0060_0000);
/// Where the spawner maps the system (SYS) clock and reset window's first page, device-typed.
///
/// Name: provisional.
pub const SYSTEM_VIRTUAL_ADDRESS: u64 = pair_page(0x0060_1000);
/// Where the spawner maps the always-on clock and reset window's first page, device-typed.
///
/// Name: provisional.
pub const ALWAYS_ON_VIRTUAL_ADDRESS: u64 = pair_page(0x0060_2000);
/// Where the spawner maps the SYS syscon's first page, device-typed and read-only: the server
/// reads phase-locked loop (PLL) 2's words there for a rate and writes nothing.
///
/// Name: provisional.
pub const SYSTEM_CONTROLLER_VIRTUAL_ADDRESS: u64 = pair_page(0x0060_3000);

/// [`SYSTEM_TOP_GROUP_VIRTUAL_ADDRESS`]'s entry on the startup page.
///
/// Name: provisional.
pub const SYSTEM_TOP_GROUP_KIND: WindowKind = WindowKind(20);
/// [`SYSTEM_VIRTUAL_ADDRESS`]'s entry on the startup page.
///
/// Name: provisional.
pub const SYSTEM_KIND: WindowKind = WindowKind(21);
/// [`ALWAYS_ON_VIRTUAL_ADDRESS`]'s entry on the startup page.
///
/// Name: provisional.
pub const ALWAYS_ON_KIND: WindowKind = WindowKind(22);
/// [`SYSTEM_CONTROLLER_VIRTUAL_ADDRESS`]'s entry on the startup page.
///
/// Name: provisional.
pub const SYSTEM_CONTROLLER_KIND: WindowKind = WindowKind(23);

/// The slot the server receives its clients' calls on, each carrying the caller's badge.
///
/// Name: provisional.
pub const REQUEST_SLOT: u64 = 0;
