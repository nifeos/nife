// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
#[cfg(any(target_os = "linux", target_os = "nife"))]
use crate::platform::types::c_uint;

#[cfg(not(any(target_os = "linux", target_os = "nife")))]
use crate::platform::types::c_int;

/// Used for some file attributes.
#[allow(non_camel_case_types)]
#[cfg(any(target_os = "linux", target_os = "nife"))]
pub type mode_t = c_uint;
/// Used for some file attributes.
#[allow(non_camel_case_types)]
#[cfg(not(any(target_os = "linux", target_os = "nife")))]
pub type mode_t = c_int;
