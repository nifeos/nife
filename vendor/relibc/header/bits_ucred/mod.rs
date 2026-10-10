// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
use crate::platform::types::{gid_t, pid_t, uid_t};

/// Non-POSIX, see <https://www.man7.org/linux/man-pages/man7/unix.7.html>.
///
/// Represents UNIX credentials.
#[repr(C)]
#[derive(Clone, Debug)]
// FIXME: CheckVsLibcCrate
pub struct ucred {
    /// Process ID of the sending process.
    pub pid: pid_t,
    /// User ID of the sending process.
    pub uid: uid_t,
    /// Group ID of the sending process.
    pub gid: gid_t,
}
