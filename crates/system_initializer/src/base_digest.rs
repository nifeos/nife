//! **The progenitor checks the measurement table against the base digest** before it reads a line
//! of it (milestone 695 (the kernel trusts two digests from the loader), part 1). The kernel grants
//! the loader's `nife.base` digest as a read-only page ([`BootEndowment::base_digest_page`]); this
//! module maps it, reads it with `base_digest_contract`, hashes the archive's
//! `program_measurements` entry and refuses the boot, out loud, unless the two agree. See
//! notes/trusted-init/measured-boot.md.
//!
//! Name: ratified 2026-11-03 (UTC), #3102. The module and the file.

use super::*;

/// Where the progenitor maps the base digest page in its own address space, for as long as it
/// takes to copy the content out. In the scratch family of [`RNG_DMA_PEEK_VA`]; given up with
/// `UNMAP` straight after.
///
/// Name: ratified 2026-11-03 (UTC), #3102.
const BASE_DIGEST_PEEK_VIRTUAL_ADDRESS: u64 = address_space_map::pair_page(0x0f60_0000);

/// **Why the progenitor refused the measurement table.** Each names a different fault, and
/// [`reason`](BaseRefused::reason) is the line the console shows.
///
/// Name: ratified 2026-11-03 (UTC), #3102.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BaseRefused {
    /// Nothing in the base digest page's slot: a kernel that granted no digest.
    NoCapability,
    /// The slot holds something `PageFrame::MAP` will not map read-only: the wrong kind of
    /// capability, or one without `READ`.
    Unmappable,
    /// The page mapped, and `base_digest_contract` refused its content.
    Page(base_digest_contract::Refused),
    /// The archive has no `program_measurements` entry to check.
    NoTable,
    /// The table's SHA-256 is not the digest the page carries.
    Mismatch,
}

impl BaseRefused {
    /// One fixed line naming the refusal, for the console.
    pub(super) fn reason(&self) -> &'static str {
        match self {
            BaseRefused::NoCapability => "the kernel granted no base digest page",
            BaseRefused::Unmappable => "the base digest slot holds no page this process may read",
            BaseRefused::Page(refusal) => refusal.reason(),
            BaseRefused::NoTable => "the archive carries no program_measurements table",
            BaseRefused::Mismatch => {
                "the program_measurements table is not the one the base digest names"
            }
        }
    }
}

/// **The check itself**: read the digest out of `page`, the base digest page's content, and hold
/// `table`, the archive's `program_measurements` entry, to it. `Ok` only when the page is well
/// formed and the table's SHA-256 is the digest it carries. Pure, so a host test can call it with
/// a wrong digest.
pub(super) fn check_table(page: &[u8], table: Option<&[u8]>) -> Result<(), BaseRefused> {
    let want = base_digest_contract::read_page(page).map_err(BaseRefused::Page)?;
    let table = table.ok_or(BaseRefused::NoTable)?;
    if measured_boot::sha256(table) != want {
        return Err(BaseRefused::Mismatch);
    }
    Ok(())
}

/// **Refuse the boot unless the archive's table is the one the kernel's base digest names.** Maps
/// the granted page read-only with `ut` paying for the page tables, copies the content, gives the
/// mapping and the capability up, and runs [`check_table`]. On a refusal it writes the reason to
/// the UART ([`refusal_console::say`]) and traps; nothing after it runs.
#[inline(never)]
pub(super) fn require_base_digest(
    g: &BootEndowment,
    ut: u64,
    fs: &nifefs::Fs<'_>,
    uart_shape: u64,
) {
    let verdict = read_granted_page(g, ut)
        .and_then(|page| check_table(&page, fs.read(measured_boot::PROGRAM_MEASUREMENTS)));
    if let Err(refusal) = verdict {
        refusal_console::say(
            g,
            uart_shape,
            &[
                "\r\n  MEASURED BOOT REFUSED by the progenitor: ",
                refusal.reason(),
                "\r\n  halting rather than reading a table nothing vouches for.\r\n",
            ],
        );
        fail()
    }
}

/// Map [`BootEndowment::base_digest_page`], copy its content out and give both the mapping and the
/// capability up.
fn read_granted_page(
    g: &BootEndowment,
    ut: u64,
) -> Result<[u8; base_digest_contract::PAGE_CONTENT_BYTES], BaseRefused> {
    if !is_granted(g.base_digest_page) {
        return Err(BaseRefused::NoCapability);
    }
    if !map_page_frame(
        g.base_digest_page,
        BASE_DIGEST_PEEK_VIRTUAL_ADDRESS,
        false,
        ut,
    ) {
        capability_delete(g.base_digest_page);
        return Err(BaseRefused::Unmappable);
    }
    let mut content = [0u8; base_digest_contract::PAGE_CONTENT_BYTES];
    // SAFETY: the kernel just mapped one page read-only at BASE_DIGEST_PEEK_VIRTUAL_ADDRESS, and
    // PAGE_CONTENT_BYTES (40) is far under a page. Nothing writes the frame after the kernel built
    // it, and `content` is this frame's own stack array, so the two do not overlap.
    unsafe {
        core::ptr::copy_nonoverlapping(
            BASE_DIGEST_PEEK_VIRTUAL_ADDRESS as *const u8,
            content.as_mut_ptr(),
            content.len(),
        );
    }
    process_builder::give_up_own_page(BASE_DIGEST_PEEK_VIRTUAL_ADDRESS);
    capability_delete(g.base_digest_page);
    Ok(content)
}

#[cfg(test)]
mod tests {
    use base_digest_contract::{PAGE_CONTENT_BYTES, Refused, build_page};

    use super::*;

    const TABLE: &[u8] =
        b"console sha256:0000000000000000000000000000000000000000000000000000000000000000\n";

    #[test]
    fn the_table_the_digest_names_passes() {
        let page = build_page(&measured_boot::sha256(TABLE));
        assert_eq!(check_table(&page, Some(TABLE)), Ok(()));
    }

    /// The refusal this module exists for: a digest that names another table.
    #[test]
    fn a_wrong_digest_is_refused_as_a_mismatch() {
        let page = build_page(&measured_boot::sha256(b"another table\n"));
        let refused = check_table(&page, Some(TABLE));
        assert_eq!(refused, Err(BaseRefused::Mismatch));
        assert_eq!(
            refused.unwrap_err().reason(),
            "the program_measurements table is not the one the base digest names"
        );
    }

    /// One flipped byte in the table, with the page left as it was.
    #[test]
    fn a_table_changed_by_one_byte_is_refused() {
        let page = build_page(&measured_boot::sha256(TABLE));
        let mut changed: [u8; TABLE.len()] = TABLE.try_into().expect("same length");
        changed[0] ^= 1;
        assert_eq!(
            check_table(&page, Some(&changed)),
            Err(BaseRefused::Mismatch)
        );
    }

    #[test]
    fn a_missing_table_is_refused() {
        let page = build_page(&measured_boot::sha256(TABLE));
        assert_eq!(check_table(&page, None), Err(BaseRefused::NoTable));
    }

    /// The page is read before the table, so a bad page refuses even beside a missing table.
    #[test]
    fn a_bad_page_is_refused_with_the_contract_reason() {
        let mut page = build_page(&measured_boot::sha256(TABLE));
        page[0] = b'X';
        assert_eq!(
            check_table(&page, None),
            Err(BaseRefused::Page(Refused::NotThisPage))
        );
        page = build_page(&measured_boot::sha256(TABLE));
        page[7] = b'9';
        assert_eq!(
            check_table(&page, Some(TABLE)),
            Err(BaseRefused::Page(Refused::UnsupportedVersion {
                byte: b'9'
            }))
        );
        assert_eq!(
            check_table(&page[..PAGE_CONTENT_BYTES - 1], Some(TABLE)),
            Err(BaseRefused::Page(Refused::ShortPage {
                length: PAGE_CONTENT_BYTES - 1
            }))
        );
        assert_eq!(
            check_table(&build_page(&[0; 32]), Some(TABLE)),
            Err(BaseRefused::Page(Refused::ZeroDigest))
        );
    }

    /// Every refusal the console can show is a different line.
    #[test]
    fn every_refusal_has_its_own_reason() {
        let all = [
            BaseRefused::NoCapability,
            BaseRefused::Unmappable,
            BaseRefused::Page(Refused::NotThisPage),
            BaseRefused::NoTable,
            BaseRefused::Mismatch,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.reason(), b.reason());
            }
        }
    }
}
