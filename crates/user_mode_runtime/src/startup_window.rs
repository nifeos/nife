//! **A window this process finds on its startup page**, read once and then used like a
//! [`MappedWindow`]. The spawner lists each window it mapped on the startup page
//! (`startup_page_contract`); a program declares the kind and the length it needs, and the first
//! use reads the address the spawner chose. The program compiles in no address but the page's own.
//! See design/roadmap/proposals/a-process-reads-its-window-addresses-from-a-startup-page.md.
//!
//! ```no_run
//! use startup_page_contract::WindowKind;
//! use user_mode_runtime::startup_window::StartupWindow;
//!
//! // The kind comes from the contract crate the spawner and this program both link.
//! const DOORBELL_PAGE: WindowKind = WindowKind(2);
//!
//! // SAFETY: this program's spawner lists the doorbell page and maps it read/write.
//! static BELLS: StartupWindow = unsafe { StartupWindow::new(DOORBELL_PAGE, 4096) };
//!
//! BELLS.get().w32(0, 1);
//! ```
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane/drivers-read-the-startup-page, after the page
//! and the type it hands back.

use core::sync::atomic::{AtomicU64, Ordering};

use startup_page_contract::{StartupPage, WindowKind};

use crate::mapped_window::MappedWindow;

/// **One window of a known kind and length**, its address read from the startup page on first use.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/drivers-read-the-startup-page.
pub struct StartupWindow {
    kind: WindowKind,
    len: u64,
    base: AtomicU64,
}

impl StartupWindow {
    /// A window of `kind`, of which the program uses the first `len` bytes.
    ///
    /// # Safety
    ///
    /// The process's spawner writes a startup page, lists a window of `kind` on it, and maps at
    /// least `len` bytes of it readable and writable for as long as the program uses it. The kernel
    /// asserts at spawn that the window's first page is mapped, and [`Self::get`] refuses a window
    /// the page lists as shorter than `len`; the rest is the trust [`MappedWindow::new`] asks for.
    pub const unsafe fn new(kind: WindowKind, len: u64) -> Self {
        Self {
            kind,
            len,
            base: AtomicU64::new(0),
        }
    }

    /// The window. Panics, naming the kind, if the startup page is missing, lists no window of this
    /// kind, or lists one shorter than the length [`Self::new`] was given: each is a spawner that
    /// does not match this program, and running on would touch memory nobody mapped.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/drivers-read-the-startup-page.
    pub fn get(&self) -> MappedWindow {
        let mut base = self.base.load(Ordering::Relaxed);
        if base == 0 {
            base = self.read_page();
            // Relaxed: the page never changes, so two threads racing here store the same value.
            self.base.store(base, Ordering::Relaxed);
        }
        // SAFETY: `new`'s caller promised the spawner maps `len` bytes of this kind read/write, and
        // `read_page` took `base` from the page's own entry, which it checked is at least `len` long.
        unsafe { MappedWindow::new(base, self.len) }
    }

    fn read_page(&self) -> u64 {
        // SAFETY: `new`'s caller promised the spawner writes a startup page, which it maps
        // read-only at its fixed address for the process's life.
        let page = unsafe { StartupPage::mapped() }
            .unwrap_or_else(|| panic!("no startup page, so no window of kind {:?}", self.kind));
        let window = page
            .window(self.kind)
            .unwrap_or_else(|| panic!("the startup page lists no window of kind {:?}", self.kind));
        assert!(
            window.pages as u64 * crate::mapped_window::PAGE >= self.len,
            "the startup page lists {window:?}, shorter than the {} bytes this program uses",
            self.len,
        );
        window.virtual_address
    }
}
