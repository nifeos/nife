//! **A child's code page**, built once for every test that spawns one.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// **A fresh page from `region` holding `code`, ready for the instruction fetcher**: the kernel's
/// `user::supervised_child::code_page`, which the boot self-test's children use too. Panics when the
/// region is spent or `code` does not fit one page.
#[cfg(test)]
pub(super) fn code_page(region: u64, code: &[u32]) -> u64 {
    super::supervised_child::code_page(region, code)
        .expect("no code frame, or the code does not fit a page")
}
