// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
// TODO: reuse more code with the thin printf impl
use crate::{
    c_str::{self, WStr},
    header::stdio::printf::inner_printf,
    io::Write,
    platform::types::c_int,
};
use core::ffi::VaList;

pub unsafe fn wprintf(w: impl Write, format: WStr, ap: VaList) -> c_int {
    unsafe { inner_printf::<c_str::Wide>(w, format, ap).unwrap_or(-1) }
}
