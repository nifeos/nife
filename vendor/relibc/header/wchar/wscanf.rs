// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
use crate::{
    c_str::{self, WStr},
    header::stdio::{reader::Reader, scanf::inner_scanf},
    platform::types::c_int,
};
use core::ffi::VaList as va_list;

pub unsafe fn scanf(r: Reader<'_, c_str::Wide>, format: WStr, ap: va_list) -> c_int {
    match unsafe { inner_scanf::<c_str::Wide>(r, format.into(), ap) } {
        Ok(n) => n,
        Err(n) => n,
    }
}
