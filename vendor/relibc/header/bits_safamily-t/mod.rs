// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
use crate::platform::types::c_ushort;

/// Represents a socket address family.
pub type sa_family_t = c_ushort;
