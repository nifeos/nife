//! **The startup page**: one read-only page, at one fixed address, that lists where a process's
//! memory windows were mapped. The spawner writes it before the process runs; the process reads
//! each window's address from it instead of compiling the address in. A window is a run of pages
//! the spawner mapped for the process: a shared page, a device's registers, a DMA region.
//!
//! The page is the only address in this contract. Each entry names a [`WindowKind`], an address
//! and a length in pages. A reader takes the kinds it knows and skips the rest, so a later spawner
//! may add kinds and move windows without breaking an earlier program. [`VERSION`] changes only
//! when the entry format itself changes. See
//! design/roadmap/proposals/a-process-reads-its-window-addresses-from-a-startup-page.md.

//! # BUGS
//!
//! - One page per address space, at one address: two programs sharing a space would share one
//!   page, and two instances of one program in one space could not tell their windows apart.
//! - A process the kernel builds by hand, or one a userspace builder spawns, has no page yet;
//!   [`StartupPage::mapped`] there reads unmapped memory and faults, on purpose.
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners, after the page it
//! defines, as `counter_frequency_contract` is named after its page. Considered `window_table`
//! (names the content but not when a process meets it) and `boot_information` (seL4's word, which
//! here would read as the machine's boot).

#![cfg_attr(not(test), no_std)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

/// The page's first eight bytes. A frame without them is not a startup page, and reads as none.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
pub const MAGIC: [u8; 8] = *b"STARTPG1";

/// The page layout's version, the digit [`MAGIC`] carries. A break bumps both in one change.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners. The name every
/// contract crate uses for its layout version.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 23, naming the startup page. The
/// number is this interface's for good; a format change is a new version, never a new number.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners. The name every
/// contract crate uses for its interface identity.
pub const INTERFACE_ID: u64 = 23;

/// **Where the spawner maps the page**, read-only, in every process it starts: the one fixed
/// address a program needs before it can find any other window.
///
/// Siting: a spawner places it for programs that did not choose the address, which is the runtime
/// band's purpose (notes/address-space-map.md, "Where a new page goes"). `PROCESS_PAGES` would
/// share the stack's tables, but its sixteen pages are the per-thread current-CPU slots. One
/// sixteen-MiB step above `std_runtime_contract::INDEX_SOURCES_PAGE`, following that crate's
/// pattern, and clear of the builder scratch window below `0x1400_0000`.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners, after
/// `counter_frequency_contract::PAGE_VIRTUAL_ADDRESS`.
pub const PAGE_VIRTUAL_ADDRESS: u64 = address_space_map::runtime_window(0x1800_0000);

/// The page's size in bytes: one frame.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
pub const PAGE_BYTES: usize = address_space_map::PAGE as usize;

const OFF_MAGIC: usize = 0;
const OFF_COUNT: usize = OFF_MAGIC + 8;
const OFF_ENTRIES: usize = OFF_COUNT + 8;
const ENTRY_BYTES: usize = 16;

/// **How many windows one page can list**: the page less its sixteen-byte header, in sixteen-byte
/// entries.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
pub const MAXIMUM_WINDOWS: usize = (PAGE_BYTES - OFF_ENTRIES) / ENTRY_BYTES;

/// **Which window an entry describes**: a number both sides read from [`kind`]. Zero names none.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowKind(pub u32);

/// **One window**: what it is, where it starts, and how many pages it spans.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// What the window is.
    pub kind: WindowKind,
    /// Its first page's virtual address, page aligned.
    pub address: u64,
    /// Its length in pages, at least one.
    pub pages: u32,
}

impl Window {
    /// The first address past the window.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const fn end(&self) -> u64 {
        self.address + self.pages as u64 * address_space_map::PAGE
    }

    /// Does `va` fall inside the window?
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const fn contains(&self, va: u64) -> bool {
        va >= self.address && va < self.end()
    }
}

/// **Why [`write`] refused a list of windows.**
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// More windows than [`MAXIMUM_WINDOWS`].
    TooManyWindows,
    /// The buffer is shorter than [`PAGE_BYTES`].
    PageTooSmall,
    /// A window of kind zero, of zero pages, not page aligned, or running past the address space.
    MalformedWindow(WindowKind),
    /// Two windows of one kind: a reader could not tell which one it meant.
    DuplicateKind(WindowKind),
}

/// **Write `windows` onto `page`**, which the caller then maps read-only at
/// [`PAGE_VIRTUAL_ADDRESS`]. Every byte past the last entry is left as the caller gave it, so a
/// zeroed frame stays zero there.
///
/// # Examples
///
/// ```
/// use startup_page_contract::{StartupPage, Window, kind, write, PAGE_BYTES};
///
/// let mut page = [0u8; PAGE_BYTES];
/// let dma = Window { kind: kind::E1000E_DATA_PLANE, address: 0x90_0000, pages: 64 };
/// write(&mut page, &[dma]).unwrap();
/// assert_eq!(StartupPage::parse(&page).unwrap().window(kind::E1000E_DATA_PLANE), Some(dma));
/// ```
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
pub fn write(page: &mut [u8], windows: &[Window]) -> Result<(), Refusal> {
    if page.len() < PAGE_BYTES {
        return Err(Refusal::PageTooSmall);
    }
    if windows.len() > MAXIMUM_WINDOWS {
        return Err(Refusal::TooManyWindows);
    }
    for (n, w) in windows.iter().enumerate() {
        let fits = w.address.is_multiple_of(address_space_map::PAGE)
            && w.address
                .checked_add(w.pages as u64 * address_space_map::PAGE)
                .is_some();
        if w.kind.0 == 0 || w.pages == 0 || !fits {
            return Err(Refusal::MalformedWindow(w.kind));
        }
        if windows[..n].iter().any(|earlier| earlier.kind == w.kind) {
            return Err(Refusal::DuplicateKind(w.kind));
        }
    }
    page[OFF_MAGIC..OFF_COUNT].copy_from_slice(&MAGIC);
    page[OFF_COUNT..OFF_COUNT + 4].copy_from_slice(&(windows.len() as u32).to_le_bytes());
    page[OFF_COUNT + 4..OFF_ENTRIES].fill(0);
    for (n, w) in windows.iter().enumerate() {
        let at = OFF_ENTRIES + n * ENTRY_BYTES;
        page[at..at + 4].copy_from_slice(&w.kind.0.to_le_bytes());
        page[at + 4..at + 8].copy_from_slice(&w.pages.to_le_bytes());
        page[at + 8..at + 16].copy_from_slice(&w.address.to_le_bytes());
    }
    Ok(())
}

/// **A startup page, read**: the bytes [`write`] made, checked for the magic and a count that fits.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
#[derive(Debug, Clone, Copy)]
pub struct StartupPage<'a> {
    bytes: &'a [u8],
    count: usize,
}

impl<'a> StartupPage<'a> {
    /// The page in `bytes`, or `None` for bytes that are not one: too short, without [`MAGIC`], or
    /// counting more windows than a page holds.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub fn parse(bytes: &'a [u8]) -> Option<StartupPage<'a>> {
        if bytes.len() < PAGE_BYTES || bytes[OFF_MAGIC..OFF_COUNT] != MAGIC {
            return None;
        }
        let mut count = [0u8; 4];
        count.copy_from_slice(&bytes[OFF_COUNT..OFF_COUNT + 4]);
        let count = u32::from_le_bytes(count) as usize;
        (count <= MAXIMUM_WINDOWS).then_some(StartupPage { bytes, count })
    }

    /// The page the spawner mapped at [`PAGE_VIRTUAL_ADDRESS`], or `None` if the frame there does
    /// not hold one.
    ///
    /// # Safety
    ///
    /// The caller is a process whose spawner mapped a readable page at [`PAGE_VIRTUAL_ADDRESS`],
    /// which stays mapped and unwritten for the life of the process.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub unsafe fn mapped() -> Option<StartupPage<'static>> {
        // SAFETY: the caller promises a readable page of `PAGE_BYTES` at this address, mapped and
        // unchanged for the process's life, which is what a `'static` shared slice needs.
        let bytes =
            unsafe { core::slice::from_raw_parts(PAGE_VIRTUAL_ADDRESS as *const u8, PAGE_BYTES) };
        StartupPage::parse(bytes)
    }

    /// Every window the page lists, in the order the spawner wrote them.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub fn windows(&self) -> impl Iterator<Item = Window> + 'a {
        let bytes = self.bytes;
        (0..self.count).map(move |n| {
            let at = OFF_ENTRIES + n * ENTRY_BYTES;
            let word = |from: usize, to: usize| {
                let mut b = [0u8; 8];
                b[..to - from].copy_from_slice(&bytes[at + from..at + to]);
                u64::from_le_bytes(b)
            };
            Window {
                kind: WindowKind(word(0, 4) as u32),
                pages: word(4, 8) as u32,
                address: word(8, 16),
            }
        })
    }

    /// The window of `kind`, or `None` if the spawner mapped none. Kinds this reader does not ask
    /// for are skipped, which is what lets a later spawner add them.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub fn window(&self, kind: WindowKind) -> Option<Window> {
        self.windows().find(|w| w.kind == kind)
    }
}

/// **The kinds of window a spawner in this tree hands out**, one number each, never reused.
///
/// A number is unique across every program, not only within one, so a page that some day lists
/// the windows of two programs in one space stays unambiguous. [`ALL`] lists them, and a test
/// holds every number apart.
pub mod kind {
    use super::WindowKind;

    /// The NVMe data plane's run: the I/O submission ring, the I/O completion ring, then the
    /// transfer buffer, one window per client.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const NON_VOLATILE_MEMORY_EXPRESS_DATA_PLANE: WindowKind = WindowKind(1);
    /// The one page of the NVMe controller's BAR0 that holds the doorbells, device memory.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const NON_VOLATILE_MEMORY_EXPRESS_DOORBELL_PAGE: WindowKind = WindowKind(2);
    /// The e1000e server's DMA region, normal memory.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const E1000E_DATA_PLANE: WindowKind = WindowKind(3);
    /// BAR0's receive-queue page of the e1000e, device memory.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const E1000E_RECEIVE_QUEUE_PAGE: WindowKind = WindowKind(4);
    /// BAR0's transmit-queue page of the e1000e, device memory.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const E1000E_TRANSMIT_QUEUE_PAGE: WindowKind = WindowKind(5);
    /// The DesignWare Ethernet server's DMA region, normal memory.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const DESIGNWARE_ETHERNET_DATA_PLANE: WindowKind = WindowKind(6);
    /// The DesignWare Ethernet controller's DMA register page, device memory.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const DESIGNWARE_ETHERNET_DIRECT_MEMORY_ACCESS_REGISTER_PAGE: WindowKind = WindowKind(7);
    /// The DesignWare mobile storage controller's register page, device memory.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const DESIGNWARE_MOBILE_STORAGE_REGISTER_PAGE: WindowKind = WindowKind(8);
    /// The DesignWare mobile storage server's transfer buffer, normal memory.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const DESIGNWARE_MOBILE_STORAGE_TRANSFER: WindowKind = WindowKind(9);
    /// The xHCI controller's register pages, device memory, as many as the register mask names.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const EXTENSIBLE_HOST_CONTROLLER_INTERFACE_REGISTERS: WindowKind = WindowKind(10);
    /// The xHCI driver's DMA region, normal memory.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const EXTENSIBLE_HOST_CONTROLLER_INTERFACE_DATA_PLANE: WindowKind = WindowKind(11);

    /// Every kind above, for the test that holds their numbers apart.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
    pub const ALL: [WindowKind; 11] = [
        NON_VOLATILE_MEMORY_EXPRESS_DATA_PLANE,
        NON_VOLATILE_MEMORY_EXPRESS_DOORBELL_PAGE,
        E1000E_DATA_PLANE,
        E1000E_RECEIVE_QUEUE_PAGE,
        E1000E_TRANSMIT_QUEUE_PAGE,
        DESIGNWARE_ETHERNET_DATA_PLANE,
        DESIGNWARE_ETHERNET_DIRECT_MEMORY_ACCESS_REGISTER_PAGE,
        DESIGNWARE_MOBILE_STORAGE_REGISTER_PAGE,
        DESIGNWARE_MOBILE_STORAGE_TRANSFER,
        EXTENSIBLE_HOST_CONTROLLER_INTERFACE_REGISTERS,
        EXTENSIBLE_HOST_CONTROLLER_INTERFACE_DATA_PLANE,
    ];
}

#[cfg(test)]
mod tests;
