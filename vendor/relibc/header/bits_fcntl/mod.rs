// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
use crate::platform::types::c_int;

/// Seek relative to start-of-file.
pub const SEEK_SET: c_int = 0;
/// Seek relative to current position.
pub const SEEK_CUR: c_int = 1;
/// Seek relative to end-of-file.
pub const SEEK_END: c_int = 2;
