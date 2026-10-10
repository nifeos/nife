// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
#[cfg(any(target_os = "linux", target_os = "nife"))]
use crate::platform::types::c_long;

#[cfg(not(any(target_os = "linux", target_os = "nife")))]
use crate::platform::types::c_int;

#[cfg(any(target_os = "linux", target_os = "nife"))]
#[allow(non_camel_case_types)]
/// Used for time in microseconds.
pub type suseconds_t = c_long;
#[cfg(not(any(target_os = "linux", target_os = "nife")))]
#[allow(non_camel_case_types)]
/// Used for time in microseconds.
pub type suseconds_t = c_int;
