//! **The startup page**: one read-only page, at one fixed address, that lists where a process's
//! memory windows were mapped. The spawner writes it before the process runs; the process reads
//! each window's address from it instead of compiling the address in. A window is a run of pages
//! the spawner mapped for the process: a shared page, a device's registers, a region a device reaches by direct memory
//! access.
//!
//! The page is the only address in this contract. Each entry names a [`WindowKind`], an address
//! and a length in pages. A reader takes the kinds it knows and skips the rest, so a later spawner
//! may add kinds and move windows without breaking an earlier program. [`VERSION`] changes only
//! when the entry format itself changes. See
//! design/roadmap/1069-a-process-reads-its-window-addresses-from-a-startup-page.md.

//! # BUGS
//!
//! - One page per address space, at one address: two programs sharing a space would share one
//!   page, and two instances of one program in one space could not tell their windows apart.
//! - A process the kernel builds by hand, or one a userspace builder spawns, has no page yet;
//!   [`StartupPage::mapped`] there reads unmapped memory and faults, on purpose.
//! - Each contract crate mints the kinds of the windows it describes, and nothing checks numbers
//!   across crates. Today only `driver_process_contract` mints any, and its own test holds them
//!   apart.
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

/// **Which window an entry describes**: a number minted by the contract crate both sides
/// link, so the agreement is code. Zero names none.
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
    pub virtual_address: u64,
    /// Its length in pages, at least one.
    pub pages: u32,
}

/// **Why [`write()`] refused a list of windows.**
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
/// use startup_page_contract::{StartupPage, Window, WindowKind, write, PAGE_BYTES};
///
/// let mut page = [0u8; PAGE_BYTES];
/// let plane = Window { kind: WindowKind(3), virtual_address: 0x90_0000, pages: 64 };
/// write(&mut page, &[plane]).unwrap();
/// assert_eq!(StartupPage::parse(&page).unwrap().window(WindowKind(3)), Some(plane));
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
        let fits = w.virtual_address.is_multiple_of(address_space_map::PAGE)
            && w.virtual_address
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
        page[at + 8..at + 16].copy_from_slice(&w.virtual_address.to_le_bytes());
    }
    Ok(())
}

/// **A startup page, read**: the bytes [`write()`] made, checked for the magic and a count that fits.
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
                virtual_address: word(8, 16),
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

/// **One window a program finds on its startup page**, by kind, its address read on first use.
///
/// A program declares the kind and how many bytes of it the program uses, as a `static`, and
/// calls [`StartupWindow::virtual_address`] where it would have named a compiled-in constant.
///
/// ```no_run
/// use startup_page_contract::{StartupWindow, WindowKind};
///
/// // The kind comes from the contract crate the spawner and this program both link.
/// const DOORBELL_PAGE: WindowKind = WindowKind(2);
///
/// // SAFETY: this program's spawner writes the startup page and lists the doorbell page on it.
/// static BELLS: StartupWindow = unsafe { StartupWindow::new(DOORBELL_PAGE, 4096) };
///
/// let doorbells = BELLS.virtual_address();
/// ```
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/drivers-read-the-startup-page.
pub struct StartupWindow {
    kind: WindowKind,
    length_bytes: u64,
    first: core::sync::atomic::AtomicU64,
}

impl StartupWindow {
    /// A window of `kind`, of which the program uses the first `length_bytes`.
    ///
    /// # Safety
    ///
    /// The process's spawner maps a startup page at [`PAGE_VIRTUAL_ADDRESS`] for the life of the
    /// process, as [`StartupPage::mapped`] asks. Whether the window itself is mapped is the
    /// spawner's promise, which the kernel checks for its first page at spawn.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/drivers-read-the-startup-page.
    pub const unsafe fn new(kind: WindowKind, length_bytes: u64) -> Self {
        Self {
            kind,
            length_bytes,
            first: core::sync::atomic::AtomicU64::new(0),
        }
    }

    /// How many bytes of the window the program uses, as [`Self::new`] was given.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/drivers-read-the-startup-page.
    pub const fn length_bytes(&self) -> u64 {
        self.length_bytes
    }

    /// The window's first virtual address. Panics, naming the kind, if the startup page is
    /// missing, lists no window of this kind, or lists one shorter than [`Self::length_bytes`]:
    /// each is a spawner that does not match this program, and running on would touch memory
    /// nobody mapped.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/drivers-read-the-startup-page.
    pub fn virtual_address(&self) -> u64 {
        // SAFETY: `new`'s caller promised the spawner maps a startup page at its fixed address
        // for the process's life.
        self.virtual_address_in(|| unsafe { StartupPage::mapped() })
    }

    /// [`Self::virtual_address`], reading the page from `page` on first use: the whole of its
    /// logic, which a host test drives with a page in a buffer.
    fn virtual_address_in<'a>(&self, page: impl FnOnce() -> Option<StartupPage<'a>>) -> u64 {
        use core::sync::atomic::Ordering;
        let cached = self.first.load(Ordering::Relaxed);
        if cached != 0 {
            return cached;
        }
        let page = page()
            .unwrap_or_else(|| panic!("no startup page, so no window of kind {:?}", self.kind));
        let window = page
            .window(self.kind)
            .unwrap_or_else(|| panic!("the startup page lists no window of kind {:?}", self.kind));
        assert!(
            window.pages as u64 * address_space_map::PAGE >= self.length_bytes,
            "the startup page lists {window:?}, shorter than the {} bytes this program uses",
            self.length_bytes,
        );
        // Relaxed: the page never changes, so two threads racing here store the same value.
        self.first.store(window.virtual_address, Ordering::Relaxed);
        window.virtual_address
    }
}

#[cfg(test)]
mod tests;
