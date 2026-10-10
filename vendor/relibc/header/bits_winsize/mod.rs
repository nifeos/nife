// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
use crate::platform::types::c_ushort;

#[repr(C)]
#[derive(Default)]
pub struct winsize {
    /// Rows, in characters.
    ws_row: c_ushort,
    /// Columns, in characters.
    ws_col: c_ushort,
    /// Non-POSIX, see <https://www.man7.org/linux/man-pages/man2/TIOCSWINSZ.2const.html>.
    ///
    /// Unused.
    ws_xpixel: c_ushort,
    /// Non-POSIX, see <https://www.man7.org/linux/man-pages/man2/TIOCSWINSZ.2const.html>.
    ///
    /// Unused.
    ws_ypixel: c_ushort,
}

impl winsize {
    /// Return the row and column in that order in a tuple.
    pub fn get_row_col(&self) -> (c_ushort, c_ushort) {
        (self.ws_row, self.ws_col)
    }
}
